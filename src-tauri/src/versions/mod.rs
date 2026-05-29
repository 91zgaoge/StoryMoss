//! 版本管理模块
//!
//! 提供场景版本历史、比较和恢复功能
#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod service;

// 旧版本兼容结构 - 将在未来版本中移除
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterVersion {
    pub id: String,
    pub chapter_id: String,
    pub version_number: i32,
    pub title: Option<String>,
    pub content: Option<String>,
    pub word_count: i32,
    pub created_at: DateTime<Utc>,
    pub change_summary: String,
}

pub struct VersionManager {
    versions: HashMap<String, Vec<ChapterVersion>>,
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            versions: HashMap::new(),
        }
    }

    pub fn create_version(
        &mut self,
        chapter_id: String,
        title: Option<String>,
        content: Option<String>,
        word_count: i32,
        change_summary: String,
    ) -> ChapterVersion {
        let versions = self.versions.entry(chapter_id.clone()).or_default();
        let version_number = versions.len() as i32 + 1;

        let version = ChapterVersion {
            id: uuid::Uuid::new_v4().to_string(),
            chapter_id,
            version_number,
            title,
            content,
            word_count,
            created_at: Utc::now(),
            change_summary,
        };

        versions.push(version.clone());
        version
    }

    pub fn get_versions(&self, chapter_id: &str) -> Vec<&ChapterVersion> {
        self.versions
            .get(chapter_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn rollback_to_version(
        &mut self,
        chapter_id: &str,
        version_id: &str,
    ) -> Result<ChapterVersion, String> {
        let target = self
            .get_version(chapter_id, version_id)
            .cloned()
            .ok_or("Version not found")?;

        let rollback = self.create_version(
            target.chapter_id,
            target.title.clone(),
            target.content.clone(),
            target.word_count,
            format!("Rollback to version {}", target.version_number),
        );

        Ok(rollback)
    }

    fn get_version(&self, chapter_id: &str, version_id: &str) -> Option<&ChapterVersion> {
        self.versions
            .get(chapter_id)
            .and_then(|v| v.iter().find(|x| x.id == version_id))
    }
}
