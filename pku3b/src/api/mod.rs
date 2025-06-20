mod low_level;

use url::Url;
use anyhow::Context;
use chrono::TimeZone;
use cyper::IntoUrl;
use itertools::Itertools;
use futures_util::future::join_all;
use scraper::Selector;
use scraper::ElementRef;
use std::{
    collections::{HashMap, HashSet,VecDeque},
    hash::{Hash, Hasher},
    str::FromStr,
    sync::Arc,
};
use compio::runtime::Runtime;
use crate::{
    multipart, qs,
    utils::{with_cache, with_cache_bytes},
};

const ONE_HOUR: std::time::Duration = std::time::Duration::from_secs(3600);
const ONE_DAY: std::time::Duration = std::time::Duration::from_secs(3600 * 24);
const AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
const MAX_DEPTH: usize = 20;  // 最大深度限制
struct ClientInner {
    http_client: low_level::LowLevelClient,
    cache_ttl: Option<std::time::Duration>,
    download_artifact_ttl: Option<std::time::Duration>,
}

impl std::fmt::Debug for ClientInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientInner")
            .field("cache_ttl", &self.cache_ttl)
            .field("download_artifact_ttl", &self.download_artifact_ttl)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Client(Arc<ClientInner>);

impl std::ops::Deref for Client {
    type Target = low_level::LowLevelClient;

    fn deref(&self) -> &Self::Target {
        &self.0.http_client
    }
}

impl Client {
    pub fn new(
        cache_ttl: Option<std::time::Duration>,
        download_artifact_ttl: Option<std::time::Duration>,
    ) -> Self {
        let mut default_headers = http::HeaderMap::new();
        default_headers.insert(http::header::USER_AGENT, AGENT.parse().unwrap());
        let http_client = cyper::Client::builder()
            .cookie_store(true)
            .default_headers(default_headers)
            .build();

        log::info!("Cache TTL: {:?}", cache_ttl);
        log::info!("Download Artifact TTL: {:?}", download_artifact_ttl);

        Self(
            ClientInner {
                http_client: low_level::LowLevelClient::from_cyper_client(http_client),
                cache_ttl,
                download_artifact_ttl,
            }
            .into(),
        )
    }

    pub fn new_nocache() -> Self {
        Self::new(None, None)
    }

    pub async fn blackboard(&self, username: &str, password: &str) -> anyhow::Result<Blackboard> {
        let c = &self.0.http_client;
        let value = c.oauth_login(username, password).await?;
        let token = value
            .as_object()
            .context("value not an object")?
            .get("token")
            .context("password not correct")?
            .as_str()
            .context("property 'token' not string")?
            .to_owned();
        c.bb_sso_login(&token).await?;

        log::debug!("iaaa oauth token for {username}: {token}");

        Ok(Blackboard {
            client: self.clone(),
        })
    }

    pub fn syncify<F, T>(&self, fut: F) -> anyhow::Result<T>
    where
        F: std::future::Future<Output = anyhow::Result<T>>,
    {
        let rt = compio::runtime::Runtime::new()?;
        rt.block_on(fut)
    }

    pub fn blackboard_sync(&self, username: &str, password: &str) -> anyhow::Result<Blackboard> {
        self.syncify(self.blackboard(username, password))
    }

    pub fn cache_ttl(&self) -> Option<&std::time::Duration> {
        self.0.cache_ttl.as_ref()
    }

    pub fn download_artifact_ttl(&self) -> Option<&std::time::Duration> {
        self.0.download_artifact_ttl.as_ref()
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(Some(ONE_HOUR), Some(ONE_DAY))
    }
}

#[derive(Debug)]
pub struct Blackboard {
    client: Client,
    // token: String,
}

impl Blackboard {
    pub fn client(&self) -> Client {
        self.client.clone()
    }
    async fn _get_courses(&self) -> anyhow::Result<Vec<(String, String, bool)>> {
        let dom = self.client.bb_homepage().await?;
        let re = regex::Regex::new(r"key=([\d_]+),").unwrap();
        let ul_sel = Selector::parse("ul.courseListing").unwrap();
        let sel = Selector::parse("li a").unwrap();


        let f = |a: scraper::ElementRef<'_>| {
            let href = a.value().attr("href").unwrap();
            let text = a.text().collect::<String>();
            // use regex to extract course key (of form key=_80052_1)

            let key = re
                .captures(href)
                .and_then(|s| s.get(1))
                .context("course key not found")?
                .as_str()
                .to_owned();

            Ok((key, text))
        };

        // the first one contains the courses in the current semester
        let ul = dom.select(&ul_sel).nth(0).context("courses not found")?;
        let courses = ul.select(&sel).map(f).collect::<anyhow::Result<Vec<_>>>()?;

        // the second one contains the courses in the previous semester
        let ul_history = dom.select(&ul_sel).nth(1).context("courses not found")?;
        let courses_history = ul_history
            .select(&sel)
            .map(f)
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(courses
            .into_iter()
            .map(|(k, t)| (k, t, true))
            .chain(courses_history.into_iter().map(|(k, t)| (k, t, false)))
            .collect())
    }
    pub async fn get_courses(&self, only_current: bool) -> anyhow::Result<Vec<CourseHandle>> {
        log::info!("fetching courses...");
        let courses = with_cache(
            "Blackboard::_get_courses",
            self.client.cache_ttl(),
            self._get_courses(),
        )
        .await?;

        let mut courses = courses
            .into_iter()
            .map(|(id, long_title, is_current)| {
                Ok(CourseHandle {
                    client: self.client.clone(),
                    meta: CourseMeta {
                        id,
                        long_title,
                        is_current,
                    }
                    .into(),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        if only_current {
            courses.retain(|c| c.meta.is_current);
        }

        Ok(courses)
    }
}

#[derive(Debug)]
pub struct CourseMeta {
    id: String,
    long_title: String,
    /// 是否是当前学期的课程
    is_current: bool,
}

impl CourseMeta {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Course Name (semester)
    pub fn title(&self) -> &str {
        self.long_title.split_once(":").unwrap().1.trim()
    }

    /// Cousre Name
    pub fn name(&self) -> &str {
        let s = self.title();
        let i = s
            .char_indices()
            .filter(|(_, c)| *c == '(')
            .last()
            .unwrap()
            .0;
        s.split_at(i).0.trim()
    }
}

#[derive(Debug, Clone)]
pub struct CourseHandle {
    client: Client,
    meta: Arc<CourseMeta>,
}

impl CourseHandle {
    pub async fn _get(&self) -> anyhow::Result<HashMap<String, String>> {
        let dom = self.client.bb_coursepage(&self.meta.id).await?;

        let entries = dom
            .select(&Selector::parse("#courseMenuPalette_contents > li > a").unwrap())
            .map(|a| {
                let text = a.text().collect::<String>();
                let href = a.value().attr("href").unwrap();
                Ok((text, href.to_owned()))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        Ok(entries)
    }
    pub fn title(&self) -> &str {
        self.meta.title()
    }
    pub fn id(&self) -> &str {
        self.meta.id()
    }

    pub async fn get(&self) -> anyhow::Result<Course> {
        log::info!("fetching course {}", self.meta.title());

        let entries = with_cache(
            &format!("CourseHandle::_get_{}", self.meta.id),
            self.client.cache_ttl(),
            self._get(),
        )
        .await?;

        Ok(Course {
            client: self.client.clone(),
            meta: self.meta.clone(),
            entries,
        })
    }
    /// 直接把内部 Course 的 list_assignments 暴露出去，供 CLI / PyO3 使用
    pub async fn list_assignments(
        &self,
    ) -> anyhow::Result<Vec<CourseAssignmentHandle>> {
        let course = self.get().await?;
        course.list_assignments().await
    }
}

#[derive(Debug, Clone)]
pub struct Course {
    client: Client,
    meta: Arc<CourseMeta>,
    entries: HashMap<String, String>,
}

impl Course {
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn meta(&self) -> &CourseMeta {
        &self.meta
    }

    // pub fn content_stream(&self) -> CourseContentStream {
    //     CourseContentStream::new(
    //         self.client.clone(),
    //         self.meta.clone(),
    //         self.entries()
    //             .iter()
    //             .filter_map(|(_, uri)| {
    //                 let url = low_level::convert_uri(uri).ok()?.into_url().ok()?;
    //                 if !low_level::LIST_CONTENT.ends_with(url.path()) {
    //                     return None;
    //                 }

    //                 let (_, content_id) = url.query_pairs().find(|(k, _)| k == "content_id")?;

    //                 Some(content_id.to_string())
    //             })
    //             .collect(),
    //     )
    // }
    pub fn content_stream(&self) -> CourseContentStream {
        let mut initial_probes = Vec::new();
        
        // 为每个栏目创建初始探针
        for (section_name, uri) in &self.entries {
            // 修正：使用 Url::parse 代替 into_url
            if let Ok(uri_str) = low_level::convert_uri(uri) {
                if let Ok(url) = Url::parse(&uri_str) {
                    if low_level::LIST_CONTENT.ends_with(url.path()) {
                        for (key, value) in url.query_pairs() {
                            if key == "content_id" {
                                let probe = ContentProbe {
                                    parent_id: None,
                                    parent_title: None,
                                    depth: 0,
                                    id: value.to_string(),
                                    section_name: Some(section_name.clone()),
                                };
                                initial_probes.push(probe);
                            }
                        }
                    }
                }
            }
        }
        
        CourseContentStream::new(
            self.client.clone(),
            self.meta.clone(),
            initial_probes,
        )
    }

    pub fn build_content(&self, data: CourseContentData) -> CourseContent {
        CourseContent {
            client: self.client.clone(),
            course: self.meta.clone(),
            data: data.into(),
        }
    }

    pub fn entries(&self) -> &HashMap<String, String> {
        &self.entries
    }
    #[allow(dead_code)]
    pub async fn query_launch_link(&self, uri: &str) -> anyhow::Result<String> {
        let res = self.client.get_by_uri(uri).await?;
        let st = res.status();
        anyhow::ensure!(st.as_u16() == 302, "invalid status: {}", st);
        let loc = res
            .headers()
            .get("location")
            .context("location header not found")?
            .to_str()
            .context("location header not str")?
            .to_owned();

        Ok(loc)
    }
    pub async fn get_video_list(&self) -> anyhow::Result<Vec<CourseVideoHandle>> {
        log::info!("fetching video list for course {}", self.meta.title());

        let videos = with_cache(
            &format!("Course::get_video_list_{}", self.meta.id),
            self.client.cache_ttl(),
            self._get_video_list(),
        )
        .await?;

        let videos = videos
            .into_iter()
            .map(|meta| {
                Ok(CourseVideoHandle {
                    client: self.client.clone(),
                    meta: meta.into(),
                    course: self.meta.clone(),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(videos)
    }
    async fn _get_video_list(&self) -> anyhow::Result<Vec<CourseVideoMeta>> {
        let u = low_level::VIDEO_LIST.into_url()?;
        let dom = self.client.bb_course_video_list(&self.meta.id).await?;

        let videos = dom
            .select(&Selector::parse("tbody#listContainer_databody > tr").unwrap())
            .map(|tr| {
                let title = tr
                    .child_elements()
                    .nth(0)
                    .unwrap()
                    .text()
                    .collect::<String>();
                let s = Selector::parse("span.table-data-cell-value").unwrap();
                let mut values = tr.select(&s);
                let time = values
                    .next()
                    .context("time not found")?
                    .text()
                    .collect::<String>();
                let _ = values.next().context("teacher not found")?;
                let link = values.next().context("video link not found")?;
                let a = link
                    .child_elements()
                    .next()
                    .context("video link anchor not found")?;
                let link = a
                    .value()
                    .attr("href")
                    .context("video link not found")?
                    .to_owned();

                Ok(CourseVideoMeta {
                    title,
                    time,
                    url: u.join(&link)?.to_string(),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(videos)
    }
    /// 列出本课程全部作业句柄（AssignmentHandle）
    pub async fn list_assignments(
        &self,
    ) -> anyhow::Result<Vec<CourseAssignmentHandle>> {
        let mut stream = self.content_stream();
        let mut list   = Vec::new();

        while let Some(batch) = stream.next_batch().await {
            for data in batch {
                if let Some(ah) = self.build_content(data).into_assignment_opt() {
                    list.push(ah);
                }
            }
        }
        Ok(list)
    }
    /// 列出本课程全部回放句柄
    pub async fn list_videos(&self) -> anyhow::Result<Vec<CourseVideoHandle>> {
        self.get_video_list().await
    }
    /// 列出本课程所有 Document（课件 / 通知等）内容句柄
    pub async fn list_documents(&self)
        -> anyhow::Result<Vec<CourseDocumentHandle>>
    {
        let mut stream = self.content_stream();
        let mut docs   = Vec::new();

        while let Some(batch) = stream.next_batch().await {
            docs.extend(
                batch.into_iter()
                    .filter(|d| matches!(d.kind, CourseContentKind::Document))
                    .map(|data| CourseDocumentHandle {
                        client:  self.client.clone(),
                        course:  self.meta.clone(),
                        content: data.into(),          // Arc<CourseContentData>
                    })
            );
        }
        Ok(docs)
    }
    pub async fn list_announcements(&self) -> anyhow::Result<Vec<CourseAnnouncementHandle>> {
        /* ① 找到 entries 里 “课程通知” */
        let url = match self.entries().get("课程通知") {
            Some(u) => u,
            None => return Ok(vec![]), // 本课没开通知
        };
        /* ② launch_link -> 302 */
        let real = self.query_launch_link(url).await?;
        /* ③ 拉页面，解析 li.announcement */
        let dom  = self.client.get_by_uri(&real).await?.text().await?;
        let doc  = scraper::Html::parse_document(&dom);
        let sel  = scraper::Selector::parse("li.announcement").unwrap();

        let out = doc.select(&sel)
            .filter_map(|li|{
                let title = li.select(&scraper::Selector::parse("a").unwrap())
                              .next()?.text().collect::<String>();
                let href  = li.select(&scraper::Selector::parse("a").unwrap())
                              .next()?.value().attr("href")?.to_string();
                let time  = li.select(&scraper::Selector::parse("span.date").unwrap())
                              .next()?.text().collect::<String>();
                let id    = href.split_once("annId=").map(|(_,x)|x).unwrap_or(&href).to_string();
                Some(CourseAnnouncementHandle{
                    client : self.client.clone(),
                    course : self.meta.clone(),
                    meta   : Arc::new(CourseAnnouncementMeta{ id,title,time,href }),
                })
            })
            .collect();
        Ok(out)
    }
    // pub async fn list_assignments_with_hierarchy(
    //     &self,
    // ) -> anyhow::Result<(
    //     Vec<CourseAssignmentHandle>,
    //     Vec<usize>,             // depths
    //     Vec<Option<String>>,    // parent_ids
    // )> {
    //     let mut stream = self.content_stream();
    //     let mut handles = Vec::new();
    //     let mut depths = Vec::new();
    //     let mut parent_ids = Vec::new();

    //     while let Some(batch) = stream.next_batch().await {
    //         for data in batch {
    //             if let Some(ah) = self.build_content(data.clone()).into_assignment_opt() {
    //                 handles.push(ah);
    //                 depths.push(data.depth);
    //                 parent_ids.push(data.parent_id);
    //             }
    //         }
    //     }
        
    //     Ok((handles, depths, parent_ids))
    // }
    /// 带层级信息的作业列表
    pub async fn list_assignments_with_hierarchy(&self) -> anyhow::Result<Vec<CourseAssignmentHandle>> {
        let mut stream = self.content_stream();
        let mut results = Vec::new();
        
        while let Some(batch) = stream.next_batch().await {
            for content in batch {
                if let CourseContentKind::Assignment = content.kind {
                    results.push(CourseAssignmentHandle {
                        client: self.client.clone(),
                        course: self.meta.clone(),
                        content: Arc::new(content),
                    });
                }
            }
        }
        
        Ok(results)
    }

    /// 列出本课程所有 Document（包含层级信息）
    pub async fn list_documents_with_hierarchy(
        &self,
    ) -> anyhow::Result<(
        Vec<CourseDocumentHandle>,
        Vec<usize>,             // depths
        Vec<Option<String>>,    // parent_ids
    )> {
        let mut stream = self.content_stream();
        let mut handles = Vec::new();
        let mut depths = Vec::new();
        let mut parent_ids = Vec::new();

        while let Some(batch) = stream.next_batch().await {
            for data in batch {
                if matches!(data.kind, CourseContentKind::Document) {
                    handles.push(CourseDocumentHandle {
                        client: self.client.clone(),
                        course: self.meta.clone(),
                        content: Arc::new(data.clone()),
                    });
                    depths.push(data.depth);
                    parent_ids.push(data.parent_id);
                }
            }
        }
        
        Ok((handles, depths, parent_ids))
    }
}

/// 队列元素：要探测的 content_id 及其层级路径
struct Probe {
    id:   String,
    path: Vec<String>,
}
/// 内容探测结构体，包含层级信息
#[derive(Debug)]
struct ContentProbe {
    parent_id: Option<String>,
    parent_title: Option<String>,
    depth: usize,
    id: String,
    section_name: Option<String>,
}
pub struct CourseContentStream {
    /// 一次性发射的请求数量
    batch_size: usize,
    client: Client,
    course: Arc<CourseMeta>,
    visited_ids: HashSet<String>,
    // // probe_ids: Vec<String>,
    probe_queue: VecDeque<ContentProbe>, // 修改为 ContentProbe 队列
    // // 保留关系映射
    parent_map: HashMap<String, Option<String>>,
    depth_map: HashMap<String, usize>,          // ID -> 深度
   
}

impl CourseContentStream {
    // fn new(client: Client, course: Arc<CourseMeta>, probe_ids: Vec<String>) -> Self {
    //     // implicitly deduplicate probe_ids
    //     let visited_ids = HashSet::from_iter(probe_ids);
    //     let probe_ids = visited_ids.iter().cloned().collect();
    //     Self {
    //         batch_size: 8,
    //         client,
    //         course,
    //         visited_ids,
    //         probe_ids,
    //     }
    // }
    // 保留原有new函数签名
    // fn new(client: Client, course: Arc<CourseMeta>, probe_ids: Vec<String>) -> Self {
    //     let visited_ids = HashSet::from_iter(probe_ids.iter().cloned());
    //     // 转换初始 probe_ids 为新类型
    //     let probe_ids_deque = probe_ids.into_iter()
    //         .map(|id| (None, 0, id))  // 初始深度为0，无父节点
    //         .collect::<VecDeque<_>>();
        
    //     Self {
    //         batch_size: 8,
    //         client,
    //         course,
    //         visited_ids,
    //         probe_ids: probe_ids_deque,
            
    //         // 初始化层级信息
    //         parent_map: HashMap::new(),
    //         depth_map: HashMap::new(),
    //     }
    // }
    fn new(client: Client, course: Arc<CourseMeta>, initial_probes: Vec<ContentProbe>) -> Self {
        let visited_ids = initial_probes.iter()
            .map(|p| p.id.clone())
            .collect::<HashSet<_>>();
            
        let probe_queue = VecDeque::from(initial_probes);
        let mut parent_map = HashMap::new();
        let mut depth_map = HashMap::new();
        
        for probe in &probe_queue {
            parent_map.insert(probe.id.clone(), probe.parent_id.clone());
            depth_map.insert(probe.id.clone(), probe.depth);
        }
        
        Self {
            batch_size: 8,
            client,
            course,
            visited_ids,
            probe_queue,
            parent_map,
            depth_map,
        }
    }
    
    // async fn try_next_batch(&mut self, ids: &[String]) -> anyhow::Result<Vec<CourseContentData>> {
    //     let futs = ids
    //         .iter()
    //         .map(|id| self.client.bb_course_content_page(&self.course.id, id));

    //     let doms = futures_util::future::join_all(futs).await;

    //     let mut all_contents = Vec::new();
    //     for dom in doms {
    //         let dom = dom?;
    //         let selector = Selector::parse("#content_listContainer > li").unwrap();
    //         let contents = dom
    //             .select(&selector)
    //             .filter_map(|li| {
    //                 CourseContentData::from_element(li)
    //                     .inspect_err(|e| log::warn!("CourseContentData::from_element error: {e}"))
    //                     .ok()
    //             })
    //             // filter out visited ids
    //             .filter(|data| self.visited_ids.insert(data.id.to_owned()))
    //             // add the rest new ids to probe_ids
    //             .inspect(|data| {
    //                 if data.has_link {
    //                     self.probe_ids.push(data.id.to_owned())
    //                 }
    //             });

    //         all_contents.extend(contents);
    //     }

    //     Ok(all_contents)
    // }
    // async fn try_next_batch(&mut self) -> anyhow::Result<Vec<CourseContentData>> {
    //     let mut batch = Vec::with_capacity(self.batch_size);
        
    //     while !self.probe_ids.is_empty() && batch.len() < self.batch_size {
    //         let (parent_id, depth, id) = self.probe_ids.pop_front().unwrap();
            
    //         // 深度检查（防止无限递归）
    //         if depth >= MAX_DEPTH {
    //             log::warn!("达到最大深度限制: {} (当前深度 {})", id, depth);
    //             continue;
    //         }
            
    //         // 记录父关系
    //         self.parent_map.insert(id.clone(), parent_id.clone());
    //         self.depth_map.insert(id.clone(), depth);
            
    //         // 获取内容页面
    //         let dom = match self.client.bb_course_content_page(&self.course.id, &id).await {
    //             Ok(dom) => dom,
    //             Err(e) => {
    //                 log::warn!("请求失败: {}，将重试。错误: {}", id, e);
    //                 self.probe_ids.push_back((parent_id.clone(), depth, id.clone()));
    //                 continue;
    //             }
    //         };
            
    //         let selector = Selector::parse("#content_listContainer > li").unwrap();
            
    //         // 直接循环处理列表项（不再使用并发）
    //         for li in dom.select(&selector) {
    //             match CourseContentData::from_element(li, Some(&id), depth + 1) {
    //                 Ok(mut data) => {
    //                     // 设置父ID和深度
    //                     data.parent_id = parent_id.clone();
    //                     data.depth = depth + 1;
                        
    //                     // 检查是否已访问
    //                     if self.visited_ids.insert(data.id.clone()) {
    //                         // 添加到当前批次
    //                         batch.push(data.clone());
                            
    //                         // 记录关系映射
    //                         self.parent_map.insert(data.id.clone(), Some(id.clone()));
                            
    //                         // 如果有链接，添加到待探测队列
    //                         if data.has_link {
    //                             self.probe_ids.push_back((
    //                                 Some(data.id.clone()),
    //                                 depth + 1,
    //                                 data.id.clone()
    //                             ));
    //                         }
    //                     } else {
    //                         log::debug!("跳过重复内容: {}", data.id);
    //                     }
    //                 }
    //                 Err(e) => log::warn!("解析元素错误: {}", e),
    //             }
    //         }
    //     }
        
    //     Ok(batch)
    // }
    async fn try_next_batch(&mut self) -> anyhow::Result<Vec<CourseContentData>> {
        if self.probe_queue.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut batch = Vec::with_capacity(self.batch_size);
        let mut to_process = Vec::with_capacity(self.batch_size);
        
        // 准备要处理的探测点
        for _ in 0..self.batch_size {
            if let Some(probe) = self.probe_queue.pop_front() {
                if probe.depth >= MAX_DEPTH {
                    log::warn!("达到最大深度限制: {} (当前深度 {})", probe.id, probe.depth);
                    continue;
                }
                to_process.push(probe);
            } else {
                break;
            }
        }
        
        // 并发获取页面
        let futs = to_process.iter().map(|p| {
            self.client.bb_course_content_page(&self.course.id, &p.id)
        });
        
        let doms = join_all(futs).await;
        
        // 处理结果
        for (probe, dom_result) in to_process.into_iter().zip(doms.into_iter()) {
            match dom_result {
                Ok(dom) => {
                    let selector = Selector::parse("#content_listContainer > li").unwrap();
                    for li in dom.select(&selector) {
                        match CourseContentData::from_element(
                            li,
                            probe.parent_id.as_deref(),
                            probe.parent_title.as_deref(),
                            probe.depth,
                            probe.section_name.as_deref(),
                        ) {
                            Ok(mut data) => {
                                // 更新层级映射
                                self.parent_map.insert(data.id.clone(), Some(probe.id.clone()));
                                self.depth_map.insert(data.id.clone(), data.depth);
                                
                                // 添加到批次
                                batch.push(data.clone());
                                
                                // 如果是文件夹且有链接，添加到探测队列
                                if data.is_folder && data.has_link {
                                    let child_probe = ContentProbe {
                                        parent_id: Some(data.id.clone()),
                                        parent_title: Some(data.title.clone()), // 使用文件夹标题作为子节点的父标题
                                        depth: data.depth,
                                        id: data.id.clone(),
                                        section_name: probe.section_name.clone(), // 保持同一栏目
                                    };
                                    
                                    self.probe_queue.push_back(child_probe);
                                }
                            }
                            Err(e) => log::warn!("解析元素错误: {}", e),
                        }
                    }
                }
                Err(e) => {
                    log::warn!("内容页面获取失败 {}: {}", probe.id, e);
                    // 重新加入队列重试
                    self.probe_queue.push_back(probe);
                }
            }
        }
        
        Ok(batch)
    }
    
    pub async fn next_batch(&mut self) -> Option<Vec<CourseContentData>> {
        match self.try_next_batch().await {
            Ok(batch) if !batch.is_empty() => Some(batch),
            Ok(_) => None,
            Err(e) => {
                log::warn!("try_next_batch 错误: {}", e);
                None
            }
        }
    }
    pub fn get_parent(&self, id: &str) -> Option<&str> {
        self.parent_map.get(id)
            .and_then(|opt| opt.as_ref().map(|s| s.as_str()))
    }
    pub fn get_children(&self, id: &str) -> Vec<&str> {
        self.parent_map.iter()
            .filter(|(_, parent)| parent.as_ref().map_or(false, |p| p == id))
            .map(|(child, _)| child.as_str())
            .collect()
    }
    
    // pub async fn next_batch(&mut self) -> Option<Vec<CourseContentData>> {
    //     let ids = self
    //         .probe_ids
    //         .split_off(self.probe_ids.len().saturating_sub(self.batch_size));
    //     if ids.is_empty() {
    //         return None;
    //     }
    //     match self.try_next_batch(&ids).await {
    //         Ok(r) => Some(r),
    //         Err(e) => {
    //             log::warn!("try_next_batch error {ids:?}: {e}");
    //             return Box::pin(self.next_batch()).await;
    //         }
    //     }
    // }
    // pub fn num_finished(&self) -> usize {
    //     self.visited_ids.len() - self.probe_ids.len()
    // }
    pub fn num_finished(&self) -> usize {
        self.visited_ids.len() - self.probe_queue.len()
    }
    pub fn len(&self) -> usize {
        self.visited_ids.len()
    }
}

#[derive(Debug, Clone)]
pub struct CourseContent {
    client: Client,
    course: Arc<CourseMeta>,
    data: Arc<CourseContentData>,
}

impl CourseContent {
    pub fn into_assignment_opt(self) -> Option<CourseAssignmentHandle> {
        if let CourseContentKind::Assignment = self.data.kind {
            Some(CourseAssignmentHandle {
                client: self.client,
                course: self.course,
                content: self.data,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
enum CourseContentKind {
    Document,
    Assignment,
    Folder, // 添加 Folder 变体
    Unknown,
}

#[derive(Debug,Clone, serde::Deserialize, serde::Serialize)]
pub struct CourseContentData {
    id: String,
    title: String,
    kind: CourseContentKind,
    has_link: bool,
    descriptions: Vec<String>,
    attachments: Vec<(String, String)>,
    // 新增字段：父节点ID和层级深度
    parent_id: Option<String>,
    pub parent_title: Option<String>, // 新增：父节点标题
    depth: usize,
    pub section_name: Option<String>, // 新增：所属栏目名称（如"课程课件"）
    pub is_folder: bool, // 是否是文件夹
}

fn collect_text(element: scraper::ElementRef) -> String {
    let mut text_content = String::new();
    for node_ref in element.children() {
        match node_ref.value() {
            scraper::node::Node::Text(text) => {
                if !text.trim().is_empty() {
                    text_content.push_str(text);
                }
            }
            scraper::node::Node::Element(el) => {
                if el.name() != "script" {
                    if let Some(child_element) = scraper::ElementRef::wrap(node_ref) {
                        text_content.push_str(&collect_text(child_element));
                    }
                }
            }
            _ => {}
        }
    }
    text_content
}

impl CourseContentData {
    // fn from_element(el: scraper::ElementRef<'_>) -> anyhow::Result<Self> {
    //     anyhow::ensure!(el.value().name() == "li", "not a li element");

    //     // ── ① 3 个子节点：图标 / 标题 div / 详情 div ─────────────
    //     let (img, title_div, detail_div) =
    //         el.child_elements().take(3).collect_tuple().unwrap();

    //     // ── ② 内容类型判定 ───────────────────────────────────────
    //     let kind = match img.attr("alt") {
    //         Some("作业")               => CourseContentKind::Assignment,
    //         Some("项目") | Some("文件") => CourseContentKind::Document,
    //         alt                       => {
    //             log::warn!("unknown content kind: {alt:?}");
    //             CourseContentKind::Unknown
    //         }
    //     };

    //     // ── ③ 基本字段 ─────────────────────────────────────────
    //     let id   = title_div.attr("id").context("content_id not found")?.to_owned();
    //     let title= title_div.text().collect::<String>().trim().to_owned();
    //     let has_link = title_div.select(&Selector::parse("a").unwrap()).next().is_some();

    //     // ── ④ 描述正文（纯文本）─────────────────────────────────
    //     let descriptions = detail_div
    //         .select(&Selector::parse("div.vtbegenerated > *").unwrap())
    //         .map(|p| collect_text(p).trim().to_owned())
    //         .collect::<Vec<_>>();

    //     // ── ⑤ (A) 原有 <a> 附件 ────────────────────────────────
    //     let mut attachments = detail_div
    //         .select(&Selector::parse("ul.attachments > li > a").unwrap())
    //         .map(|a| {
    //             let text = a.text().collect::<String>().trim_start_matches('\u{a0}').to_owned();
    //             let href = a.value().attr("href").unwrap().to_owned();
    //             Ok((text, href))
    //         })
    //         .collect::<anyhow::Result<Vec<_>>>()?;

    //     // ── ⑥ (B) 额外把 <img src=...> 也当作附件 ──────────────
    //     /*-----------  (B) 嵌入 <img> 修正版 ----------*/
    //     for (idx, img) in detail_div.select(&Selector::parse("img").unwrap()).enumerate() {
    //         if let Some(src) = img.value().attr("src") {
    //             // 1. 路径过滤：仍然要求 /bbcswebdav/
    //             if !src.starts_with("/bbcswebdav/") {
    //                 continue;
    //             }

    //             /* ---------- 2. 扩展名黑名单 ---------- */
    //             // 取最后一个 '.' 后缀（若有）
    //             let ext = src.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    //             // gif / svg / ico 这些 99% 是装饰图标
    //             if matches!(ext.as_deref(), Some("gif" | "svg" | "ico")) {
    //                 continue;
    //             }

    //             /* ---------- 3. 生成文件名 ---------- */
    //             let fname = Url::parse(&format!("https://dummy{src}"))
    //                 .ok()
    //                 .and_then(|u| u.path_segments().and_then(|seg| seg.last()).map(|s| s.to_string()))
    //                 .filter(|s| !s.is_empty())
    //                 // 无扩展名时补 `.bin`
    //                 .map(|s| if s.contains('.') { s } else { format!("{s}.bin") })
    //                 .unwrap_or_else(|| format!("embed_img_{idx}.bin"));

    //             attachments.push((fname, src.to_string()));
    //         }
    //     }

    //     // ── ⑦ 结果 ─────────────────────────────────────────────
    //     Ok(CourseContentData {
    //         id,
    //         title,
    //         kind,
    //         has_link,
    //         descriptions,
    //         attachments,
    //     })
    // }
    pub fn is_folder(&self) -> bool {
        self.is_folder
    }
    // fn from_element(
    //     el: scraper::ElementRef<'_>,
    //     parent_id: Option<&str>, // 新增：父节点ID
    //     depth: usize            // 新增：层级深度
    // ) -> anyhow::Result<Self> {
    //     // ...现有解析逻辑保持不变...
    //     anyhow::ensure!(el.value().name() == "li", "not a li element");

    //     // ── ① 3 个子节点：图标 / 标题 div / 详情 div ─────────────
    //     let (img, title_div, detail_div) =
    //         el.child_elements().take(3).collect_tuple().unwrap();

    //     // ── ② 内容类型判定 ───────────────────────────────────────
    //     let kind = match img.attr("alt") {
    //         Some("作业")               => CourseContentKind::Assignment,
    //         Some("项目") | Some("文件") => CourseContentKind::Document,
    //         alt                       => {
    //             log::warn!("unknown content kind: {alt:?}");
    //             CourseContentKind::Unknown
    //         }
    //     };

    //     // ── ③ 基本字段 ─────────────────────────────────────────
    //     let id   = title_div.attr("id").context("content_id not found")?.to_owned();
    //     let title= title_div.text().collect::<String>().trim().to_owned();
    //     let has_link = title_div.select(&Selector::parse("a").unwrap()).next().is_some();

    //     // ── ④ 描述正文（纯文本）─────────────────────────────────
    //     let descriptions = detail_div
    //         .select(&Selector::parse("div.vtbegenerated > *").unwrap())
    //         .map(|p| collect_text(p).trim().to_owned())
    //         .collect::<Vec<_>>();

    //     // ── ⑤ (A) 原有 <a> 附件 ────────────────────────────────
    //     let mut attachments = detail_div
    //         .select(&Selector::parse("ul.attachments > li > a").unwrap())
    //         .map(|a| {
    //             let text = a.text().collect::<String>().trim_start_matches('\u{a0}').to_owned();
    //             let href = a.value().attr("href").unwrap().to_owned();
    //             Ok((text, href))
    //         })
    //         .collect::<anyhow::Result<Vec<_>>>()?;

    //     // ── ⑥ (B) 额外把 <img src=...> 也当作附件 ──────────────
    //     /*-----------  (B) 嵌入 <img> 修正版 ----------*/
    //     for (idx, img) in detail_div.select(&Selector::parse("img").unwrap()).enumerate() {
    //         if let Some(src) = img.value().attr("src") {
    //             // 1. 路径过滤：仍然要求 /bbcswebdav/
    //             if !src.starts_with("/bbcswebdav/") {
    //                 continue;
    //             }

    //             /* ---------- 2. 扩展名黑名单 ---------- */
    //             // 取最后一个 '.' 后缀（若有）
    //             let ext = src.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    //             // gif / svg / ico 这些 99% 是装饰图标
    //             if matches!(ext.as_deref(), Some("gif" | "svg" | "ico")) {
    //                 continue;
    //             }

    //             /* ---------- 3. 生成文件名 ---------- */
    //             let fname = Url::parse(&format!("https://dummy{src}"))
    //                 .ok()
    //                 .and_then(|u| u.path_segments().and_then(|seg| seg.last()).map(|s| s.to_string()))
    //                 .filter(|s| !s.is_empty())
    //                 // 无扩展名时补 `.bin`
    //                 .map(|s| if s.contains('.') { s } else { format!("{s}.bin") })
    //                 .unwrap_or_else(|| format!("embed_img_{idx}.bin"));

    //             attachments.push((fname, src.to_string()));
    //         }
    //     }
        
    //     Ok(CourseContentData {
    //         id,
    //         title,
    //         kind,
    //         has_link,
    //         descriptions,
    //         attachments,
    //         // 新添加的层级信息
    //         parent_id: parent_id.map(String::from),
    //         depth,
    //     })
    // }
    pub fn from_element(
        el: ElementRef,
        parent_id: Option<&str>,
        parent_title: Option<&str>,
        depth: usize,
        section_name: Option<&str>,
    ) -> anyhow::Result<Self> { // 修正：使用 anyhow::Result
        anyhow::ensure!(el.value().name() == "li", "not a li element");

        // ── ① 3 个子节点：图标 / 标题 div / 详情 div ─────────────
        let (img, title_div, detail_div) = match el.child_elements().take(3).collect_tuple() {
            Some(tup) => tup,
            None => anyhow::bail!("Expected at least 3 child elements"),
        };

        // ── ② 内容类型判定 ───────────────────────────────────────
        let alt = img.attr("alt");
        let (kind, is_folder) = match alt {
            Some("作业") => (CourseContentKind::Assignment, false),
            Some("内容文件夹") | Some("文件夹") | Some("目录") => (CourseContentKind::Folder, true), // 使用 Folder 变体
            Some("项目") | Some("文件") => (CourseContentKind::Document, false),
            Some(alt) => {
                log::warn!("unknown content kind: {alt:?}");
                (CourseContentKind::Unknown, false)
            }
            None => (CourseContentKind::Unknown, false),
        };

        // ── ③ 基本字段 ─────────────────────────────────────────
        let id = title_div.attr("id").context("content_id not found")?.to_owned();
        let title = title_div.text().collect::<String>().trim().to_owned();
        let has_link = title_div.select(&Selector::parse("a").unwrap()).next().is_some();

        // ── ④ 描述正文（纯文本）─────────────────────────────────
        let descriptions = detail_div
            .select(&Selector::parse("div.vtbegenerated > *").unwrap())
            .map(|p| Self::collect_text(p).trim().to_owned())
            .collect::<Vec<_>>();

        // ── ⑤ (A) 原有 <a> 附件 ────────────────────────────────
        let mut attachments = detail_div
            .select(&Selector::parse("ul.attachments > li > a").unwrap())
            .map(|a| {
                let text = a.text().collect::<String>().trim_start_matches('\u{a0}').to_owned();
                let href = a.value().attr("href").unwrap().to_owned();
                Ok((text, href))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // ── ⑥ (B) 额外把 <img src=...> 也当作附件 ──────────────
        for (idx, img) in detail_div.select(&Selector::parse("img").unwrap()).enumerate() {
            if let Some(src) = img.value().attr("src") {
                // 1. 路径过滤：仍然要求 /bbcswebdav/
                if !src.starts_with("/bbcswebdav/") {
                    continue;
                }

                // 2. 扩展名黑名单
                let ext = src.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
                if matches!(ext.as_deref(), Some("gif" | "svg" | "ico")) {
                    continue;
                }

                // 3. 生成文件名
                let fname = Url::parse(&format!("https://dummy{src}"))
                    .ok()
                    .and_then(|u| u.path_segments().and_then(|seg| seg.last()).map(|s| s.to_string()))
                    .filter(|s| !s.is_empty())
                    .map(|s| if s.contains('.') { s } else { format!("{s}.bin") })
                    .unwrap_or_else(|| format!("embed_img_{idx}.bin"));

                attachments.push((fname, src.to_string()));
            }
        }
        
        Ok(Self {
            id,
            title,
            kind,
            has_link,
            descriptions,
            attachments,
            // 新增层级字段
            parent_id: parent_id.map(ToOwned::to_owned),
            parent_title: parent_title.map(ToOwned::to_owned),
            depth,
            section_name: section_name.map(ToOwned::to_owned),
            is_folder,
        })
    }
    
    /// 递归收集元素的文本内容
    fn collect_text(element: ElementRef) -> String {
        let mut buffer = String::new();
        
        for node in element.children() {
            match node.value() {
                scraper::node::Node::Text(text) => buffer.push_str(text),
                scraper::node::Node::Element(el) => {
                    if el.name() == "script" || el.name() == "style" {
                        continue; // 跳过脚本和样式
                    }
                    if let Some(child) = ElementRef::wrap(node) {
                        buffer.push_str(&Self::collect_text(child));
                    }
                }
                _ => {}
            }
        }
        
        buffer
    }
}


#[derive(Debug, Clone)]
pub struct CourseAssignmentHandle {
    pub client: Client,
    pub course: Arc<CourseMeta>,
    pub content: Arc<CourseContentData>,
}

impl CourseAssignmentHandle {
    // pub fn id(&self) -> String {
    //     let mut hasher = std::hash::DefaultHasher::new();
    //     self.course.id.hash(&mut hasher);
    //     self.content.id.hash(&mut hasher);
    //     let x = hasher.finish();
    //     format!("{x:x}")
    // }
    /// 获取深度（在内容树中的层级）
    pub fn depth(&self) -> usize {
        self.content.depth
    }
    
    /// 获取父节点ID
    pub fn parent_id(&self) -> Option<String> {
        self.content.parent_id.clone()
    }

    /* ---------- 新稳定 ID ---------- */
    /// 稳定 ID：<course_id>::<content_id>
    pub fn id(&self) -> String {
        format!("{}::{}", self.course.id, self.content.id)
    }

    /* ---------- 旧随机哈希（兼容用） ---------- */
    pub fn id_legacy(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.course.id.hash(&mut h);
        self.content.id.hash(&mut h);
        format!("{:x}", h.finish())
    }
    /// 返回作业标题（等价于 CourseAssignment::title）
    pub fn title(&self) -> &str {
        &self.content.title
    }

    async fn _get(&self) -> anyhow::Result<CourseAssignmentData> {
        let dom = self
            .client
            .bb_course_assignment_uploadpage(&self.course.id, &self.content.id)
            .await?;

        let deadline = dom
            .select(&Selector::parse("#assignMeta2 + div").unwrap())
            .next()
            .map(|e| {
                // replace consecutive whitespaces with a single space
                e.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            });

        let attempt = self._get_current_attempt().await?;

        Ok(CourseAssignmentData { deadline, attempt })
    }
    pub async fn get(&self) -> anyhow::Result<CourseAssignment> {
        let data = with_cache(
            &format!(
                "CourseAssignmentHandle::_get_{}_{}",
                self.content.id, self.course.id
            ),
            self.client.cache_ttl(),
            self._get(),
        )
        .await?;

        Ok(CourseAssignment {
            client: self.client.clone(),
            course: self.course.clone(),
            content: self.content.clone(),
            data,
        })
    }

    async fn _get_current_attempt(&self) -> anyhow::Result<Option<String>> {
        let dom = self
            .client
            .bb_course_assignment_viewpage(&self.course.id, &self.content.id)
            .await?;

        let attempt_label = if let Some(e) = dom
            .select(&Selector::parse("h3#currentAttempt_label").unwrap())
            .next()
        {
            e.text().collect::<String>()
        } else {
            return Ok(None);
        };

        let attempt_label = attempt_label
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        Ok(Some(attempt_label))
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct CourseAssignmentData {
    // descriptions: Vec<String>,
    // attachments: Vec<(String, String)>,
    deadline: Option<String>,
    attempt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CourseAssignment {
    client: Client,
    course: Arc<CourseMeta>,
    content: Arc<CourseContentData>,
    data: CourseAssignmentData,
}

impl CourseAssignment {
    pub fn title(&self) -> &str {
        &self.content.title
    }

    pub fn descriptions(&self) -> &[String] {
        &self.content.descriptions
    }

    pub fn attachments(&self) -> &[(String, String)] {
        &self.content.attachments
    }

    pub fn last_attempt(&self) -> Option<&str> {
        self.data.attempt.as_deref()
    }


    pub async fn get_submit_formfields(&self) -> anyhow::Result<HashMap<String, String>> {
        let dom = self
            .client
            .bb_course_assignment_uploadpage(&self.course.id, &self.content.id)
            .await?;

        let extract_field = |input: scraper::ElementRef<'_>| {
            let name = input.value().attr("name")?.to_owned();
            let value = input.value().attr("value")?.to_owned();
            Some((name, value))
        };

        let submitformfields = dom
            .select(&Selector::parse("form#uploadAssignmentFormId input").unwrap())
            .map(extract_field)
            .chain(
                dom.select(&Selector::parse("div.field input").unwrap())
                    .map(extract_field),
            )
            .flatten()
            .collect::<HashMap<_, _>>();

        Ok(submitformfields)
    }

    pub async fn submit_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        log::info!("submitting file: {}", path.display());

        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let content_type = get_mime_type(&ext);
        log::info!("content type: {}", content_type);

        let filename = path
            .file_name()
            .context("file name not found")?
            .to_string_lossy()
            .to_string();

        let map = self.get_submit_formfields().await?;
        log::trace!("map: {:#?}", map);

        macro_rules! add_field_from_map {
            ($body:ident, $name:expr) => {
                let $body = $body.add_field(
                    $name,
                    map.get($name)
                        .with_context(|| format!("field '{}' not found", $name))?
                        .as_bytes(),
                );
            };
        }

        let body = multipart::MultipartBuilder::new();
        add_field_from_map!(body, "attempt_id");
        add_field_from_map!(body, "blackboard.platform.security.NonceUtil.nonce");
        add_field_from_map!(body, "blackboard.platform.security.NonceUtil.nonce.ajax");
        add_field_from_map!(body, "content_id");
        add_field_from_map!(body, "course_id");
        add_field_from_map!(body, "isAjaxSubmit");
        add_field_from_map!(body, "lu_link_id");
        add_field_from_map!(body, "mode");
        add_field_from_map!(body, "recallUrl");
        add_field_from_map!(body, "remove_file_id");
        add_field_from_map!(body, "studentSubmission.text_f");
        add_field_from_map!(body, "studentSubmission.text_w");
        add_field_from_map!(body, "studentSubmission.type");
        add_field_from_map!(body, "student_commentstext_f");
        add_field_from_map!(body, "student_commentstext_w");
        add_field_from_map!(body, "student_commentstype");
        add_field_from_map!(body, "textbox_prefix");
        let body = body
            .add_field("studentSubmission.text", b"")
            .add_field("student_commentstext", b"")
            .add_field("dispatch", b"submit")
            .add_field("newFile_artifactFileId", b"undefined")
            .add_field("newFile_artifactType", b"undefined")
            .add_field("newFile_artifactTypeResourceKey", b"undefined")
            .add_field("newFile_attachmentType", b"L") // not sure
            .add_field("newFile_fileId", b"new")
            .add_field("newFile_linkTitle", filename.as_bytes())
            .add_field("newFilefilePickerLastInput", b"dummyValue")
            .add_file(
                "newFile_LocalFile0",
                &filename,
                content_type,
                std::fs::File::open(path)?,
            )
            .add_field("useless", b"");

        let res = self.client.bb_course_assignment_uploaddata(body).await?;

        if !res.status().is_success() {
            let st = res.status();
            let rbody = res.text().await?;
            if rbody.contains("尝试呈现错误页面时发生严重的内部错误") {
                anyhow::bail!("invalid status {} (caused by unknown server error)", st);
            }

            log::debug!("response: {}", rbody);
            anyhow::bail!("invalid status {}", st);
        }

        Ok(())
    }

    /// Try to parse the deadline string into a NaiveDateTime.
    pub fn deadline(&self) -> Option<chrono::DateTime<chrono::Local>> {
        let d = self.data.deadline.as_deref()?;
        let re = regex::Regex::new(
            r"(\d{4})年(\d{1,2})月(\d{1,2})日 星期. (上午|下午)(\d{1,2}):(\d{1,2})",
        )
        .unwrap();

        if let Some(caps) = re.captures(d) {
            let year: i32 = caps[1].parse().ok()?;
            let month: u32 = caps[2].parse().ok()?;
            let day: u32 = caps[3].parse().ok()?;
            let mut hour: u32 = caps[5].parse().ok()?;
            let minute: u32 = caps[6].parse().ok()?;

            // Adjust for PM times
            if &caps[4] == "下午" && hour < 12 {
                hour += 12;
            }

            // Create NaiveDateTime
            let naive_dt = chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd_opt(year, month, day)?,
                chrono::NaiveTime::from_hms_opt(hour, minute, 0)?,
            );

            let r = chrono::Local.from_local_datetime(&naive_dt).unwrap();

            Some(r)
        } else {
            None
        }
    }

    pub fn deadline_raw(&self) -> Option<&str> {
        self.data.deadline.as_deref()
    }

    pub async fn download_attachment(
        &self,
        uri: &str,
        dest: &std::path::Path,
    ) -> anyhow::Result<()> {
        log::debug!("downloading attachment from https://course.pku.edu.cn{uri}");

        /* ---------- 第 1 次请求 ---------- */
        let mut res = self.client.get_by_uri(uri).await?;

        /* ---------- 如遇 3xx，再跟一次 ---------- */
        if res.status().is_redirection() {
            let loc = res
                .headers()
                .get("location")
                .context("location header not found")?
                .to_str()
                .context("location header not str")?
                .to_owned();
            log::debug!("redirected to https://course.pku.edu.cn{loc}");

            // 跟随一次重定向（教学网只会重定向一次到真实文件）
            res = self.client.get_by_uri(&loc).await?;
        }

        /* ---------- 200 OK：拿到数据 ---------- */
        anyhow::ensure!(res.status().is_success(), "status not success: {}", res.status());
        let body = res.bytes().await?;

        // compio::fs::write 返回 BufResult，仍需用宏展开成 Result
        let r = compio::fs::write(dest, body).await;
        compio::buf::buf_try!(@try r);

        Ok(())
    }
    // pub async fn download_attachment(
    //     &self,
    //     uri: &str,
    //     dest: &std::path::Path,
    // ) -> anyhow::Result<()> {
    //     log::debug!(
    //         "downloading attachment from https://course.pku.edu.cn{}",
    //         uri
    //     );
    //     let res = self.client.get_by_uri(uri).await?;
    //     anyhow::ensure!(
    //         res.status().as_u16() == 302,
    //         "status not 302: {}",
    //         res.status()
    //     );

    //     let loc = res
    //         .headers()
    //         .get("location")
    //         .context("location header not found")?
    //         .to_str()
    //         .context("location header not str")?
    //         .to_owned();

    //     log::debug!("redicted to https://course.pku.edu.cn{}", loc);
    //     let res = self.client.get_by_uri(&loc).await?;
    //     anyhow::ensure!(res.status().is_success(), "status not success");

    //     let rbody = res.bytes().await?;
    //     let r = compio::fs::write(dest, rbody).await;
    //     compio::buf::buf_try!(@try r);
    //     Ok(())
    // }
    /// 旧随机哈希（仅供 CLI 兼容）
    pub fn id_legacy(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.course.id.hash(&mut h);
        self.content.id.hash(&mut h);
        format!("{:x}", h.finish())
    }
}

/*━━━━━━━━━━━━━━━━━━━━━━━  CourseDocument  ━━━━━━━━━━━━━━━━━━━*/

#[derive(Debug, Clone)]
pub struct CourseDocumentHandle {
    pub client: Client,
    pub course: Arc<CourseMeta>,
    pub content: Arc<CourseContentData>,
}

impl CourseDocumentHandle {
    /// 获取深度（在内容树中的层级）
    pub fn depth(&self) -> usize {
        self.content.depth
    }
    
    /// 获取父节点ID
    pub fn parent_id(&self) -> Option<String> {
        self.content.parent_id.clone()
    }
    /// 与 Assignment 相同的稳定 id
    pub fn id(&self) -> String {
        let mut h = std::hash::DefaultHasher::new();
        self.course.id.hash(&mut h);
        self.content.id.hash(&mut h);
        format!("{:x}", h.finish())
    }
    /// 课程内容标题（与 Assignment/Video 的实现保持一致）
    pub fn title(&self) -> &str {
        &self.content.title
    }

    pub async fn get(&self) -> anyhow::Result<CourseDocument> {
        Ok(CourseDocument {
            client: self.client.clone(),
            course: self.course.clone(),
            content: self.content.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CourseDocument {
    pub client: Client,
    pub course: Arc<CourseMeta>,
    pub content: Arc<CourseContentData>,
}

impl CourseDocument {
    /* —— 基本信息 —— */
    pub fn id(&self) -> String            { self.course.id().to_owned() }
    pub fn title(&self) -> &str           { &self.content.title         }
    pub fn descriptions(&self) -> &[String]               { &self.content.descriptions }
    pub fn attachments(&self)  -> &[(String,String)]      { &self.content.attachments  }

    pub async fn download_attachment(
        &self,
        uri: &str,
        dest: &std::path::Path,
    ) -> anyhow::Result<()> {
        log::debug!(
            "downloading attachment from https://course.pku.edu.cn{}",
            uri
        );

        // 第 1 次请求：有时直接 200，有时先 302
        let res = self.client.get_by_uri(uri).await?;
        match res.status().as_u16() {
            /* ---------- ① 302 跳转 ---------- */
            302 => {
                let loc = res
                    .headers()
                    .get("location")
                    .context("location header not found")?
                    .to_str()
                    .context("location header not str")?
                    .to_owned();

                log::debug!("redirected to https://course.pku.edu.cn{}", loc);

                let res2 = self.client.get_by_uri(&loc).await?;
                anyhow::ensure!(res2.status().is_success(), "status not success");

                let body = res2.bytes().await?;
                let r = compio::fs::write(dest, body).await;
                compio::buf::buf_try!(@try r);
            }

            /* ---------- ② 直接 200 OK ---------- */
            200 => {
                let body = res.bytes().await?;
                let r = compio::fs::write(dest, body).await;
                compio::buf::buf_try!(@try r);
            }

            other => anyhow::bail!("unexpected status {}", other),
        }

        log::debug!("attachment saved -> {}", dest.display());
        Ok(())
    }
}

/* ━━━━━━━━━ CourseAnnouncement(Handle) ━━━━━━━━━ */

#[derive(Debug, Clone)]
pub struct CourseAnnouncementMeta {
    id:   String,   // 可用 href 中 annId 或时间戳做稳定 id
    time: String,
    title:String,
    href: String,
}

#[derive(Debug, Clone)]
pub struct CourseAnnouncementHandle {
    client : Client,
    course : Arc<CourseMeta>,
    meta   : Arc<CourseAnnouncementMeta>,
}

impl CourseAnnouncementHandle {
    pub fn id   (&self) -> &str { &self.meta.id   }
    pub fn title(&self) -> &str { &self.meta.title}
    pub fn time (&self) -> &str { &self.meta.time }

    pub async fn get(&self) -> anyhow::Result<CourseAnnouncement> {
        // 真正请求正文页
        let dom = self.client.get_by_uri(&self.meta.href).await?.text().await?;
        // …解析正文、图片、附件…
        Ok(CourseAnnouncement {
            client : self.client.clone(),
            course : self.course.clone(),
            meta   : self.meta.clone(),
            html   : dom.into(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CourseAnnouncement {
    client : Client,
    course : Arc<CourseMeta>,
    meta   : Arc<CourseAnnouncementMeta>,
    html   : String,                      // ↓ 可延迟解析
}
impl CourseAnnouncement {
    /// 原始 HTML 字符串（只读）
    pub fn html_raw(&self) -> &str {
        &self.html
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CourseVideoMeta {
    title: String,
    time: String,
    url: String,
}

impl CourseVideoMeta {
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn time(&self) -> &str {
        &self.time
    }
}

#[derive(Debug, Clone)]
pub struct CourseVideoHandle {
    client: Client,
    meta: Arc<CourseVideoMeta>,
    course: Arc<CourseMeta>,
}

impl CourseVideoHandle {
    /// Course video identifier computed from hash.
    // pub fn id(&self) -> String {
    //     let mut hasher = std::hash::DefaultHasher::new();
    //     self.course.id.hash(&mut hasher);
    //     self.meta.title.hash(&mut hasher);
    //     self.meta.time.hash(&mut hasher);
    //     let x = hasher.finish();
    //     format!("{x:x}")
    // }
    pub fn id(&self) -> String {
        // meta.url 形如 https://...player.html?course_id=_80167_1&sub_id=abc123&app_id=4
        let sub_id = Url::parse(&self.meta.url)
            .ok()                                    // Result → Option
            .and_then(|u| {
                u.query_pairs()
                 .find(|(k, _)| k == "sub_id")       // 找到 sub_id
                 .map(|(_, v)| v.to_string())
            })
            .unwrap_or_default();                    // 若解析失败给空串

        format!("{}::{}", self.course.id, sub_id)
    }

    pub fn id_legacy(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.course.id.hash(&mut h);
        self.meta.title.hash(&mut h);
        self.meta.time.hash(&mut h);
        format!("{:x}", h.finish())
    }
    pub fn title(&self) -> &str { self.meta.title() }
    pub fn time(&self)  -> &str { self.meta.time()  }

    pub fn meta(&self) -> &CourseVideoMeta {
        &self.meta
    }
    async fn get_iframe_url(&self) -> anyhow::Result<String> {
        let res = self.client.get_by_uri(&self.meta.url).await?;
        anyhow::ensure!(res.status().is_success(), "status not success");
        let rbody = res.text().await?;
        let dom = scraper::Html::parse_document(&rbody);
        let iframe = dom
            .select(&Selector::parse("#content iframe").unwrap())
            .next()
            .context("iframe not found")?;
        let src = iframe
            .value()
            .attr("src")
            .context("src not found")?
            .to_owned();

        let res = self.client.get_by_uri(&src).await?;
        anyhow::ensure!(res.status().as_u16() == 302, "status not 302");
        let loc = res
            .headers()
            .get("location")
            .context("location header not found")?
            .to_str()
            .context("location header not str")?
            .to_owned();

        Ok(loc)
    }

    async fn get_sub_info(&self, loc: &str) -> anyhow::Result<serde_json::Value> {
        let qs = qs::Query::from_str(loc).context("parse loc qs failed")?;
        let course_id = qs
            .get("course_id")
            .context("course_id not found")?
            .to_owned();
        let sub_id = qs.get("sub_id").context("sub_id not found")?.to_owned();
        let app_id = qs.get("app_id").context("app_id not found")?.to_owned();
        let auth_data = qs
            .get("auth_data")
            .context("auth_data not found")?
            .to_owned();

        let value = self
            .client
            .bb_course_video_sub_info(&course_id, &sub_id, &app_id, &auth_data)
            .await?;

        Ok(value)
    }

    fn get_m3u8_path(&self, sub_info: serde_json::Value) -> anyhow::Result<String> {
        let sub_content = sub_info
            .as_object()
            .context("sub_info not object")?
            .get("list")
            .context("sub_info.list not found")?
            .as_array()
            .context("sub_info.list not array")?
            .first()
            .context("sub_info.list empty")?
            .as_object()
            .context("sub_info.list[0] not object")?
            .get("sub_content")
            .context("sub_info.list[0].sub_content not found")?
            .as_str()
            .context("sub_info.list[0].sub_content not string")?;

        let sub_content = serde_json::Value::from_str(sub_content)?;

        let save_playback = sub_content
            .as_object()
            .context("sub_content not object")?
            .get("save_playback")
            .context("sub_content.save_playback not found")?
            .as_object()
            .context("sub_content.save_playback not object")?;

        let is_m3u8 = save_playback
            .get("is_m3u8")
            .context("sub_content.save_playback.is_m3u8 not found")?
            .as_str()
            .context("sub_content.save_playback.is_m3u8 not string")?;

        anyhow::ensure!(is_m3u8 == "yes", "not m3u8");

        let url = save_playback
            .get("contents")
            .context("save_playback.contents not found")?
            .as_str()
            .context("save_playback.contents not string")?;

        Ok(url.to_owned())
    }

    async fn get_m3u8_playlist(&self, url: &str) -> anyhow::Result<bytes::Bytes> {
        let res = self.client.get_by_uri(url).await?;
        anyhow::ensure!(res.status().is_success(), "status not success");
        let rbody = res.bytes().await?;
        Ok(rbody)
    }

    async fn _get(&self) -> anyhow::Result<(String, bytes::Bytes)> {
        let loc = self.get_iframe_url().await?;
        let info = self.get_sub_info(&loc).await?;
        let pl_url = self.get_m3u8_path(info)?;
        let pl_raw = self.get_m3u8_playlist(&pl_url).await?;
        Ok((pl_url, pl_raw))
    }

    pub async fn get(&self) -> anyhow::Result<CourseVideo> {
        let (pl_url, pl_raw) = self._get().await.with_context(|| {
            format!(
                "get course video for {} {}",
                self.course.title(),
                self.meta().title()
            )
        })?;

        let pl_raw = pl_raw.to_vec();
        let (_, pl) = m3u8_rs::parse_playlist(&pl_raw)
            .map_err(|e| anyhow::anyhow!("{:#}", e))
            .context("parse m3u8 failed")?;

        match pl {
            m3u8_rs::Playlist::MasterPlaylist(_) => anyhow::bail!("master playlist not supported"),
            m3u8_rs::Playlist::MediaPlaylist(pl) => Ok(CourseVideo {
                client: self.client.clone(),
                course: self.course.clone(),
                meta: self.meta.clone(),
                pl_url: pl_url.into_url().context("parse pl_url failed")?,
                pl_raw: pl_raw.into(),
                pl,
            }),
        }
    }
}

#[derive(Debug)]
pub struct CourseVideo {
    client: Client,
    course: Arc<CourseMeta>,
    meta: Arc<CourseVideoMeta>,
    pl_raw: bytes::Bytes,
    pl_url: url::Url,
    pl: m3u8_rs::MediaPlaylist,
}

impl CourseVideo {
    pub fn course_name(&self) -> &str {
        self.course.name()
    }

    pub fn meta(&self) -> &CourseVideoMeta {
        &self.meta
    }

    pub fn m3u8_raw(&self) -> bytes::Bytes {
        self.pl_raw.clone()
    }

    pub fn len_segments(&self) -> usize {
        self.pl.segments.len()
    }

    /// Refresh the key for the given segment index. You should call this method before getting the segment data referenced by the index.
    ///
    /// The EXT-X-KEY tag specifies how to decrypt them.  It applies to every Media Segment and to every Media
    /// Initialization Section declared by an EXT-X-MAP tag that appears
    /// between it and the next EXT-X-KEY tag in the Playlist file with the
    /// same KEYFORMAT attribute (or the end of the Playlist file).
    pub fn refresh_key<'a>(
        &'a self,
        index: usize,
        key: Option<&'a m3u8_rs::Key>,
    ) -> Option<&'a m3u8_rs::Key> {
        let seg = &self.pl.segments[index];
        fn fallback_keyformat(key: &m3u8_rs::Key) -> &str {
            key.keyformat.as_deref().unwrap_or("identity")
        }

        if let Some(newkey) = &seg.key {
            if key.is_none_or(|k| fallback_keyformat(k) == fallback_keyformat(newkey)) {
                return Some(newkey);
            }
        }
        key
    }

    pub fn segment(&self, index: usize) -> &m3u8_rs::MediaSegment {
        &self.pl.segments[index]
    }

    /// Fetch the segment data for the given index. If `key` is provided, the segment will be decrypted.
    pub async fn get_segment_data<'a>(
        &'a self,
        index: usize,
        key: Option<&'a m3u8_rs::Key>,
    ) -> anyhow::Result<bytes::Bytes> {
        log::info!(
            "downloading segment {}/{} for video {}",
            index,
            self.len_segments(),
            self.meta.title()
        );

        let seg = &self.pl.segments[index];

        // fetch maybe encrypted segment data
        let seg_url: String = self.pl_url.join(&seg.uri).context("join seg url")?.into();
        let mut bytes = with_cache_bytes(
            &format!("CourseVideo::download_segment_{}", seg_url),
            self.client.download_artifact_ttl(),
            self._download_segment(&seg_url),
        )
        .await
        .context("download segment data")?;

        // decrypt it if needed
        if let Some(key) = key {
            // sequence number may be used to construct IV
            let seq = (self.pl.media_sequence as usize + index) as u128;
            bytes = self
                .decrypt_segment(key, bytes, seq)
                .await
                .context("decrypt segment data")?;
        }

        Ok(bytes)
    }

    async fn _download_segment(&self, seg_url: &str) -> anyhow::Result<bytes::Bytes> {
        let res = self.client.get_by_uri(seg_url).await?;
        anyhow::ensure!(res.status().is_success(), "status not success");

        let bytes = res.bytes().await?;
        Ok(bytes)
    }

    async fn get_aes128_key(&self, url: &str) -> anyhow::Result<[u8; 16]> {
        // fetch aes128 key from uri
        let r = with_cache_bytes(
            &format!("CourseVideo::get_aes128_uri_{}", url),
            self.client.download_artifact_ttl(),
            async {
                let r = self.client.get_by_uri(url).await?.bytes().await?;
                Ok(r)
            },
        )
        .await?
        .to_vec();

        if r.len() != 16 {
            anyhow::bail!("key length not 16: {:?}", String::from_utf8(r));
        }

        // convert to array
        let mut key = [0; 16];
        key.copy_from_slice(&r);
        Ok(key)
    }

    async fn decrypt_segment(
        &self,
        key: &m3u8_rs::Key,
        bytes: bytes::Bytes,
        seq: u128,
    ) -> anyhow::Result<bytes::Bytes> {
        use aes::cipher::{
            BlockDecryptMut, KeyIvInit, block_padding::Pkcs7, generic_array::GenericArray,
        };
        // ref: https://datatracker.ietf.org/doc/html/rfc8216#section-4.3.2.4
        match &key.method {
            // An encryption method of AES-128 signals that Media Segments are
            // completely encrypted using [AES_128] with a 128-bit key, Cipher
            // Block Chaining, and PKCS7 padding [RFC5652].  CBC is restarted
            // on each segment boundary, using either the IV attribute value
            // or the Media Sequence Number as the IV; see Section 5.2.  The
            // URI attribute is REQUIRED for this METHOD.
            m3u8_rs::KeyMethod::AES128 => {
                let uri = key.uri.as_ref().context("key uri not found")?;
                let iv = if let Some(iv) = &key.iv {
                    let iv = iv.to_ascii_uppercase();
                    let hx = iv.strip_prefix("0x").context("iv not start with 0x")?;
                    u128::from_str_radix(hx, 16).context("parse iv failed")?
                } else {
                    seq
                }
                .to_be_bytes();

                let aes_key = self.get_aes128_key(uri).await?;

                let aes_key = GenericArray::from(aes_key);
                let iv = GenericArray::from(iv);

                let de = cbc::Decryptor::<aes::Aes128>::new(&aes_key, &iv)
                    .decrypt_padded_vec_mut::<Pkcs7>(&bytes)
                    .context("decrypt failed")?;

                Ok(de.into())
            }
            r => unimplemented!("m3u8 key: {:?}", r),
        }
    }
}

/// 根据文件扩展名返回对应的 MIME 类型
pub fn get_mime_type(extension: &str) -> &str {
    let mime_types: HashMap<&str, &str> = [
        ("html", "text/html"),
        ("htm", "text/html"),
        ("txt", "text/plain"),
        ("csv", "text/csv"),
        ("json", "application/json"),
        ("xml", "application/xml"),
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("bmp", "image/bmp"),
        ("webp", "image/webp"),
        ("mp3", "audio/mpeg"),
        ("wav", "audio/wav"),
        ("mp4", "video/mp4"),
        ("avi", "video/x-msvideo"),
        ("pdf", "application/pdf"),
        ("zip", "application/zip"),
        ("tar", "application/x-tar"),
        ("7z", "application/x-7z-compressed"),
        ("rar", "application/vnd.rar"),
        ("exe", "application/octet-stream"),
        ("bin", "application/octet-stream"),
    ]
    .iter()
    .cloned()
    .collect();

    mime_types
        .get(extension)
        .copied()
        .unwrap_or("application/octet-stream")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mime_type() {
        assert_eq!(get_mime_type("html"), "text/html");
        assert_eq!(get_mime_type("png"), "image/png");
        assert_eq!(get_mime_type("mp3"), "audio/mpeg");
        assert_eq!(get_mime_type("unknown"), "application/octet-stream");
    }
}
