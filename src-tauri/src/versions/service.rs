//! 场景版本服务 - Phase 3.2
//!
//! 提供版本比较、恢复和版本链管理功能

use std::collections::HashMap;

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::db::{
    models::SceneVersion,
    repositories::{SceneRepository, SceneVersionRepository},
    DbPool,
};

/// 版本差异结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDiff {
    pub from_version: i32,
    pub to_version: i32,
    pub content_diff: Option<TextDiff>,
    pub title_changed: bool,
    pub setting_changed: bool,
    pub characters_changed: bool,
    pub dramatic_goal_changed: bool,
    pub word_count_delta: i32,
    pub confidence_delta: f32,
}

/// 文本差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDiff {
    pub added_lines: Vec<String>,
    pub removed_lines: Vec<String>,
    pub unchanged_percentage: f32,
}

/// 版本恢复结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub new_version: SceneVersion,
    pub restored_from_version_id: String,
    pub restored_from_version_number: i32,
}

/// 版本链节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChainNode {
    pub version: SceneVersion,
    pub children: Vec<String>, // 子版本ID列表
    pub depth: i32,            // 在链中的深度
}

/// 场景版本服务
pub struct SceneVersionService {
    pool: DbPool,
}

impl SceneVersionService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 比较两个版本
    pub fn compare_versions(
        &self,
        from_version_id: &str,
        to_version_id: &str,
    ) -> Result<VersionDiff, Box<dyn std::error::Error>> {
        let version_repo = SceneVersionRepository::new(self.pool.clone());

        let from_version = version_repo
            .get_version(from_version_id)?
            .ok_or("Source version not found")?;

        let to_version = version_repo
            .get_version(to_version_id)?
            .ok_or("Target version not found")?;

        // 计算内容差异
        let content_diff = self.compute_text_diff(
            from_version.content.as_deref(),
            to_version.content.as_deref(),
        );

        // 检查各项变更
        let title_changed = from_version.title != to_version.title;
        let setting_changed = from_version.setting_location != to_version.setting_location
            || from_version.setting_time != to_version.setting_time
            || from_version.setting_atmosphere != to_version.setting_atmosphere;
        let characters_changed = from_version.characters_present != to_version.characters_present
            || from_version.character_conflicts != to_version.character_conflicts;
        let dramatic_goal_changed = from_version.dramatic_goal != to_version.dramatic_goal
            || from_version.external_pressure != to_version.external_pressure;

        // 计算字数变化
        let word_count_delta = to_version.word_count - from_version.word_count;

        // 计算置信度变化
        let confidence_delta = match (from_version.confidence_score, to_version.confidence_score) {
            (Some(from), Some(to)) => to - from,
            (None, Some(to)) => to,
            (Some(from), None) => -from,
            (None, None) => 0.0,
        };

        Ok(VersionDiff {
            from_version: from_version.version_number,
            to_version: to_version.version_number,
            content_diff,
            title_changed,
            setting_changed,
            characters_changed,
            dramatic_goal_changed,
            word_count_delta,
            confidence_delta,
        })
    }

    /// 恢复场景到指定版本
    pub fn restore_version(
        &self,
        scene_id: &str,
        version_id: &str,
        restored_by: &str, // "user" | "ai" | "system"
    ) -> Result<RestoreResult, Box<dyn std::error::Error>> {
        let version_repo = SceneVersionRepository::new(self.pool.clone());
        let scene_repo = SceneRepository::new(self.pool.clone());

        // 获取要恢复的版本
        let target_version = version_repo
            .get_version(version_id)?
            .ok_or("Version not found")?;

        // 获取当前场景
        let scene = scene_repo.get_by_id(scene_id)?.ok_or("Scene not found")?;

        // 创建场景更新
        let updates = crate::db::repositories::SceneUpdate {
            title: target_version.title.clone(),
            content: target_version.content.clone(),
            dramatic_goal: target_version.dramatic_goal.clone(),
            external_pressure: target_version.external_pressure.clone(),
            conflict_type: target_version.conflict_type,
            characters_present: Some(target_version.characters_present.clone()),
            character_conflicts: Some(target_version.character_conflicts.clone()),
            setting_location: target_version.setting_location.clone(),
            setting_time: target_version.setting_time.clone(),
            setting_atmosphere: target_version.setting_atmosphere.clone(),
            previous_scene_id: scene.previous_scene_id.clone(),
            next_scene_id: scene.next_scene_id.clone(),
            confidence_score: target_version.confidence_score,
            ..Default::default()
        };

        // 更新场景
        scene_repo.update(scene_id, &updates)?;

        // 创建新版本记录这次恢复操作
        let restored_scene = scene_repo.get_by_id(scene_id)?.ok_or("Scene not found")?;
        let creator_type = match restored_by {
            "user" => crate::db::models::CreatorType::User,
            "ai" => crate::db::models::CreatorType::Ai,
            _ => crate::db::models::CreatorType::System,
        };

        let change_summary = format!(
            "恢复到版本 #{} ({})",
            target_version.version_number, target_version.change_summary
        );

        let new_version = version_repo.create_version(
            &restored_scene,
            &change_summary,
            creator_type,
            None, // model_used
            target_version.confidence_score,
        )?;

        Ok(RestoreResult {
            new_version,
            restored_from_version_id: version_id.to_string(),
            restored_from_version_number: target_version.version_number,
        })
    }

    /// 获取版本链（包含分支结构）
    pub fn get_version_chain(
        &self,
        scene_id: &str,
    ) -> Result<Vec<VersionChainNode>, Box<dyn std::error::Error>> {
        let version_repo = SceneVersionRepository::new(self.pool.clone());
        let versions = version_repo.get_versions(scene_id)?;

        // 构建版本ID到版本的映射
        let version_map: HashMap<String, SceneVersion> =
            versions.iter().map(|v| (v.id.clone(), v.clone())).collect();

        // 构建父子关系
        let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
        for version in &versions {
            if let Some(ref prev_id) = version.previous_version_id {
                children_map
                    .entry(prev_id.clone())
                    .or_default()
                    .push(version.id.clone());
            }
        }

        // 找到根版本（没有previous_version_id的）
        let root_versions: Vec<&SceneVersion> = versions
            .iter()
            .filter(|v| v.previous_version_id.is_none())
            .collect();

        // 构建链
        let mut chain = vec![];
        for root in root_versions {
            self.build_chain_nodes(root, &version_map, &children_map, 0, &mut chain);
        }

        // 按版本号排序
        chain.sort_by_key(|a| a.version.version_number);

        Ok(chain)
    }

    /// 获取版本统计信息
    pub fn get_version_stats(
        &self,
        scene_id: &str,
    ) -> Result<VersionStats, Box<dyn std::error::Error>> {
        let version_repo = SceneVersionRepository::new(self.pool.clone());
        let versions = version_repo.get_versions(scene_id)?;

        let total_versions = versions.len();
        if total_versions == 0 {
            return Ok(VersionStats::default());
        }

        // 计算平均置信度
        let avg_confidence = versions
            .iter()
            .filter_map(|v| v.confidence_score)
            .fold(0.0, |sum, score| sum + score)
            / total_versions as f32;

        // 找到最高置信度版本
        let best_version = versions
            .iter()
            .max_by(|a, b| {
                a.confidence_score
                    .partial_cmp(&b.confidence_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();

        // 按创建者统计
        let mut user_edits = 0;
        let mut ai_edits = 0;
        let mut system_edits = 0;

        for version in &versions {
            match version.created_by {
                crate::db::models::CreatorType::User => user_edits += 1,
                crate::db::models::CreatorType::Ai => ai_edits += 1,
                crate::db::models::CreatorType::System => system_edits += 1,
            }
        }

        // 计算总字数变化
        let total_word_delta = if versions.len() >= 2 {
            let first = versions.iter().min_by_key(|v| v.version_number);
            let last = versions.iter().max_by_key(|v| v.version_number);
            match (first, last) {
                (Some(f), Some(l)) => l.word_count - f.word_count,
                _ => 0,
            }
        } else {
            0
        };

        Ok(VersionStats {
            total_versions: total_versions as i32,
            avg_confidence,
            best_version_id: best_version.as_ref().map(|v| v.id.clone()),
            best_version_number: best_version.as_ref().map(|v| v.version_number),
            user_edits,
            ai_edits,
            system_edits,
            total_word_delta,
            first_version_at: versions.iter().map(|v| v.created_at).min(),
            last_version_at: versions.iter().map(|v| v.created_at).max(),
        })
    }

    /// 查找最近的高质量版本（用于自动恢复建议）
    pub fn find_best_recent_version(
        &self,
        scene_id: &str,
        min_confidence: f32,
    ) -> Result<Option<SceneVersion>, Box<dyn std::error::Error>> {
        let version_repo = SceneVersionRepository::new(self.pool.clone());
        let versions = version_repo.get_versions(scene_id)?;

        // 过滤出满足置信度阈值且不是系统创建的版本
        let best = versions
            .into_iter()
            .filter(|v| {
                v.confidence_score
                    .is_some_and(|score| score >= min_confidence)
                    && !matches!(v.created_by, crate::db::models::CreatorType::System)
            })
            .max_by(|a, b| {
                a.confidence_score
                    .partial_cmp(&b.confidence_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        Ok(best)
    }

    // ============== 私有辅助方法 ==============

    fn compute_text_diff(&self, old: Option<&str>, new: Option<&str>) -> Option<TextDiff> {
        let old = old?;
        let new = new?;

        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let mut added = vec![];
        let mut removed = vec![];

        // 简单的行级diff
        let old_set: std::collections::HashSet<&str> = old_lines.iter().cloned().collect();
        let new_set: std::collections::HashSet<&str> = new_lines.iter().cloned().collect();

        for line in &new_lines {
            if !old_set.contains(line) {
                added.push(line.to_string());
            }
        }

        for line in &old_lines {
            if !new_set.contains(line) {
                removed.push(line.to_string());
            }
        }

        // 计算未变更百分比
        let common_lines = old_lines.len() + new_lines.len() - added.len() - removed.len();
        let unchanged_percentage = if old_lines.is_empty() && new_lines.is_empty() {
            100.0
        } else {
            (common_lines as f32 / (old_lines.len() + new_lines.len()) as f32) * 200.0
        };

        Some(TextDiff {
            added_lines: added,
            removed_lines: removed,
            unchanged_percentage: unchanged_percentage.min(100.0),
        })
    }

    fn build_chain_nodes(
        &self,
        version: &SceneVersion,
        version_map: &HashMap<String, SceneVersion>,
        children_map: &HashMap<String, Vec<String>>,
        depth: i32,
        chain: &mut Vec<VersionChainNode>,
    ) {
        let children = children_map.get(&version.id).cloned().unwrap_or_default();

        chain.push(VersionChainNode {
            version: version.clone(),
            children: children.clone(),
            depth,
        });

        // 递归处理子版本
        for child_id in children {
            if let Some(child) = version_map.get(&child_id) {
                self.build_chain_nodes(child, version_map, children_map, depth + 1, chain);
            }
        }
    }
}

/// 版本统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionStats {
    pub total_versions: i32,
    pub avg_confidence: f32,
    pub best_version_id: Option<String>,
    pub best_version_number: Option<i32>,
    pub user_edits: usize,
    pub ai_edits: usize,
    pub system_edits: usize,
    pub total_word_delta: i32,
    pub first_version_at: Option<chrono::DateTime<Local>>,
    pub last_version_at: Option<chrono::DateTime<Local>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_diff() {
        let service = SceneVersionService {
            pool: crate::db::create_test_pool().unwrap(),
        };

        let old = "Line 1\nLine 2\nLine 3";
        let new = "Line 1\nLine 2 modified\nLine 3\nLine 4";

        let diff = service.compute_text_diff(Some(old), Some(new));
        assert!(diff.is_some());

        let diff = diff.unwrap();
        assert_eq!(diff.added_lines.len(), 2); // "Line 2 modified" 和 "Line 4"
        assert_eq!(diff.removed_lines.len(), 1); // "Line 2"
    }
}
