//! pku3b_py – 2025-06 重构版
use pyo3::prelude::*;
use std::{cell::RefCell, collections::HashMap, path::PathBuf};
use std::{io::Write, fs, path::Path};   // ← 把 io::Write 补进来

use pku3b::utils;    
use anyhow::Error;
use compio::runtime::Runtime;
use pku3b::api::{
    Blackboard, Client, Course, CourseAssignment, CourseAssignmentHandle, CourseHandle, CourseVideoHandle,CourseVideo,CourseDocumentHandle, CourseDocument,CourseAnnouncement,CourseAnnouncementHandle
};

// ───────────── ① 每线程唯一的 Compio Runtime ─────────────
thread_local! {
    static LOCAL_RT: RefCell<Option<Runtime>> = RefCell::new(None);
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
        let bb = with_rt(|rt| rt.block_on(self.inner.blackboard(&user, &pwd)))
            .map_err(anyhow_to_py)?;
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
        let v = with_rt(|rt| rt.block_on(self.inner.get_courses(true)))
            .map_err(anyhow_to_py)?;
        Ok(v.into_iter().map(|h| PyCourseHandle { handle: h }).collect())
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
            .clone();                               // CourseHandle

        // 进入课程，得到 PyCourse
        let course = with_rt(|rt| rt.block_on(h.get()))
            .map_err(anyhow_to_py)?;
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
    fn entries(&self) -> HashMap<String, String> {
        self.inner.entries().clone()
    }

    // /*—— 作业 ——*/
    // fn list_assignments(&self) -> PyResult<Vec<PyAssignmentHandle>> {
    //     let handles = with_rt(|rt| rt.block_on(self.inner.list_assignments()))
    //         .map_err(anyhow_to_py)?;
    //     Ok(handles.into_iter()
    //         .map(|h| PyAssignmentHandle { handle: h })
    //         .collect())
    // }
    /*—— 视频 ——*/
    #[pyo3(name = "list_videos")]
    fn list_videos(&self) -> PyResult<Vec<PyVideoHandle>> {
        let handles = with_rt(|rt| rt.block_on(self.inner.list_videos()))
            .map_err(anyhow_to_py)?;

        Ok(handles.into_iter()
                  .map(|h| PyVideoHandle { handle: h })
                  .collect())
    }
    // /*—— 文档 ——*/
    // fn list_documents(&self) -> PyResult<Vec<PyDocumentHandle>> {
    //     let handles = with_rt(|rt| rt.block_on(self.inner.list_documents()))
    //         .map_err(anyhow_to_py)?;

    //     Ok(handles
    //         .into_iter()
    //         .map(|h| PyDocumentHandle { handle: h })
    //         .collect())
    // }
    /*—— 公告 ——*/
    fn list_announcements(&self) -> PyResult<Vec<PyAnnouncementHandle>> {
        let v = with_rt(|rt| rt.block_on(self.inner.list_announcements()))
                .map_err(anyhow_to_py)?;
        Ok(v.into_iter().map(|h| PyAnnouncementHandle{ inner:h }).collect())
    }
     /*—— 作业（带层级信息） ——*/
    // fn list_assignments_with_hierarchy(&self) -> PyResult<Vec<PyAssignmentHandle>> {
    //     let (handles, depths, parent_ids) = with_rt(|rt| rt.block_on(
    //         self.inner.list_assignments_with_hierarchy()
    //     ))
    //     .map_err(anyhow_to_py)?;
        
    //     let results = handles.into_iter()
    //         .zip(depths)
    //         .zip(parent_ids)
    //         .map(|((handle, depth), parent_id)| {
    //             PyAssignmentHandle { 
    //                 handle, 
    //                 depth,
    //                 parent_id
    //             }
    //         })
    //         .collect();
            
    //     Ok(results)
    // }
    /// 获取作业及其层级信息
    fn list_assignments_with_hierarchy(&self) -> PyResult<Vec<PyAssignmentHandle>> {
        let handles = with_rt(|rt| rt.block_on(
            self.inner.list_assignments_with_hierarchy()
        ))
        .map_err(anyhow_to_py)?;
        
        Ok(handles.into_iter()
            .map(|h| PyAssignmentHandle { handle: h })
            .collect())
    }
    
    /*—— 文档（带层级信息） ——*/
    // fn list_documents_with_hierarchy(&self) -> PyResult<Vec<PyDocumentHandle>> {
    //     let (handles, depths, parent_ids) = with_rt(|rt| rt.block_on(
    //         self.inner.list_documents_with_hierarchy()
    //     ))
    //     .map_err(anyhow_to_py)?;
        
    //     let results = handles.into_iter()
    //         .zip(depths)
    //         .zip(parent_ids)
    //         .map(|((handle, depth), parent_id)| {
    //             PyDocumentHandle { 
    //                 handle, 
    //                 depth,
    //                 parent_id
    //             }
    //         })
    //         .collect();
            
    //     Ok(results)
    // }
    /*—— 文档（带层级信息） ——*/
    fn list_documents_with_hierarchy(&self) -> PyResult<Vec<PyDocumentHandle>> {
        let result = with_rt(|rt| rt.block_on(
            self.inner.list_documents_with_hierarchy()
        ))
        .map_err(anyhow_to_py)?;
        
        // 提取元组中的各部分
        let (handles, depths, parent_ids) = result;
        
        // 合并结果
        let results = handles.into_iter()
            .zip(depths.into_iter())
            .zip(parent_ids.into_iter())
            .map(|((handle, depth), parent_id)| {
                PyDocumentHandle { 
                    handle, 
                }
            })
            .collect();
            
        Ok(results)
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyAssignmentHandler ━━━━━━━━━━━━━━━━━━━━*/
#[pyclass]
#[derive(Clone)]
pub struct PyAssignmentHandle { 
    handle: CourseAssignmentHandle ,
}

#[pymethods]
impl PyAssignmentHandle {
    #[getter] fn id(&self)    -> String { self.handle.id() }
    #[getter] fn title(&self) -> String { self.handle.title().to_string() }
    // #[getter] fn depth(&self) -> usize { self.depth }
    // #[getter] fn parent_id(&self) -> Option<String> { self.parent_id.clone() }
    #[getter] 
    fn parent_title(&self) -> Option<String> { 
        self.handle.content.parent_title.clone()
    }
    #[getter] 
    fn section_name(&self) -> Option<String> { 
        self.handle.content.section_name.clone()
    }
    

    // fn get(&self) -> PyResult<PyAssignment> {
    //     // let a = with_rt(|rt| rt.block_on(self.handle.get()))
    //     //     .map_err(anyhow_to_py)?;
    //     // Ok(PyAssignment { inner: a })
    //     let a = with_rt(|rt| rt.block_on(self.handle.clone().get()))
    //         .map_err(anyhow_to_py)?;
    //     Ok(PyAssignment { 
    //         inner: a,
    //         depth: self.depth,               // 传递深度信息
    //         parent_id: self.parent_id.clone() // 传递父节点ID
    //     })
    // }
    fn get(&self) -> PyResult<PyAssignment> {
        let assignment = with_rt(|rt| rt.block_on(self.handle.get()))
            .map_err(anyhow_to_py)?;
            
        // 将层级信息传递给完整对象
        Ok(PyAssignment { 
            inner: assignment,
            // depth: self.handle.content.depth,
            // parent_id: self.handle.content.parent_id.clone(),
            parent_title: self.handle.content.parent_title.clone(),
            section_name: self.handle.content.section_name.clone(),
        })
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyAssignment ━━━━━━━━━━━━━━━━━━━━*/

#[pyclass]
pub struct PyAssignment {
    inner: CourseAssignment,
    // depth: usize,
    // parent_id: Option<String>,
    parent_title: Option<String>,
    section_name: Option<String>,
}

#[pymethods]
impl PyAssignment {
    #[getter]
    fn title(&self) -> String {
        self.inner.title().to_string()
    }

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

    fn download_all(&self, dst: String) -> PyResult<()> {
        let mut dst = PathBuf::from(dst);      // ← 改为可变
        for (name, uri) in self.inner.attachments() {
            with_rt(|rt| rt.block_on(self.inner.download_attachment(uri, &dst.join(name))))
                .map_err(anyhow_to_py)?;
        }
        Ok(())
    }

    fn upload(&self, file_path: String) -> PyResult<()> {
        with_rt(|rt| rt.block_on(
            self.inner
                .submit_file(std::path::Path::new(&file_path)),
        ))
        .map_err(anyhow_to_py)
    }

    fn deadline_raw(&self) -> Option<String> {
        self.inner.deadline_raw().map(|s| s.to_string())
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyVideoHandler ━━━━━━━━━━━━━━━━━━━━*/
#[pyclass]
#[derive(Clone)]
pub struct PyVideoHandle { handle: CourseVideoHandle }

#[pymethods]
impl PyVideoHandle {
    #[getter] fn id(&self)    -> String { self.handle.id() }
    #[getter] fn title(&self) -> String { self.handle.title().to_string() }
    #[getter] fn time(&self)  -> String { self.handle.time().to_string() }

    fn get(&self) -> PyResult<PyVideo> {
        let v = with_rt(|rt| rt.block_on(self.handle.get()))
            .map_err(anyhow_to_py)?;
        Ok(PyVideo { inner: v })
    }
}
/*━━━━━━━━━━━━━━━━━━━━━━━ ⑤ PyVideo ━━━━━━━━━━━━━━━━━━━━*/
#[pyclass]
pub struct PyVideo { inner: CourseVideo }

#[pymethods]
impl PyVideo {
    #[getter] fn course(&self) -> String { self.inner.course_name().to_string() }
    #[getter] fn title (&self) -> String { self.inner.meta().title().to_string() }
    #[getter] fn len   (&self) -> usize  { self.inner.len_segments() }

    /// download(dst_dir:str, to_mp4:bool=False) -> str
    #[pyo3(name = "download")]
    fn download(&self, dst: String, to_mp4: Option<bool>) -> PyResult<String> {
        let dst  = PathBuf::from(dst);
        if !dst.exists() { std::fs::create_dir_all(&dst).map_err(|e| anyhow_to_py(e.into()))?; }

        /* ---------- 1. 准备缓存工作目录 ---------- */
        let cache_dir = utils::projectdir()
            .cache_dir()
            .join("video_download")
            .join(self.inner.meta().title());          // stable-id 更好
        std::fs::create_dir_all(&cache_dir).ok();

        /* ---------- 2. 如果缺少片段才下载 ---------- */
        let mut key = None;
        let mut paths = Vec::<PathBuf>::new();
        for i in 0..self.inner.len_segments() {
            key = self.inner.refresh_key(i, key);
            let seg_path = cache_dir.join(format!("{:05}.ts", i));

            if !seg_path.exists() {
                let data = with_rt(|rt| rt.block_on(
                    self.inner.get_segment_data(i, key)
                )).map_err(anyhow_to_py)?;
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
                    "-y", "-hide_banner", "-loglevel", "quiet",
                    "-i", merged.to_str().unwrap(), "-c", "copy",
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
                Ok(md) if md.is_dir()  => sum += dir_size(&entry.path()),
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
    let freed = cache_size_gb()?;         // 先记下大小
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| anyhow_to_py(e.into()))?;
    }
    fs::create_dir_all(&dir).ok();        // 重建空目录
    Ok(freed)
}

/*━━━━━━━━━━━━━━ ⑤ PyDocumentHandle ━━━━━━━━━━━━━*/
#[pyclass]
#[derive(Clone)]
pub struct PyDocumentHandle { 
    handle: CourseDocumentHandle,
    // depth: usize,
    // parent_id: Option<String>,
}

#[pymethods]
impl PyDocumentHandle {
    #[getter] fn id   (&self) -> String { self.handle.id()     }
    #[getter] fn title(&self) -> String { self.handle.title().to_string() }
    // #[getter] fn depth(&self) -> usize { self.depth }
    // #[getter] fn parent_id(&self) -> Option<String> { self.parent_id.clone() }
    
    #[getter] 
    fn parent_title(&self) -> Option<String> { 
        self.handle.content.parent_title.clone()
    }
    
    #[getter] 
    fn section_name(&self) -> Option<String> { 
        self.handle.content.section_name.clone()
    }
    fn get(&self) -> PyResult<PyDocument> {
        // let d = with_rt(|rt| rt.block_on(self.handle.get()))
        //     .map_err(anyhow_to_py)?;
        // Ok(PyDocument { inner: d })
        let d = with_rt(|rt| rt.block_on(self.handle.clone().get()))
            .map_err(anyhow_to_py)?;
        Ok(PyDocument { 
            inner: d,
            // depth: self.depth,                // 传递深度信息
            // parent_id: self.parent_id.clone()  // 传递父节点ID
        })
    }
}

/* ---------- PyDocument (正文 + 附件) ---------- */
#[pyclass]
pub struct PyDocument { 
    inner: CourseDocument ,
    // depth: usize,
    // parent_id: Option<String>,
}

#[pymethods]
impl PyDocument {
    #[getter] fn title(&self) -> String { self.inner.title().to_string() }
    #[getter]fn descriptions(&self) -> Vec<String> { self.inner.descriptions().to_vec() }
    #[getter] 
    fn parent_title(&self) -> Option<String> { 
        self.inner.content.parent_title.clone()
    }
    
    #[getter] 
    fn section_name(&self) -> Option<String> { 
        self.inner.content.section_name.clone()
    }
    fn attachments  (&self) -> Vec<(String,String)> { self.inner.attachments().to_vec() }

    /// download_all(dst_dir:str)
    fn download_all(&self, dst: String) -> PyResult<()> {
        let mut dst = PathBuf::from(dst);      // ← 改为可变
        std::fs::create_dir_all(&dst).ok();
        for (name, uri) in self.inner.attachments() {
            with_rt(|rt| rt.block_on(
                self.inner.download_attachment(uri, &dst.join(name))
            ))
            .map_err(anyhow_to_py)?;
        }
        Ok(())
    }
}

#[pyclass]
pub struct PyAnnouncementHandle { inner: CourseAnnouncementHandle }
#[pymethods] impl PyAnnouncementHandle {
    #[getter] fn id   (&self)->String{ self.inner.id().to_string() }
    #[getter] fn time (&self)->String{ self.inner.time().to_string()}
    #[getter] fn title(&self)->String{ self.inner.title().to_string()}
    fn get(&self) -> PyResult<PyAnnouncement>{
        Ok( PyAnnouncement {
            inner: with_rt(|rt| rt.block_on(self.inner.get())).map_err(anyhow_to_py)?
        })
    }
}
#[pyclass] pub struct PyAnnouncement { inner: CourseAnnouncement }
#[pymethods] impl PyAnnouncement {
    /// 获取通知原始 HTML
    #[pyo3(name = "html")]
    fn html(&self) -> String {
        self.inner.html_raw().to_string()
    }
    // TODO: 提供 markdown/plaintext 渲染、附件下载等
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
    m.add_function(wrap_pyfunction!(cache_size_gb, m)?)?;
    m.add_function(wrap_pyfunction!(cache_clean,   m)?)?;
    Ok(())
}