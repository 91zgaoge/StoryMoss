//! 统一实时状态同步中心
//!
//! 提供跨窗口（幕前↔幕后）状态同步机制。
//! 所有数据修改操作完成后自动发射同步事件，前后台监听并刷新对应数据。
//!
//! ## 使用方式
//!
//! 后端命令中数据修改后调用：
//! ```ignore
//! let _ = crate::state_sync::StateSync::emit_story_updated(&app, &story_id, Some("新标题"));
//! ```
//!
//! 前端通过 `useSyncStore` Hook 监听：
//! ```typescript
//! useSyncStore({
//!   onStoryUpdated: (storyId) => invalidateStories(),
//!   onSceneUpdated: (storyId, sceneId) => invalidateScenes(storyId),
//! });
//! ```

pub mod events;
pub mod service;

pub use service::StateSync;
