//! 故事结构健康检查 (v5.3.0)
//!
//! 对已有故事进行逆向分析，检测结构完整性指标：
//! - 伏笔回收率
//! - 角色弧光完整度
//! - 场景冲突多样性
//! - 大纲覆盖率

use serde::{Deserialize, Serialize};
use crate::db::{DbPool, repositories_narrative as repo};
use crate::narrative::elements::*;

/// 单项健康检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub score: f64,              // 0.0 - 100.0
    pub max_score: f64,
    pub status: HealthStatus,
    pub description: String,
    pub detail: Option<String>,
}

/// 健康状态等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Excellent,   // >= 90
    Good,        // >= 70
    Fair,        // >= 50
    Poor,        // < 50
}

impl HealthStatus {
    pub fn from_score(score: f64) -> Self {
        if score >= 90.0 { HealthStatus::Excellent }
        else if score >= 70.0 { HealthStatus::Good }
        else if score >= 50.0 { HealthStatus::Fair }
        else { HealthStatus::Poor }
    }
}

/// 故事结构健康报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub story_id: String,
    pub overall_score: f64,
    pub status: HealthStatus,
    pub checks: Vec<HealthCheck>,
    pub suggestions: Vec<String>,
    pub analyzed_at: String,
}

/// 故事结构健康分析器
pub struct StoryHealthAnalyzer {
    pool: DbPool,
    story_id: String,
}

impl StoryHealthAnalyzer {
    pub fn new(pool: DbPool, story_id: String) -> Self {
        Self { pool, story_id }
    }

    /// 执行完整分析
    pub fn analyze(&self) -> Result<HealthReport, rusqlite::Error> {
        let mut checks = Vec::new();
        let story_id = self.story_id.clone();

        // 加载数据
        let characters = self.load_characters(&story_id)?;
        let scenes = self.load_scenes(&story_id)?;
        let foreshadowings = self.load_foreshadowings(&story_id)?;
        let outline = self.load_outline(&story_id)?;
        let world_building = self.load_world_building(&story_id)?;

        // 1. 伏笔回收率
        let recovery_rate = self.check_foreshadowing_recovery(&foreshadowings);
        checks.push(HealthCheck {
            name: "伏笔回收率".to_string(),
            score: recovery_rate,
            max_score: 100.0,
            status: HealthStatus::from_score(recovery_rate),
            description: "已回收伏笔占总伏笔的比例".to_string(),
            detail: Some(format!(
                "已回收: {}, 待回收: {}, 已放弃: {}",
                foreshadowings.iter().filter(|f| f.status == ForeshadowingStatus::Payoff).count(),
                foreshadowings.iter().filter(|f| f.status == ForeshadowingStatus::Setup).count(),
                foreshadowings.iter().filter(|f| f.status == ForeshadowingStatus::Abandoned).count(),
            )),
        });

        // 2. 角色弧光完整度
        let arc_completeness = self.check_character_arcs(&characters, &scenes);
        checks.push(HealthCheck {
            name: "角色弧光完整度".to_string(),
            score: arc_completeness,
            max_score: 100.0,
            status: HealthStatus::from_score(arc_completeness),
            description: "主要角色在场景中是否有成长和变化".to_string(),
            detail: Some(format!("主要角色数: {}", characters.len())),
        });

        // 3. 场景冲突多样性
        let conflict_diversity = self.check_conflict_diversity(&scenes);
        checks.push(HealthCheck {
            name: "冲突类型多样性".to_string(),
            score: conflict_diversity,
            max_score: 100.0,
            status: HealthStatus::from_score(conflict_diversity),
            description: "场景冲突类型是否丰富多样".to_string(),
            detail: Some(format!("总场景数: {}", scenes.len())),
        });

        // 4. 大纲覆盖率
        let outline_coverage = self.check_outline_coverage(&outline, &scenes);
        checks.push(HealthCheck {
            name: "大纲覆盖率".to_string(),
            score: outline_coverage,
            max_score: 100.0,
            status: HealthStatus::from_score(outline_coverage),
            description: "已有场景覆盖大纲估计场景数的比例".to_string(),
            detail: outline.as_ref().map(|o| format!("估计场景数: {}, 实际场景数: {}", o.total_scenes_estimate, scenes.len())),
        });

        // 5. 世界观完整度
        let world_depth = self.check_world_depth(&world_building);
        checks.push(HealthCheck {
            name: "世界观完整度".to_string(),
            score: world_depth,
            max_score: 100.0,
            status: HealthStatus::from_score(world_depth),
            description: "世界观规则、历史、地点的详细程度".to_string(),
            detail: world_building.as_ref().map(|w| format!("规则数: {}, 关键地点数: {}", w.rules.len(), w.key_locations.len())),
        });

        // 6. 角色关系网络密度
        let relationship_density = self.check_relationship_density(&characters);
        checks.push(HealthCheck {
            name: "角色关系网络密度".to_string(),
            score: relationship_density,
            max_score: 100.0,
            status: HealthStatus::from_score(relationship_density),
            description: "角色之间是否有足够的关系连接".to_string(),
            detail: Some(format!("总角色数: {}", characters.len())),
        });

        let overall = checks.iter().map(|c| c.score).sum::<f64>() / checks.len() as f64;
        let suggestions = self.generate_suggestions(&checks, &characters, &scenes, &foreshadowings);

        Ok(HealthReport {
            story_id,
            overall_score: overall,
            status: HealthStatus::from_score(overall),
            checks,
            suggestions,
            analyzed_at: chrono::Local::now().to_rfc3339(),
        })
    }

    // ==================== 数据加载 ====================

    fn load_characters(&self, story_id: &str) -> Result<Vec<CharacterElement>, rusqlite::Error> {
        let repo = repo::NarrativeCharacterRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
    }

    fn load_scenes(&self, story_id: &str) -> Result<Vec<SceneElement>, rusqlite::Error> {
        let repo = repo::NarrativeSceneRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
    }

    fn load_foreshadowings(&self, story_id: &str) -> Result<Vec<ForeshadowingElement>, rusqlite::Error> {
        // 统一存储层暂无专门的 foreshadowing repo，从 production 表读取
        let conn = self.pool.get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, content, importance, target_act, hint_style,
                    setup_scene_id, payoff_scene_id, status, source
             FROM foreshadowings WHERE story_id = ?1"
        )?;

        let rows = stmt.query_map([story_id], |row| {
            let status_str: String = row.get(8)?;
            let status = match status_str.as_str() {
                "setup" => ForeshadowingStatus::Setup,
                "payoff" => ForeshadowingStatus::Payoff,
                "abandoned" => ForeshadowingStatus::Abandoned,
                _ => ForeshadowingStatus::Pending,
            };

            Ok(ForeshadowingElement {
                id: row.get(0)?,
                story_id: row.get(1)?,
                content: row.get(2)?,
                importance: row.get(3)?,
                target_act: row.get(4)?,
                hint_style: row.get(5)?,
                setup_scene_id: row.get(6)?,
                payoff_scene_id: row.get(7)?,
                status,
                source: ElementSource::UserCreated,
                source_ref_id: None,
            })
        })?;

        rows.collect()
    }

    fn load_outline(&self, story_id: &str) -> Result<Option<OutlineElement>, rusqlite::Error> {
        let conn = self.pool.get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, acts, total_scenes_estimate, source
             FROM narrative_outlines WHERE story_id = ?1"
        )?;

        let mut rows = stmt.query_map([story_id], |row| {
            let acts_json: String = row.get(2)?;
            let acts: Vec<OutlineAct> = serde_json::from_str(&acts_json).unwrap_or_default();

            Ok(OutlineElement {
                id: row.get(0)?,
                story_id: row.get(1)?,
                acts,
                total_scenes_estimate: row.get(3)?,
                source: ElementSource::UserCreated,
                source_ref_id: None,
            })
        })?;

        rows.next().transpose()
    }

    fn load_world_building(&self, story_id: &str) -> Result<Option<WorldBuildingElement>, rusqlite::Error> {
        let repo = repo::NarrativeWorldBuildingRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
    }

    // ==================== 检查逻辑 ====================

    /// 伏笔回收率：已回收 / (已回收 + 已埋设)
    fn check_foreshadowing_recovery(&self, foreshadowings: &[ForeshadowingElement]) -> f64 {
        if foreshadowings.is_empty() {
            return 50.0; // 无伏笔时给中等分（不一定是坏事）
        }
        let payoff_count = foreshadowings.iter()
            .filter(|f| f.status == ForeshadowingStatus::Payoff)
            .count();
        let active_count = foreshadowings.iter()
            .filter(|f| f.status == ForeshadowingStatus::Setup || f.status == ForeshadowingStatus::Payoff)
            .count();
        if active_count == 0 { return 50.0; }
        (payoff_count as f64 / active_count as f64) * 100.0
    }

    /// 角色弧光完整度：检查角色是否在不同场景中有行为/目标变化
    fn check_character_arcs(&self, characters: &[CharacterElement], scenes: &[SceneElement]) -> f64 {
        if characters.is_empty() || scenes.is_empty() {
            return 0.0;
        }

        let main_chars: Vec<_> = characters.iter()
            .filter(|c| c.importance_score >= 7.0 || c.role_type == "protagonist")
            .collect();

        if main_chars.is_empty() {
            return 30.0;
        }

        let mut total_arc_score = 0.0;
        for character in &main_chars {
            let presence_count = scenes.iter()
                .filter(|s| s.characters_present.contains(&character.name))
                .count();
            // 出现越多场景，弧光潜力越大
            let presence_ratio = (presence_count as f64 / scenes.len() as f64).min(1.0);
            // 有目标描述加分
            let has_goals = if character.goals.len() > 5 { 1.0 } else { 0.3 };
            // 有恐惧/弱点描述加分
            let has_fears = if character.fears.len() > 5 { 1.0 } else { 0.5 };

            let arc_score = (presence_ratio * 40.0 + has_goals * 30.0 + has_fears * 30.0).min(100.0);
            total_arc_score += arc_score;
        }

        total_arc_score / main_chars.len() as f64
    }

    /// 冲突类型多样性：统计不同 conflict_type 的数量
    fn check_conflict_diversity(&self, scenes: &[SceneElement]) -> f64 {
        if scenes.is_empty() {
            return 0.0;
        }

        let mut conflict_types = std::collections::HashSet::new();
        for scene in scenes {
            if !scene.conflict_type.is_empty() && scene.conflict_type != "none" {
                conflict_types.insert(scene.conflict_type.clone());
            }
        }

        let type_count = conflict_types.len();
        if type_count >= 4 { 100.0 }
        else if type_count >= 3 { 85.0 }
        else if type_count >= 2 { 70.0 }
        else if type_count >= 1 { 50.0 }
        else { 20.0 }
    }

    /// 大纲覆盖率：实际场景数 / 估计场景数
    fn check_outline_coverage(&self, outline: &Option<OutlineElement>, scenes: &[SceneElement]) -> f64 {
        let estimated = match outline {
            Some(o) if o.total_scenes_estimate > 0 => o.total_scenes_estimate as f64,
            _ => return 50.0, // 无大纲时无法评估
        };
        let actual = scenes.len() as f64;
        let ratio = actual / estimated;
        if ratio >= 1.0 { 100.0 }
        else if ratio >= 0.8 { 90.0 }
        else if ratio >= 0.6 { 75.0 }
        else if ratio >= 0.4 { 60.0 }
        else if ratio >= 0.2 { 40.0 }
        else { 20.0 }
    }

    /// 世界观完整度
    fn check_world_depth(&self, world_building: &Option<WorldBuildingElement>) -> f64 {
        let wb = match world_building {
            Some(w) => w,
            None => return 0.0,
        };

        let mut score = 0.0;
        // 概念描述
        if wb.concept.len() > 20 { score += 20.0; }
        else if wb.concept.len() > 5 { score += 10.0; }

        // 规则数量
        score += (wb.rules.len() as f64 * 15.0).min(30.0);

        // 历史描述
        if wb.history.len() > 30 { score += 20.0; }
        else if wb.history.len() > 10 { score += 10.0; }

        // 关键地点
        score += (wb.key_locations.len() as f64 * 10.0).min(20.0);

        // 力量体系
        if wb.power_system.len() > 10 { score += 10.0; }

        score
    }

    /// 角色关系网络密度
    fn check_relationship_density(&self, characters: &[CharacterElement]) -> f64 {
        if characters.len() < 2 {
            return if characters.len() == 1 { 30.0 } else { 0.0 };
        }

        let total_possible = characters.len() * (characters.len() - 1) / 2;
        let total_actual: usize = characters.iter()
            .map(|c| c.relationships.len())
            .sum();

        // 去重计数（双向关系只算一次）
        let density = if total_possible > 0 {
            (total_actual as f64 / total_possible as f64) * 100.0
        } else {
            0.0
        };

        density.min(100.0)
    }

    // ==================== 建议生成 ====================

    fn generate_suggestions(
        &self,
        checks: &[HealthCheck],
        characters: &[CharacterElement],
        scenes: &[SceneElement],
        foreshadowings: &[ForeshadowingElement],
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        for check in checks {
            if check.score < 70.0 {
                match check.name.as_str() {
                    "伏笔回收率" => {
                        let pending = foreshadowings.iter()
                            .filter(|f| f.status == ForeshadowingStatus::Setup)
                            .count();
                        suggestions.push(format!(
                            "有 {} 个伏笔尚未回收，建议在后续场景中安排回收",
                            pending
                        ));
                    }
                    "角色弧光完整度" => {
                        suggestions.push("为主要角色设置更清晰的目标和内心冲突，让他们在故事中有所成长".to_string());
                        if characters.len() < 3 {
                            suggestions.push("角色数量较少，考虑添加配角来丰富故事层次".to_string());
                        }
                    }
                    "冲突类型多样性" => {
                        suggestions.push("场景冲突类型较为单一，尝试引入人与自我、人与环境等不同层面的冲突".to_string());
                    }
                    "大纲覆盖率" => {
                        suggestions.push(format!(
                            "当前有 {} 个场景，距离大纲目标还有差距，可继续扩展关键情节",
                            scenes.len()
                        ));
                    }
                    "世界观完整度" => {
                        suggestions.push("世界观描述可以更加详细：添加具体规则、历史事件或关键地点".to_string());
                    }
                    "角色关系网络密度" => {
                        suggestions.push("角色之间的关系网较稀疏，建议为角色之间添加更复杂的情感联系".to_string());
                    }
                    _ => {}
                }
            }
        }

        if suggestions.is_empty() {
            suggestions.push("故事结构健康度良好！继续保持创作节奏。".to_string());
        }

        suggestions
    }
}
