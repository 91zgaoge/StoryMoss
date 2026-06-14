//! Preflight - 写前校验
//!
//! 检查合同完整性、大纲结构化、blocking issues

use crate::db::{CharacterRepository, DbPool, SceneRepository, StoryContractRepository};

/// 校验结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreflightResult {
    pub ready: bool,
    pub missing_contracts: Vec<String>,
    pub warnings: Vec<String>,
    pub blocking_issues: Vec<String>,
}

/// 写前校验器
pub struct PreflightChecker;

impl PreflightChecker {
    pub fn new() -> Self {
        Self
    }

    pub async fn check(
        &self,
        pool: &DbPool,
        story_id: &str,
        chapter_number: i32,
    ) -> PreflightResult {
        let pool = pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || Self::check_sync(&pool, &story_id, chapter_number))
            .await
            .unwrap_or_else(|e| PreflightResult {
                ready: false,
                missing_contracts: vec![],
                warnings: vec![format!("预检任务执行失败: {}", e)],
                blocking_issues: vec!["预检无法完成".to_string()],
            })
    }

    fn check_sync(pool: &DbPool, story_id: &str, chapter_number: i32) -> PreflightResult {
        let mut missing_contracts = Vec::new();
        let mut warnings = Vec::new();
        let mut blocking_issues = Vec::new();

        // 1. 检查 MASTER_SETTING 合同
        let contract_repo = StoryContractRepository::new(pool.clone());
        match contract_repo.get_by_story(story_id) {
            Ok(contracts) => {
                let has_master = contracts
                    .iter()
                    .any(|c| c.contract_type == "MASTER_SETTING");
                let has_chapter = contracts.iter().any(|c| {
                    if c.contract_type != "CHAPTER" {
                        return false;
                    }
                    // 从 contract_json 中解析 chapter_number
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&c.contract_json) {
                        json.get("chapter_number")
                            .and_then(|v| v.as_i64())
                            .map(|n| n as i32 == chapter_number)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                });

                if !has_master {
                    missing_contracts.push("MASTER_SETTING".to_string());
                    blocking_issues.push(format!(
                        "故事 [{}] 缺少世界观合同 (MASTER_SETTING)，请先创建世界观设定",
                        story_id
                    ));
                }
                if !has_chapter {
                    missing_contracts.push(format!("CHAPTER_{}", chapter_number));
                    blocking_issues.push(format!(
                        "第 {} 章缺少章节合同，请先创建章节合同",
                        chapter_number
                    ));
                }
            }
            Err(e) => {
                warnings.push(format!("查询合同时出错: {}", e));
            }
        }

        // 2. 检查角色列表是否非空
        let char_repo = CharacterRepository::new(pool.clone());
        match char_repo.get_by_story(story_id) {
            Ok(characters) => {
                if characters.is_empty() {
                    blocking_issues.push("故事中没有角色，请先创建至少一个角色".to_string());
                } else if characters.len() < 2 {
                    warnings.push("故事中角色较少（<2），建议增加角色以丰富互动".to_string());
                }
            }
            Err(e) => {
                warnings.push(format!("查询角色时出错: {}", e));
            }
        }

        // 3. 检查当前 scene 是否有 outline
        let scene_repo = SceneRepository::new(pool.clone());
        match scene_repo.get_by_story(story_id) {
            Ok(scenes) => {
                let scene = scenes.iter().find(|s| s.sequence_number == chapter_number);
                if let Some(s) = scene {
                    let has_outline = s
                        .outline_content
                        .as_ref()
                        .map(|o| !o.trim().is_empty())
                        .unwrap_or(false);
                    if !has_outline {
                        blocking_issues.push(format!(
                            "第 {} 章 (scene_id: {}) 缺少大纲，请先编写场景大纲",
                            chapter_number, s.id
                        ));
                    }
                } else {
                    blocking_issues.push(format!(
                        "第 {} 章的场景不存在，请先创建场景",
                        chapter_number
                    ));
                }
            }
            Err(e) => {
                warnings.push(format!("查询场景时出错: {}", e));
            }
        }

        let ready = blocking_issues.is_empty();

        PreflightResult {
            ready,
            missing_contracts,
            warnings,
            blocking_issues,
        }
    }
}

impl Default for PreflightChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// 轻量预检器 — 仅用于 GenerationMode::TimeSliced（分时模式）
///
/// 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md 模块 4
/// 与完整 `PreflightChecker` 的区别：
/// - 只检查「角色非空」一项（保证 Writer 有基本角色信息可遵循）
/// - 失败直接返回错误，**不触发 auto_contract**（TimeSliced 追求速度，不花 5 次
///   LLM 补合同）
/// - 不检查合同/大纲（那些由后台审计在时间线 2 兜底）
pub struct QuickPreflightChecker;

impl QuickPreflightChecker {
    pub fn new() -> Self {
        Self
    }

    /// 仅检查角色非空。DB 查询用 spawn_blocking 包裹，避免阻塞 tokio worker。
    pub async fn check(pool: &DbPool, story_id: &str) -> PreflightResult {
        let pool = pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || -> PreflightResult {
            let char_repo = CharacterRepository::new(pool);
            match char_repo.get_by_story(&story_id) {
                Ok(characters) => {
                    if characters.is_empty() {
                        PreflightResult {
                            ready: false,
                            missing_contracts: vec![],
                            warnings: vec![],
                            blocking_issues: vec![
                                "故事中没有角色，请先创建至少一个角色后再生成".to_string()
                            ],
                        }
                    } else {
                        PreflightResult {
                            ready: true,
                            missing_contracts: vec![],
                            warnings: vec![],
                            blocking_issues: vec![],
                        }
                    }
                }
                Err(e) => PreflightResult {
                    ready: false,
                    missing_contracts: vec![],
                    warnings: vec![format!("查询角色时出错: {}", e)],
                    blocking_issues: vec!["预检无法完成".to_string()],
                },
            }
        })
        .await
        .unwrap_or_else(|e| PreflightResult {
            ready: false,
            missing_contracts: vec![],
            warnings: vec![format!("预检任务执行失败: {}", e)],
            blocking_issues: vec!["预检无法完成".to_string()],
        })
    }
}

impl Default for QuickPreflightChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn insert_story(pool: &DbPool, story_id: &str) {
        let conn = pool.get().expect("Failed to get connection");
        conn.execute(
            "INSERT INTO stories (id, title, description, created_at, updated_at)
             VALUES (?1, 'Test Story', 'test', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            rusqlite::params![story_id],
        )
        .expect("Failed to insert test story");
    }

    fn insert_character(pool: &DbPool, char_id: &str, story_id: &str, name: &str) {
        let conn = pool.get().expect("Failed to get connection");
        // dynamic_traits 必须给合法 JSON（get_by_story 会 serde_json::from_str 解析它）
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, dynamic_traits, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'bg', '[]', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            rusqlite::params![char_id, story_id, name],
        )
        .expect("Failed to insert test character");
    }

    #[tokio::test]
    async fn quick_check_no_characters_fails() {
        let pool = create_test_pool().expect("Failed to create test pool");
        insert_story(&pool, "story-empty");
        // 不插入任何角色
        let result = QuickPreflightChecker::check(&pool, "story-empty").await;
        assert!(!result.ready, "空角色应当 ready=false");
        assert!(
            result.blocking_issues.iter().any(|i| i.contains("角色")),
            "blocking_issues 应提及角色，实际: {:?}",
            result.blocking_issues
        );
    }

    #[tokio::test]
    async fn quick_check_with_characters_passes() {
        let pool = create_test_pool().expect("Failed to create test pool");
        insert_story(&pool, "story-with-chars");
        insert_character(&pool, "char-1", "story-with-chars", "沈惊鸿");
        let result = QuickPreflightChecker::check(&pool, "story-with-chars").await;
        assert!(
            result.ready,
            "有角色应当 ready=true，blocking_issues: {:?}",
            result.blocking_issues
        );
        assert!(result.blocking_issues.is_empty());
    }

    /// 验证 QuickPreflightChecker 不触发 auto_contract：
    /// 它没有任何 LLM 调用路径，这里通过确认函数纯同步 DB 查询来保证。
    /// （auto_contract 只在 agents/service.rs 的 Full
    /// 路径触发，本函数不调用它。）
    #[tokio::test]
    async fn quick_check_does_not_require_contracts() {
        let pool = create_test_pool().expect("Failed to create test pool");
        insert_story(&pool, "story-no-contract");
        insert_character(&pool, "char-x", "story-no-contract", "林知秋");
        // 故意不创建任何 story_contracts / scenes / outlines
        let result = QuickPreflightChecker::check(&pool, "story-no-contract").await;
        // 即使没有合同/大纲/场景，只要角色存在就通过——这正是 TimeSliced 追求的速度
        assert!(
            result.ready,
            "QuickCheck 应忽略合同/大纲缺失，实际: {:?}",
            result.blocking_issues
        );
        assert!(result.missing_contracts.is_empty(), "QuickCheck 不检查合同");
    }
}
