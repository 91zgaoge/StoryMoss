//! Story Continuity Engine - 故事连续性引擎
//!
//! 追踪角色状态、检测一致性冲突、管理时间线。
//! 在幕后运行，为 Agent 提供连续性保障。

use std::collections::HashMap;

use crate::db::{
    models::{EntityType, RelationType},
    repositories::{CharacterRepository, KnowledgeGraphRepository, SceneRepository},
    DbPool,
};

/// 角色当前状态
#[derive(Debug, Clone)]
pub struct CharacterState {
    pub character_id: String,
    pub name: String,
    pub current_location: Option<String>,
    pub current_emotion: Option<String>,
    pub active_goal: Option<String>,
    pub secrets_known: Vec<String>,
    pub secrets_unknown: Vec<String>,
    pub arc_progress: f32, // 0.0 - 1.0
}

/// 连续性检查结果
#[derive(Debug, Clone)]
pub struct ConsistencyCheck {
    pub is_valid: bool,
    pub issues: Vec<ConsistencyIssue>,
}

/// 一致性问题
#[derive(Debug, Clone)]
pub struct ConsistencyIssue {
    pub issue_type: IssueType,
    pub severity: Severity,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IssueType {
    CharacterLocation,
    CharacterEmotion,
    TimelineConflict,
    WorldRuleViolation,
    RelationshipInconsistency,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// 连续性引擎
pub struct ContinuityEngine {
    pool: DbPool,
}

impl ContinuityEngine {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 检查场景的连续性
    pub fn check_scene_continuity(
        &self,
        story_id: &str,
        scene_id: &str,
        proposed_content: &str,
    ) -> Result<ConsistencyCheck, String> {
        let mut issues = Vec::new();

        // 1. 检查角色位置一致性
        if let Ok(scene_issues) =
            self.check_character_locations(story_id, scene_id, proposed_content)
        {
            issues.extend(scene_issues);
        }

        // 2. 检查世界观规则一致性
        if let Ok(rule_issues) = self.check_world_rules(story_id, proposed_content) {
            issues.extend(rule_issues);
        }

        // 3. 检查时间线一致性
        if let Ok(timeline_issues) = self.check_timeline(story_id, scene_id) {
            issues.extend(timeline_issues);
        }

        // 4. 检查角色情绪连续性
        if let Ok(emotion_issues) =
            self.check_character_emotions(story_id, scene_id, proposed_content)
        {
            issues.extend(emotion_issues);
        }

        // 5. 检查关系一致性
        if let Ok(rel_issues) = self.check_relationships(story_id, proposed_content) {
            issues.extend(rel_issues);
        }

        let is_valid = !issues.iter().any(|i| i.severity == Severity::Critical);

        Ok(ConsistencyCheck { is_valid, issues })
    }

    /// 获取角色的当前状态
    pub fn get_character_states(&self, story_id: &str) -> Result<Vec<CharacterState>, String> {
        let char_repo = CharacterRepository::new(self.pool.clone());
        let characters = char_repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取角色失败: {}", e))?;

        // 一次性查询知识图谱，构建 name -> entity 映射，避免循环内重复查询
        let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
        let entities = kg_repo
            .get_entities_by_story(story_id)
            .map_err(|e| format!("获取实体失败: {}", e))?;

        let entity_map: HashMap<String, crate::db::models::Entity> = entities
            .into_iter()
            .filter(|e| matches!(e.entity_type, EntityType::Character))
            .map(|e| (e.name.clone(), e))
            .collect();

        let mut states = Vec::new();
        for c in characters {
            let character_entity = entity_map.get(&c.name);

            let (location, emotion, goal) = if let Some(entity) = character_entity {
                let attrs = &entity.attributes;
                (
                    attrs
                        .get("current_location")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    attrs
                        .get("current_emotion")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    attrs
                        .get("active_goal")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                )
            } else {
                (None, None, None)
            };

            states.push(CharacterState {
                character_id: c.id,
                name: c.name,
                current_location: location,
                current_emotion: emotion,
                active_goal: goal,
                secrets_known: vec![],
                secrets_unknown: vec![],
                arc_progress: 0.0,
            });
        }

        Ok(states)
    }

    // ==================== 私有检查方法 ====================

    fn check_character_locations(
        &self,
        story_id: &str,
        scene_id: &str,
        content: &str,
    ) -> Result<Vec<ConsistencyIssue>, String> {
        let mut issues = Vec::new();

        let scene_repo = SceneRepository::new(self.pool.clone());
        let current_scene = scene_repo
            .get_by_id(scene_id)
            .map_err(|e| format!("获取场景失败: {}", e))?
            .ok_or("场景不存在")?;

        let scene_location = current_scene.setting_location.clone().unwrap_or_default();

        // 获取角色状态
        let states = self.get_character_states(story_id)?;

        for state in &states {
            // 如果角色在当前场景中出场，但内容中提到了他在其他地方
            if current_scene.characters_present.contains(&state.name) {
                if let Some(ref last_location) = state.current_location {
                    if !scene_location.is_empty() && last_location != &scene_location {
                        // 这是一个潜在的一致性问题（但不一定是错误，角色可以移动）
                        issues.push(ConsistencyIssue {
                            issue_type: IssueType::CharacterLocation,
                            severity: Severity::Info,
                            message: format!(
                                "{} 从 '{}' 移动到了 '{}'。请确保移动过程合理。",
                                state.name, last_location, scene_location
                            ),
                            suggestion: Some(format!(
                                "考虑在场景中描述 {} 如何到达 {}",
                                state.name, scene_location
                            )),
                        });
                    }
                }
            }
        }

        // 增强：检测生成内容中是否提到了角色在 setting_location 以外的位置
        if !scene_location.is_empty() && !content.is_empty() {
            let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
            let entities = kg_repo
                .get_entities_by_story(story_id)
                .map_err(|e| format!("获取实体失败: {}", e))?;

            let location_names: Vec<&str> = entities
                .iter()
                .filter(|e| matches!(e.entity_type, EntityType::Location))
                .map(|e| e.name.as_str())
                .filter(|name| !name.is_empty() && *name != scene_location)
                .collect();

            for state in &states {
                if !current_scene.characters_present.contains(&state.name) {
                    continue;
                }
                if !content.contains(&state.name) {
                    continue;
                }

                for loc in &location_names {
                    if content.contains(loc) {
                        issues.push(ConsistencyIssue {
                            issue_type: IssueType::CharacterLocation,
                            severity: Severity::Info,
                            message: format!(
                                "内容中提到了 {} 在 '{}'，但当前场景设定为 \
                                 '{}'。请确认位置是否合理。",
                                state.name, loc, scene_location
                            ),
                            suggestion: Some(format!(
                                "请检查 {} 是否确实出现在 '{}'，或考虑更新场景设定",
                                state.name, loc
                            )),
                        });
                        break;
                    }
                }
            }
        }

        Ok(issues)
    }

    fn check_world_rules(
        &self,
        story_id: &str,
        content: &str,
    ) -> Result<Vec<ConsistencyIssue>, String> {
        let mut issues = Vec::new();

        let wb_repo = crate::db::repositories::WorldBuildingRepository::new(self.pool.clone());
        let world_building = match wb_repo.get_by_story(story_id) {
            Ok(Some(wb)) => wb,
            _ => return Ok(issues),
        };

        let negation_words = ["不能", "禁止", "不得", "无法", "没有", "不许", "不可", "勿"];

        for rule in world_building.rules {
            if let Some(ref desc) = rule.description {
                let clauses: Vec<&str> = desc
                    .split(|c| c == '，' || c == '。' || c == ';' || c == '、')
                    .collect();
                for clause in clauses {
                    let trimmed = clause.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    // 只处理包含否定词的子句（禁止性规则）
                    if let Some(&neg_word) = negation_words.iter().find(|&&w| trimmed.contains(w)) {
                        // 提取禁止行为：否定词之后的文本
                        let forbidden = if let Some(pos) = trimmed.find(neg_word) {
                            let after = &trimmed[pos + neg_word.len()..];
                            after.trim().trim_start_matches(|c: char| {
                                c == '了' || c == '的' || c == '地' || c == '着'
                            })
                        } else {
                            trimmed
                        };

                        // 禁止行为关键词需要有一定长度才具有辨识度
                        if forbidden.len() > 2 && content.contains(forbidden) {
                            issues.push(ConsistencyIssue {
                                issue_type: IssueType::WorldRuleViolation,
                                severity: Severity::Warning,
                                message: format!(
                                    "可能违反世界观规则「{}」: 内容中出现了禁止行为 '{}'（规则: \
                                     {}）",
                                    rule.name, forbidden, desc
                                ),
                                suggestion: Some(format!(
                                    "请检查 '{}' 是否符合规则 '{}': {}",
                                    forbidden, rule.name, desc
                                )),
                            });
                        }
                    }
                }
            }
        }

        Ok(issues)
    }

    fn check_timeline(
        &self,
        story_id: &str,
        scene_id: &str,
    ) -> Result<Vec<ConsistencyIssue>, String> {
        let mut issues = Vec::new();

        let scene_repo = SceneRepository::new(self.pool.clone());
        let mut scenes = scene_repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取场景失败: {}", e))?;
        scenes.sort_by_key(|s| s.sequence_number);

        let current_idx = scenes.iter().position(|s| s.id == scene_id);
        let current_scene = match current_idx {
            Some(idx) => &scenes[idx],
            None => return Ok(issues),
        };

        // 查找前一场景
        let previous_scene =
            current_idx.and_then(|idx| if idx > 0 { scenes.get(idx - 1) } else { None });

        if let Some(prev) = previous_scene {
            // 检查 sequence_number 顺序
            if prev.sequence_number >= current_scene.sequence_number {
                issues.push(ConsistencyIssue {
                    issue_type: IssueType::TimelineConflict,
                    severity: Severity::Critical,
                    message: format!(
                        "时间线倒错：场景 '{}' (序号 {}) 排在 '{}' (序号 {}) \
                         之后，但序号更小或相等",
                        current_scene.title.as_deref().unwrap_or("未命名"),
                        current_scene.sequence_number,
                        prev.title.as_deref().unwrap_or("未命名"),
                        prev.sequence_number
                    ),
                    suggestion: Some("请检查场景顺序，确保序号按时间线递增".to_string()),
                });
            }

            // 检查 setting_time 一致性（简单启发式）
            if let Some(ref curr_time) = current_scene.setting_time {
                if let Some(ref prev_time) = prev.setting_time {
                    let curr_num = extract_time_number(curr_time);
                    let prev_num = extract_time_number(prev_time);

                    if let (Some(c), Some(p)) = (curr_num, prev_num) {
                        if c < p && !curr_time.contains("回忆") && !curr_time.contains("闪回") {
                            issues.push(ConsistencyIssue {
                                issue_type: IssueType::TimelineConflict,
                                severity: Severity::Warning,
                                message: format!(
                                    "时间可能倒错：当前场景设定为 '{}'，但前一场景为 \
                                     '{}'，数字线索暗示时间倒退",
                                    curr_time, prev_time
                                ),
                                suggestion: Some("如非闪回/回忆场景，请检查时间设定".to_string()),
                            });
                        }
                    }
                }
            }
        }

        Ok(issues)
    }

    fn check_character_emotions(
        &self,
        story_id: &str,
        _scene_id: &str,
        content: &str,
    ) -> Result<Vec<ConsistencyIssue>, String> {
        let mut issues = Vec::new();

        let states = self.get_character_states(story_id)?;
        let content_lower = content.to_lowercase();

        for state in &states {
            if !content.contains(&state.name) {
                continue;
            }

            let last_emotion = state.current_emotion.as_deref().unwrap_or("");
            if last_emotion.is_empty() {
                continue;
            }

            // 检测剧烈情绪突变（无过渡）
            if is_drastic_emotion_change(last_emotion, &content_lower)
                && !has_transition_words(content)
            {
                issues.push(ConsistencyIssue {
                    issue_type: IssueType::CharacterEmotion,
                    severity: Severity::Warning,
                    message: format!(
                        "{} 的情绪可能变化过于剧烈：从「{}」变为内容中的对立情绪，且缺少过渡描写",
                        state.name, last_emotion
                    ),
                    suggestion: Some(format!("建议在 {} 的情绪转变前添加铺垫或过渡", state.name)),
                });
            }
        }

        Ok(issues)
    }

    fn check_relationships(
        &self,
        story_id: &str,
        content: &str,
    ) -> Result<Vec<ConsistencyIssue>, String> {
        let mut issues = Vec::new();

        let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
        let relations = kg_repo
            .get_relations_by_story(story_id)
            .map_err(|e| format!("获取关系失败: {}", e))?;

        let entities = kg_repo
            .get_entities_by_story(story_id)
            .map_err(|e| format!("获取实体失败: {}", e))?;

        for relation in &relations {
            let source_name = entities
                .iter()
                .find(|e| e.id == relation.source_id)
                .map(|e| e.name.as_str())
                .unwrap_or("");
            let target_name = entities
                .iter()
                .find(|e| e.id == relation.target_id)
                .map(|e| e.name.as_str())
                .unwrap_or("");

            if source_name.is_empty() || target_name.is_empty() {
                continue;
            }

            // 只检查内容中同时出现双方角色名的关系
            if !content.contains(source_name) || !content.contains(target_name) {
                continue;
            }

            let relation_str = relation.relation_type.to_string();

            match relation.relation_type {
                // 敌对关系但内容出现亲密/合作描写
                RelationType::Enemy | RelationType::Rival => {
                    let cooperation_keywords =
                        ["合作", "亲密", "拥抱", "信任", "携手", "友爱", "友好"];
                    if cooperation_keywords.iter().any(|&k| content.contains(k)) {
                        issues.push(ConsistencyIssue {
                            issue_type: IssueType::RelationshipInconsistency,
                            severity: Severity::Warning,
                            message: format!(
                                "关系不一致：{} 与 {} 的关系为「{}」，但内容中出现亲密/合作描写",
                                source_name, target_name, relation_str
                            ),
                            suggestion: Some(format!(
                                "请确认 {} 与 {} 的互动是否符合「{}」设定",
                                source_name, target_name, relation_str
                            )),
                        });
                    }
                }
                // 友好/亲密关系但内容出现敌对/冲突描写
                RelationType::Friend | RelationType::Ally | RelationType::Lover => {
                    let hostility_keywords =
                        ["敌对", "仇恨", "厮杀", "背叛", "攻击", "杀害", "决斗"];
                    if hostility_keywords.iter().any(|&k| content.contains(k)) {
                        issues.push(ConsistencyIssue {
                            issue_type: IssueType::RelationshipInconsistency,
                            severity: Severity::Warning,
                            message: format!(
                                "关系不一致：{} 与 {} 的关系为「{}」，但内容中出现敌对/冲突描写",
                                source_name, target_name, relation_str
                            ),
                            suggestion: Some(format!(
                                "请确认 {} 与 {} 的互动是否符合「{}」设定，或考虑更新关系",
                                source_name, target_name, relation_str
                            )),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(issues)
    }
}

// ==================== 辅助函数 ====================

/// 从时间描述字符串中提取首个数字，用于简单的时间比较
fn extract_time_number(time_str: &str) -> Option<i32> {
    let digits: String = time_str.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// 检测是否发生剧烈情绪变化（无过渡）
fn is_drastic_emotion_change(last_emotion: &str, content: &str) -> bool {
    let last_lower = last_emotion.to_lowercase();

    let positive = [
        "喜悦", "高兴", "开心", "兴奋", "快乐", "幸福", "满足", "欣喜", "愉悦",
    ];
    let negative = [
        "悲伤", "痛苦", "绝望", "哀痛", "伤心", "难过", "忧郁", "沮丧",
    ];
    let angry = ["愤怒", "暴怒", "恼火", "气愤", "憎恨"];
    let calm = ["平静", "冷静", "沉着", "淡然", "安宁", "安详"];
    let fearful = ["恐惧", "害怕", "惊恐", "畏惧", "胆寒"];
    let brave = ["勇敢", "镇定", "无畏", "坚毅", "果敢"];

    let last_is_positive = positive.iter().any(|&w| last_lower.contains(w));
    let last_is_negative = negative.iter().any(|&w| last_lower.contains(w));
    let last_is_angry = angry.iter().any(|&w| last_lower.contains(w));
    let last_is_calm = calm.iter().any(|&w| last_lower.contains(w));
    let last_is_fearful = fearful.iter().any(|&w| last_lower.contains(w));
    let last_is_brave = brave.iter().any(|&w| last_lower.contains(w));

    if last_is_positive && negative.iter().any(|&w| content.contains(w)) {
        return true;
    }
    if last_is_negative && positive.iter().any(|&w| content.contains(w)) {
        return true;
    }
    if last_is_angry && calm.iter().any(|&w| content.contains(w)) {
        return true;
    }
    if last_is_calm && angry.iter().any(|&w| content.contains(w)) {
        return true;
    }
    if last_is_fearful && brave.iter().any(|&w| content.contains(w)) {
        return true;
    }
    if last_is_brave && fearful.iter().any(|&w| content.contains(w)) {
        return true;
    }

    false
}

/// 检查内容中是否包含情绪过渡词
fn has_transition_words(content: &str) -> bool {
    let transition = ["渐渐", "慢慢", "逐渐", "转变", "变化"];
    transition.iter().any(|&w| content.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistency_check_creation() {
        let check = ConsistencyCheck {
            is_valid: true,
            issues: vec![],
        };
        assert!(check.is_valid);
    }
}
