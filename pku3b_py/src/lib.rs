//! pku3b_py – 2025-06 重构版
use pyo3::prelude::*;
use std::{cell::RefCell, collections::HashMap, path::PathBuf};
use std::{fs, path::Path}; // ← 把 io::Write 补进来

use anyhow::Error;
use compio::runtime::Runtime;
use pku3b::api::{
    Blackboard, Client, ContentHandle, Course, CourseAnnouncement, CourseAnnouncementHandle,
    CourseAssignment, CourseAssignmentHandle, CourseDocument, CourseDocumentHandle, CourseHandle,
    CourseTreeNode, CourseVideo, CourseVideoHandle,
};
use pku3b::utils;

// ───────────── ① 每线程唯一的 Compio Runtime ─────────────
thread_local! {
    static LOCAL_RT: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}
/// 在当前线程取出（或创建）Runtime，并在其中执行闭包
fn with_rt<F, R>(f: F) -> R
where
    F: FnOnce(&Runtime) -> R,
{
    LOCAL_RT.with(|cell| {
        if cell.borrow().is_none() {
            *cell.borrow_mut() = Some(Runtime::new().expect("create compio runtime"));
        }
        f(cell.borrow().as_ref().unwrap())
    })
}
/// anyhow -> PyErr 简化
fn anyhow_to_py(e: Error) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(format!("{e:?}"))
}

/*━━━━━━━━━━━━━━━━━━━━━━━━ ② PyClient ━━━━━━━━━━━━━━━━━━━━━━━*/

#[pyclass]
pub struct PyClient {
    inner: Client,
}

#[pymethods]
impl PyClient {
    #[new]
    fn new() -> Self {
        Self {
            inner: Client::new(None, None),
        }
    }

    fn login_blackboard(&self, user: String, pwd: String) -> PyResult<PyBlackboard> {
        let bb =
            with_rt(|rt| rt.block_on(self.inner.blackboard(&user, &pwd))).map_err(anyhow_to_py)?;
        Ok(PyBlackboard { inner: bb })
    }

    /// 返回当前平台下 pku3b 的缓存目录绝对路径
    #[getter]
    fn cache_dir(&self) -> String {
        utils::projectdir()
            .cache_dir()
            .to_string_lossy()
            .into_owned()
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━━━ ③ PyBlackboard ━━━━━━━━━━━━━━━━━━━*/

#[pyclass]
pub struct PyBlackboard {
    inner: Blackboard,
}

#[pyclass]
#[derive(Clone)]
pub struct PyCourseHandle {
    handle: CourseHandle,
}

#[pymethods]
impl PyCourseHandle {
    #[getter]
    fn title(&self) -> String {
        self.handle.title().to_string()
    }

    #[getter]
    fn id(&self) -> String {
        self.handle.id().to_string()
    }

    /// 拉取完整 Course 对象
    fn get(&self) -> PyResult<PyCourse> {
        let c = with_rt(|rt| rt.block_on(self.handle.get())).map_err(anyhow_to_py)?;
        Ok(PyCourse { inner: c })
    }
}

#[pymethods]
impl PyBlackboard {
    /// 课程句柄列表（轻量，不触发进入课程页面）
    fn list_courses(&self) -> PyResult<Vec<PyCourseHandle>> {
        let v = with_rt(|rt| rt.block_on(self.inner.get_courses(true))).map_err(anyhow_to_py)?;
        Ok(v.into_iter()
            .map(|h| PyCourseHandle { handle: h })
            .collect())
    }
    /// 便捷：按下标直接获取 `PyCourse`
    #[pyo3(name = "course")]
    fn course(&self, index: usize) -> PyResult<PyCourse> {
        // 先拿轻量句柄列表
        let handles = self.list_courses()?;
        let h = handles
            .get(index)
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("index out of range"))?
            .handle
            .clone(); // CourseHandle

        // 进入课程，得到 PyCourse
        let course = with_rt(|rt| rt.block_on(h.get())).map_err(anyhow_to_py)?;
        Ok(PyCourse { inner: course })
    }

    /// **可选**：仅课程标题，给 UI 快速渲染用
    #[allow(dead_code)]
    fn course_titles(&self) -> PyResult<Vec<String>> {
        Ok(self
            .list_courses()?
            .into_iter()
            .map(|h| h.title())
            .collect())
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━━ ④ PyCourse ━━━━━━━━━━━━━━━━━━━━━━━*/

#[pyclass]
pub struct PyCourse {
    inner: Course,
}

#[pymethods]
impl PyCourse {
    #[getter]
    fn title(&self) -> String {
        self.inner.meta().title().to_string()
    }

    /// 左侧菜单 HashMap<title, url>
    #[getter]
    fn entries(&self) -> HashMap<String, String> {
        self.inner.entries().clone()
    }

    /*—— 视频 ——*/
    #[pyo3(name = "list_videos")]
    fn list_videos(&self) -> PyResult<Vec<PyVideoHandle>> {
        let handles = with_rt(|rt| rt.block_on(self.inner.list_videos())).map_err(anyhow_to_py)?;

        Ok(handles
            .into_iter()
            .map(|h| PyVideoHandle { handle: h })
            .collect())
    }
    fn find_videos_by_title(&self, query: String) -> PyResult<Vec<PyVideoHandle>> {
        Ok(self
            .list_videos()?
            .into_iter()
            .filter(|h| h.title().contains(&query))
            .collect())
    }
    /*—— 公告 ——*/
    pub fn list_announcements(&self) -> PyResult<Vec<PyAnnouncementHandle>> {
        let handles =
            with_rt(|rt| rt.block_on(self.inner.list_announcements())).map_err(anyhow_to_py)?;

        let mut py_handles = Vec::new();
        for handle in handles {
            py_handles.push(PyAnnouncementHandle { handle });
        }

        Ok(py_handles)
    }

    fn find_announcements_by_title(&self, query: String) -> PyResult<Vec<PyAnnouncementHandle>> {
        Ok(self
            .list_announcements()?
            .into_iter()
            .filter(|h| h.title().contains(&query))
            .collect())
    }
    /*—— 作业（带层级信息） ——*/
    fn list_assignments(&self) -> PyResult<Vec<PyAssignmentHandle>> {
        let handles = with_rt(|rt| rt.block_on(self.inner.list_assignments_with_hierarchy()))
            .map_err(anyhow_to_py)?;

        Ok(handles
            .into_iter()
            .map(|h| PyAssignmentHandle { handle: h })
            .collect())
    }
    fn find_assignments_by_title(&self, query: String) -> PyResult<Vec<PyAssignmentHandle>> {
        Ok(self
            .list_assignments()?
            .into_iter()
            .filter(|h| h.title().contains(&query))
            .collect())
    }

    /*—— 文档（带层级信息） ——*/
    fn list_documents(&self) -> PyResult<Vec<PyDocumentHandle>> {
        let (handles, _, _) = with_rt(|rt| rt.block_on(self.inner.list_documents_with_hierarchy()))
            .map_err(anyhow_to_py)?;

        Ok(handles
            .into_iter()
            .map(|h| PyDocumentHandle { handle: h })
            .collect())
    }

    fn find_documents_by_title(&self, query: String) -> PyResult<Vec<PyDocumentHandle>> {
        Ok(self
            .list_documents()?
            .into_iter()
            .filter(|h| h.title().contains(&query))
            .collect())
    }
    /// 构建整棵课程内容树并返回根节点
    fn build_tree(&self) -> PyResult<PyCourseTreeNode> {
        let root = with_rt(|rt| rt.block_on(self.inner.build_tree())).map_err(anyhow_to_py)?;

        Ok(PyCourseTreeNode { inner: root })
    }
    // ====== Entry 左侧菜单项 ======
    fn list_entry_titles(&self) -> Vec<String> {
        self.entries().keys().cloned().collect()
    }

    fn list_entry_pairs(&self) -> Vec<(String, String)> {
        self.entries().into_iter().collect()
    }

    fn find_entries_by_title(&self, query: String) -> Vec<(String, String)> {
        self.entries()
            .into_iter()
            .filter(|(k, _)| k.contains(&query))
            .collect()
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyAssignmentHandler ━━━━━━━━━━━━━━━━━━━━*/
#[pyclass]
#[derive(Clone)]
pub struct PyAssignmentHandle {
    handle: CourseAssignmentHandle,
}

#[pymethods]
impl PyAssignmentHandle {
    #[getter]
    fn id(&self) -> String {
        self.handle.id()
    }

    #[getter]
    fn title(&self) -> String {
        self.handle.title().to_string()
    }

    #[getter]
    fn parent_title(&self) -> Option<String> {
        self.handle.content.parent_title.clone()
    }

    #[getter]
    fn section_name(&self) -> Option<String> {
        self.handle.content.section_name.clone()
    }

    fn get(&self) -> PyResult<PyAssignment> {
        let assignment = with_rt(|rt| rt.block_on(self.handle.get())).map_err(anyhow_to_py)?;

        // 将层级信息传递给完整对象
        Ok(PyAssignment {
            inner: assignment,
            parent_title: self.handle.content.parent_title.clone(),
            section_name: self.handle.content.section_name.clone(),
        })
    }
    fn summary(&self) -> String {
        format!(
            "[{}] {} ({:?})",
            self.id(),
            self.title(),
            self.section_name()
        )
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyAssignment ━━━━━━━━━━━━━━━━━━━━*/

#[pyclass]
pub struct PyAssignment {
    inner: CourseAssignment,
    parent_title: Option<String>,
    section_name: Option<String>,
}

#[pymethods]
impl PyAssignment {
    #[getter]
    fn title(&self) -> String {
        self.inner.title().to_string()
    }
    #[getter]
    fn attachments(&self) -> Vec<(String, String)> {
        self.inner.attachments().to_vec()
    }

    #[getter]
    fn descriptions(&self) -> Vec<String> {
        self.inner.descriptions().to_vec()
    }

    #[getter]
    fn parent_title(&self) -> Option<String> {
        self.parent_title.clone()
    }

    #[getter]
    fn section_name(&self) -> Option<String> {
        self.section_name.clone()
    }

    fn download(&self, dst: String) -> PyResult<()> {
        let dst = PathBuf::from(dst);
        for (name, uri) in self.inner.attachments() {
            with_rt(|rt| rt.block_on(self.inner.download_attachment(uri, &dst.join(name))))
                .map_err(anyhow_to_py)?;
        }
        Ok(())
    }

    fn submit_file(&self, file_path: String) -> PyResult<()> {
        with_rt(|rt| rt.block_on(self.inner.submit_file(std::path::Path::new(&file_path))))
            .map_err(anyhow_to_py)
    }

    fn deadline_raw(&self) -> Option<String> {
        self.inner.deadline_raw().map(|s| s.to_string())
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyVideoHandler ━━━━━━━━━━━━━━━━━━━━*/
#[pyclass]
#[derive(Clone)]
pub struct PyVideoHandle {
    handle: CourseVideoHandle,
}

#[pymethods]
impl PyVideoHandle {
    #[getter]
    fn id(&self) -> String {
        self.handle.id()
    }

    #[getter]
    fn title(&self) -> String {
        self.handle.title().to_string()
    }

    #[getter]
    fn time(&self) -> String {
        self.handle.time().to_string()
    }

    fn get(&self) -> PyResult<PyVideo> {
        let v = with_rt(|rt| rt.block_on(self.handle.get())).map_err(anyhow_to_py)?;
        Ok(PyVideo { inner: v })
    }
    fn summary(&self) -> String {
        format!("[{}] {} ({})", self.id(), self.title(), self.time())
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyVideo ━━━━━━━━━━━━━━━━━━━━*/
#[pyclass]
pub struct PyVideo {
    inner: CourseVideo,
}

#[pymethods]
impl PyVideo {
    #[getter]
    fn course(&self) -> String {
        self.inner.course_name().to_string()
    }
    #[getter]
    fn title(&self) -> String {
        self.inner.meta().title().to_string()
    }
    #[getter]
    fn len(&self) -> usize {
        self.inner.len_segments()
    }

    #[pyo3(name = "download")]
    fn download(&self, dst: String, to_mp4: Option<bool>) -> PyResult<String> {
        let dst = PathBuf::from(dst);
        if !dst.exists() {
            std::fs::create_dir_all(&dst).map_err(|e| anyhow_to_py(e.into()))?;
        }

        /* ---------- 1. 准备缓存工作目录 ---------- */
        let cache_dir = utils::projectdir()
            .cache_dir()
            .join("video_download")
            .join(self.inner.meta().title()); // stable-id 更好
        std::fs::create_dir_all(&cache_dir).ok();

        /* ---------- 2. 如果缺少片段才下载 ---------- */
        let mut key = None;
        let mut paths = Vec::<PathBuf>::new();
        for i in 0..self.inner.len_segments() {
            key = self.inner.refresh_key(i, key);
            let seg_path = cache_dir.join(format!("{:05}.ts", i));

            if !seg_path.exists() {
                let data = with_rt(|rt| rt.block_on(self.inner.get_segment_data(i, key)))
                    .map_err(anyhow_to_py)?;
                std::fs::write(&seg_path, data).map_err(|e| anyhow_to_py(e.into()))?;
            }
            paths.push(seg_path);
        }

        /* ---------- 3. 合并 ---------- */
        let merged = cache_dir.join("merged.ts");
        {
            let mut out = std::fs::File::create(&merged).map_err(|e| anyhow_to_py(e.into()))?;
            for p in &paths {
                let data = std::fs::read(p).map_err(|e| anyhow_to_py(e.into()))?;
                std::io::Write::write_all(&mut out, &data).map_err(|e| anyhow_to_py(e.into()))?;
            }
        }

        // ---------- 4. （可选）转 mp4 ----------
        let need_mp4 = to_mp4.unwrap_or(false);

        let final_path = if need_mp4 {
            // 保持现有：课程标题.mp4
            let mp4 = dst.join(format!("{}.mp4", self.inner.meta().title()));
            let status = std::process::Command::new("ffmpeg")
                .args([
                    "-y",
                    "-hide_banner",
                    "-loglevel",
                    "quiet",
                    "-i",
                    merged.to_str().unwrap(),
                    "-c",
                    "copy",
                ])
                .arg(&mp4)
                .status()
                .map_err(|e| anyhow_to_py(e.into()))?;

            if !status.success() {
                return Err(anyhow_to_py(anyhow::anyhow!("ffmpeg failed")));
            }
            mp4
        } else {
            // 仅 TS 模式：课程标题.ts
            let ts = dst.join(format!("{}.ts", self.inner.meta().title()));
            std::fs::copy(&merged, &ts).map_err(|e| anyhow_to_py(e.into()))?;
            ts
        };

        Ok(final_path.to_string_lossy().into_owned())
    }
}

// ─────────────  递归计算目录大小 (同步)  ─────────────
fn dir_size(p: &Path) -> u64 {
    let mut sum = 0u64;
    if let Ok(rd) = fs::read_dir(p) {
        for entry in rd.flatten() {
            match entry.metadata() {
                Ok(md) if md.is_file() => sum += md.len(),
                Ok(md) if md.is_dir() => sum += dir_size(&entry.path()),
                _ => {}
            }
        }
    }
    sum
}

#[pyfunction]
fn cache_size_gb() -> PyResult<f64> {
    let dir = utils::projectdir().cache_dir().to_path_buf();
    let bytes = dir_size(&dir);
    Ok(bytes as f64 / 1024_f64.powi(3))
}

#[pyfunction]
fn cache_clean() -> PyResult<f64> {
    let dir = utils::projectdir().cache_dir().to_path_buf();
    let freed = cache_size_gb()?; // 先记下大小
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| anyhow_to_py(e.into()))?;
    }
    fs::create_dir_all(&dir).ok(); // 重建空目录
    Ok(freed)
}

/*━━━━━━━━━━━━━━ ⑤ PyDocumentHandle ━━━━━━━━━━━━━*/
#[pyclass]
#[derive(Clone)]
pub struct PyDocumentHandle {
    handle: CourseDocumentHandle,
}

#[pymethods]
impl PyDocumentHandle {
    #[getter]
    fn id(&self) -> String {
        self.handle.id()
    }

    #[getter]
    fn title(&self) -> String {
        self.handle.title().to_string()
    }

    #[getter]
    fn parent_title(&self) -> Option<String> {
        self.handle.content.parent_title.clone()
    }

    #[getter]
    fn section_name(&self) -> Option<String> {
        self.handle.content.section_name.clone()
    }
    fn get(&self) -> PyResult<PyDocument> {
        let d = with_rt(|rt| rt.block_on(self.handle.clone().get())).map_err(anyhow_to_py)?;
        Ok(PyDocument { inner: d })
    }
    fn summary(&self) -> String {
        format!(
            "[{}] {} ({:?})",
            self.id(),
            self.title(),
            self.section_name()
        )
    }
}

/* ---------- PyDocument (正文 + 附件) ---------- */
#[pyclass]
pub struct PyDocument {
    inner: CourseDocument,
}

#[pymethods]
impl PyDocument {
    #[getter]
    fn title(&self) -> String {
        self.inner.title().to_string()
    }
    #[getter]
    fn descriptions(&self) -> Vec<String> {
        self.inner.descriptions().to_vec()
    }
    #[getter]
    fn parent_title(&self) -> Option<String> {
        self.inner.content.parent_title.clone()
    }

    #[getter]
    fn section_name(&self) -> Option<String> {
        self.inner.content.section_name.clone()
    }
    #[getter]
    fn attachments(&self) -> Vec<(String, String)> {
        self.inner.attachments().to_vec()
    }

    /// download_all(dst_dir:str)
    fn download(&self, dst: String) -> PyResult<()> {
        let dst = PathBuf::from(dst); // ← 改为可变
        std::fs::create_dir_all(&dst).ok();
        for (name, uri) in self.inner.attachments() {
            with_rt(|rt| rt.block_on(self.inner.download_attachment(uri, &dst.join(name))))
                .map_err(anyhow_to_py)?;
        }
        Ok(())
    }
}
#[pyclass]
#[derive(Clone)]
pub struct PyAnnouncementHandle {
    pub handle: CourseAnnouncementHandle,
}

#[pymethods]
impl PyAnnouncementHandle {
    #[getter]
    fn id(&self) -> String {
        self.handle.id()
    }

    #[getter]
    fn title(&self) -> String {
        self.handle.title().to_string()
    }

    #[getter]
    fn parent_title(&self) -> Option<String> {
        self.handle.content.parent_title.clone()
    }

    #[getter]
    fn section_name(&self) -> Option<String> {
        self.handle.content.section_name.clone()
    }

    fn get(&self) -> PyResult<PyAnnouncement> {
        let a = with_rt(|rt| rt.block_on(self.handle.clone().get())).map_err(anyhow_to_py)?;
        Ok(PyAnnouncement { inner: a })
    }

    fn summary(&self) -> String {
        format!(
            "[{}] {} ({:?})",
            self.id(),
            self.title(),
            self.section_name()
        )
    }
}
#[pyclass]
pub struct PyAnnouncement {
    pub inner: CourseAnnouncement,
}

#[pymethods]
impl PyAnnouncement {
    #[getter]
    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    #[getter]
    fn descriptions(&self) -> Vec<String> {
        self.inner.descriptions().to_vec()
    }

    #[getter]
    fn parent_title(&self) -> Option<String> {
        self.inner.content.parent_title.clone()
    }

    #[getter]
    fn section_name(&self) -> Option<String> {
        self.inner.content.section_name.clone()
    }

    #[getter]
    fn attachments(&self) -> Vec<(String, String)> {
        self.inner.attachments().to_vec()
    }

    fn download(&self, dst: String) -> PyResult<()> {
        let dst = PathBuf::from(dst);
        std::fs::create_dir_all(&dst).ok();
        for (name, uri) in self.inner.attachments() {
            with_rt(|rt| rt.block_on(self.inner.download_attachment(uri, &dst.join(name))))
                .map_err(anyhow_to_py)?;
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("<Announcement {}>", self.inner.title())
    }
}
// ========== 末尾放在其他类之后 ==========
#[pyclass]
pub struct PyCourseTreeNode {
    inner: CourseTreeNode,
}

#[pymethods]
impl PyCourseTreeNode {
    #[getter]
    fn id(&self) -> String {
        self.inner.id().to_string()
    }
    #[getter]
    fn title(&self) -> String {
        self.inner.title().to_string()
    }
    #[getter]
    fn kind(&self) -> String {
        format!("{:?}", self.inner.kind())
    }

    /// 子节点列表
    #[getter]
    fn children(&self) -> Vec<PyCourseTreeNode> {
        self.inner
            .children()
            .iter()
            .cloned()
            .map(|c| PyCourseTreeNode { inner: c })
            .collect()
    }

    /// 获取文档句柄（如果节点是文档类型）
    fn get_document_handle(&self) -> Option<PyDocumentHandle> {
        if let Some(ContentHandle::Document(handle)) = &self.inner.content_handle {
            Some(PyDocumentHandle {
                handle: handle.clone(),
            })
        } else {
            None
        }
    }

    /// 获取作业句柄（如果节点是作业类型）
    fn get_assignment_handle(&self) -> Option<PyAssignmentHandle> {
        if let Some(ContentHandle::Assignment(handle)) = &self.inner.content_handle {
            Some(PyAssignmentHandle {
                handle: handle.clone(),
            })
        } else {
            None
        }
    }

    /// 获取视频句柄（如果节点是视频类型）
    fn get_video_handle(&self) -> Option<PyVideoHandle> {
        if let Some(ContentHandle::Video(handle)) = &self.inner.content_handle {
            Some(PyVideoHandle {
                handle: handle.clone(),
            })
        } else {
            None
        }
    }

    /// 获取公告句柄（如果节点是公告类型）
    fn get_announcement_handle(&self) -> Option<PyAnnouncementHandle> {
        if let Some(ContentHandle::Announcement(handle)) = &self.inner.content_handle {
            Some(PyAnnouncementHandle {
                handle: handle.clone(),
            })
        } else {
            None
        }
    }

    /// 递归查找节点（根据标题或ID）
    fn find(&self, query: &str) -> Option<PyCourseTreeNode> {
        // 检查当前节点是否匹配
        if self.id() == query || self.title() == query {
            return Some(PyCourseTreeNode {
                inner: self.inner.clone(),
            });
        }

        // 递归检查子节点
        for child in self.children() {
            if let Some(found) = child.find(query) {
                return Some(found);
            }
        }

        None
    }

    /// 按类型查找节点
    fn find_by_kind(&self, kind: &str) -> Vec<PyCourseTreeNode> {
        let mut results = Vec::new();
        let current_kind = self.kind();

        // 检查当前节点是否匹配
        if current_kind == kind {
            results.push(PyCourseTreeNode {
                inner: self.inner.clone(),
            });
        }

        // 递归检查子节点
        for child in self.children() {
            results.extend(child.find_by_kind(kind));
        }

        results
    }

    /// 以文本形式打印整棵子树
    fn summary_tree(&self) -> String {
        fn dfs(node: &PyCourseTreeNode, indent: &str, last: bool, out: &mut String) {
            let icon = match node.kind().as_str() {
                "Document" => "📄",
                "Assignment" => "📝",
                "Video" => "📺",
                "Announcement" => "📢",
                "Folder" => "📁",
                _ => "📦",
            };
            let branch = if last { "└── " } else { "├── " };
            out.push_str(&format!("{indent}{branch}{icon} {}\n", node.title()));

            let next_indent = if last {
                format!("{indent}    ")
            } else {
                format!("{indent}│   ")
            };
            let children = node.children();
            for (i, child) in children.iter().enumerate() {
                dfs(child, &next_indent, i == children.len() - 1, out);
            }
        }

        let mut result = String::new();
        dfs(self, "", true, &mut result);
        result
    }

    fn __repr__(&self) -> String {
        format!("<Node {} {:?}>", self.inner.title(), self.inner.kind())
    }
}

/*━━━━━━━━━━━━━━━━━━━━━━━ ⑥ Python 模块注册 ━━━━━━━━━━━━━━━━*/

#[pymodule]
fn pku3b_py(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<PyClient>()?;
    m.add_class::<PyBlackboard>()?;
    m.add_class::<PyCourseHandle>()?;
    m.add_class::<PyCourse>()?;
    m.add_class::<PyAssignment>()?;
    m.add_class::<PyAssignmentHandle>()?;
    m.add_class::<PyVideoHandle>()?;
    m.add_class::<PyVideo>()?;
    m.add_class::<PyDocumentHandle>()?;
    m.add_class::<PyDocument>()?;
    m.add_class::<PyAnnouncementHandle>()?;
    m.add_class::<PyAnnouncement>()?;
    m.add_class::<PyCourseTreeNode>()?;
    m.add_function(wrap_pyfunction!(cache_size_gb, m)?)?;
    m.add_function(wrap_pyfunction!(cache_clean, m)?)?;
    Ok(())
}
