//! Repository 兼容层
//!
//! 所有 Repository 已迁移至 repositories_v3.rs，本文件仅保留重新导出以兼容现有代码。
//! 新代码应直接使用 `crate::db::repositories_v3` 或 `crate::db::StoryRepository`。

pub use super::repositories_v3::{
    StoryRepository,
    CharacterRepository,
    ChapterRepository,
    UserRepository,
    GenesisRunRepository,
};
