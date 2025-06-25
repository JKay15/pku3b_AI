//! 课程内容树（初版骨架）
//!
//! 目前只是数据结构 + 若干辅助方法，真正的构建逻辑后续补上。
use super::{
    CourseAnnouncementHandle, CourseAssignmentHandle, CourseDocumentHandle, CourseVideoHandle,
};
/// 结点类型 —— 后续可扩充
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Course,       // 课程根结点
    Entry,        // 左侧导航条
    Folder,       // Blackboard 文件夹
    Document,     // 课件 / 通知等
    Assignment,   // 作业
    Video,        // 回放
    Announcement, // 课程公告
}

// 新增枚举，表示不同类型的内容句柄
#[derive(Debug, Clone)]
pub enum ContentHandle {
    Assignment(CourseAssignmentHandle),
    Document(CourseDocumentHandle),
    Video(CourseVideoHandle),
    Announcement(CourseAnnouncementHandle),
}

/// 一棵树里的一个节点
#[derive(Debug, Clone)]
pub struct CourseTreeNode {
    pub id: String,     // 稳定 ID（课程内唯一）
    pub title: String,  // 展示标题
    pub kind: NodeKind, // 结点类型
    pub content_handle: Option<ContentHandle>,
    pub children: Vec<CourseTreeNode>,
}

impl CourseTreeNode {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn kind(&self) -> NodeKind {
        self.kind
    }
    pub fn children(&self) -> &[CourseTreeNode] {
        &self.children
    }
    pub fn children_mut(&mut self) -> &mut [CourseTreeNode] {
        &mut self.children
    }
    /// 创建空节点（仅 root / placeholder 用）
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        kind: NodeKind,
        content_handle: Option<ContentHandle>, // 新增参数
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            kind,
            content_handle, // 新增字段初始化
            children: Vec::new(),
        }
    }
    pub fn new_basic(id: impl Into<String>, title: impl Into<String>, kind: NodeKind) -> Self {
        Self::new(id, title, kind, None)
    }
    // 新增方法：获取作业句柄
    pub fn as_assignment(&self) -> Option<&CourseAssignmentHandle> {
        match &self.content_handle {
            Some(ContentHandle::Assignment(handle)) => Some(handle),
            _ => None,
        }
    }
    // 新增方法：获取文档句柄
    pub fn as_document(&self) -> Option<&CourseDocumentHandle> {
        match &self.content_handle {
            Some(ContentHandle::Document(handle)) => Some(handle),
            _ => None,
        }
    }
    // 新增方法：获取视频句柄
    pub fn as_video(&self) -> Option<&CourseVideoHandle> {
        match &self.content_handle {
            Some(ContentHandle::Video(handle)) => Some(handle),
            _ => None,
        }
    }

    // 新增方法：获取公告句柄
    pub fn as_announcement(&self) -> Option<&CourseAnnouncementHandle> {
        match &self.content_handle {
            Some(ContentHandle::Announcement(handle)) => Some(handle),
            _ => None,
        }
    }

    /// 添加子节点并返回其可变引用，便于链式操作
    pub fn add_child(&mut self, child: CourseTreeNode) -> &mut CourseTreeNode {
        self.children.push(child);
        self.children.last_mut().unwrap()
    }

    /// 深度优先遍历（只读）
    pub fn dfs<'a>(&'a self) -> impl Iterator<Item = &'a CourseTreeNode> + 'a {
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            for c in node.children.iter().rev() {
                stack.push(c);
            }
            Some(node)
        })
    }

    /// 广度优先遍历（可变）
    pub fn bfs_mut(&mut self) -> impl Iterator<Item = &mut CourseTreeNode> {
        use std::collections::VecDeque;

        // 裸指针队列保证生命周期分离
        let mut queue: VecDeque<*mut CourseTreeNode> = VecDeque::new();
        queue.push_back(self as *mut _);

        std::iter::from_fn(move || {
            let ptr = queue.pop_front()?;
            unsafe {
                // ① 取可变引用
                let node: &mut CourseTreeNode = &mut *ptr;
                // ② 把孩子裸指针继续排队
                for child in node.children.iter_mut() {
                    queue.push_back(child as *mut _);
                }
                // ③ 返回当前结点
                Some(node)
            }
        })
    }
}
