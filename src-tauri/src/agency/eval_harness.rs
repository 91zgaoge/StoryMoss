//! Eval harness：JSON 场景 → 种子 DB + 角色队列 mock → 驱动 coordinator → 断言期望。
//! CI 跑确定性模式（随 cargo test --lib）；real-LLM 模式经 IPC（T9 agency_run_evals(live)）。

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::{
    agency::{
        board::BlackboardService, coordinator::AgencyCoordinator, models::BoardZone,
        repository::AgencyRepository, tool_loop::LoopLlm,
    },
    db::DbPool,
    error::AppError,
};

#[derive(Debug, serde::Deserialize)]
pub struct EvalScenario {
    pub id: String,
    pub description: String,
    pub seed: Seed,
    pub mock_llm: MockQueues,
    pub expect: Expect,
}

#[derive(Debug, serde::Deserialize)]
pub struct Seed {
    pub story: SeedStory,
    #[serde(default)]
    pub characters: Vec<SeedCharacter>,
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub scenes: Vec<SeedScene>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SeedStory {
    pub title: String,
    #[serde(default)]
    pub genre: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SeedCharacter {
    pub name: String,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub goals: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SeedScene {
    pub sequence_number: i32,
    #[serde(default)]
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MockQueues {
    #[serde(default)]
    pub writer: Vec<String>,
    #[serde(default)]
    pub editor: Vec<String>,
    #[serde(default)]
    pub producer: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Expect {
    pub flow: String, // continue | batch | genesis
    #[serde(default)]
    pub chapter: i32, // continue 用（batch 作 start_chapter）
    #[serde(default)]
    pub count: usize, // batch 用
    #[serde(default)]
    pub revised: Option<bool>,
    #[serde(default)]
    pub run_status: Option<String>,
    #[serde(default)]
    pub gate_outcomes: Vec<String>,
    #[serde(default)]
    pub min_gate_items: usize,
    #[serde(default)]
    pub scenes_created: Option<usize>,
}

#[derive(Debug)]
pub struct EvalOutcome {
    pub scenario_id: String,
    pub passed: bool,
    pub details: String,
    pub duration_ms: u64,
}

pub type Baseline = std::collections::HashMap<String, BaselineEntry>;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BaselineEntry {
    pub passed: bool,
}

/// 仓库内置场景目录：<repo>/evals（CARGO_MANIFEST_DIR = src-tauri）。
pub fn evals_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("evals")
}

pub fn load_scenarios(dir: &std::path::Path) -> Result<Vec<EvalScenario>, crate::error::AppError> {
    let mut out = Vec::new();
    let scenarios_dir = dir.join("scenarios");
    let mut entries: Vec<_> = std::fs::read_dir(&scenarios_dir)
        .map_err(|e| {
            crate::error::AppError::from(format!("读场景目录失败 {}: {}", scenarios_dir.display(), e))
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let text = std::fs::read_to_string(entry.path()).map_err(crate::error::AppError::from)?;
        let scenario: EvalScenario = serde_json::from_str(&text)
            .map_err(|e| {
                crate::error::AppError::from(format!("场景解析失败 {}: {}", entry.path().display(), e))
            })?;
        out.push(scenario);
    }
    Ok(out)
}

/// pass@k 的经验估计：results 中至少 1 次通过则为 1（确定性门档用；
/// 概率化估计 1-(1-p)^k 在 real-LLM 多次采样模式（P5）再引入）。
pub fn pass_at_k(results: &[bool], _k: usize) -> f64 {
    if results.iter().any(|r| *r) {
        1.0
    } else {
        0.0
    }
}

/// pass^k：全部通过才为 1（CI 回归门即 pass^1）。
pub fn pass_pow_k(results: &[bool]) -> f64 {
    if results.iter().all(|r| *r) {
        1.0
    } else {
        0.0
    }
}

/// 回归检测：baseline 中曾通过的场景本次失败 → 回归清单；新场景不算回归。
pub fn check_against_baseline(outcomes: &[EvalOutcome], baseline: &Baseline) -> Vec<String> {
    outcomes
        .iter()
        .filter(|o| baseline.get(&o.scenario_id).map(|b| b.passed).unwrap_or(false) && !o.passed)
        .map(|o| format!("回归: {} 曾通过现失败 ({})", o.scenario_id, o.details))
        .collect()
}

/// 按系统提示词路由的队列 mock：与 coordinator 测试模块 RoutingMock 同构
/// （「你是「角色」」前缀路由三队列；精简掉测试专有的 intervals/delay_ms，
/// eval 路径不做并发断言）。自带同构实现而非上提共享，避免为测试工具改动
/// coordinator 主文件。
struct RoutingMock {
    writer: Mutex<VecDeque<String>>,
    editor: Mutex<VecDeque<String>>,
    producer: Mutex<VecDeque<String>>,
}

impl RoutingMock {
    fn from_queues(q: &MockQueues) -> Arc<Self> {
        Arc::new(Self {
            writer: Mutex::new(q.writer.iter().cloned().collect()),
            editor: Mutex::new(q.editor.iter().cloned().collect()),
            producer: Mutex::new(q.producer.iter().cloned().collect()),
        })
    }
}

#[async_trait::async_trait]
impl LoopLlm for RoutingMock {
    async fn complete(
        &self,
        system: &str,
        _u: &str,
        _t: crate::router::TaskType,
        _m: i32,
    ) -> Result<String, AppError> {
        // 按角色标记路由（真实种子提示词与内置回退提示词均以 你是「角色」开头；
        // 不能裸判 "编辑"——writer 提示词中也含「编辑审计」字样）
        let role = if system.contains("你是「编辑审计」") {
            "editor"
        } else if system.contains("你是「主创」") {
            "writer"
        } else {
            "producer"
        };
        let q = match role {
            "editor" => &self.editor,
            "writer" => &self.writer,
            _ => &self.producer,
        };
        q.lock().unwrap().pop_front().ok_or_else(|| {
            AppError::validation_failed(format!("eval mock[{}] exhausted", role), None::<String>)
        })
    }
}

/// 跑一个场景：seed DB → mock → for_test coordinator → 按 expect.flow 驱动 →
/// 断言。任何失败（含 flow 执行错误与断言不符）进 details，不 panic。
pub async fn run_scenario(pool: &DbPool, scenario: &EvalScenario) -> EvalOutcome {
    let start = std::time::Instant::now();
    let result = run_scenario_inner(pool, scenario).await;
    EvalOutcome {
        scenario_id: scenario.id.clone(),
        passed: result.is_ok(),
        details: result.err().unwrap_or_else(|| "ok".to_string()),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

async fn run_scenario_inner(pool: &DbPool, scenario: &EvalScenario) -> Result<(), String> {
    let story_id = seed_db(pool, scenario)?;
    let mock = RoutingMock::from_queues(&scenario.mock_llm);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let run_id = format!("eval-{}", scenario.id);

    // 1) 按 flow 驱动
    let revised_actual: Option<bool> = match scenario.expect.flow.as_str() {
        "continue" => {
            let r = coordinator
                .run_continue(&run_id, &story_id, scenario.expect.chapter)
                .await
                .map_err(|e| format!("run_continue 失败: {}", e))?;
            Some(r.revised)
        }
        "batch" => {
            let r = coordinator
                .run_continue_batch(&run_id, &story_id, scenario.expect.chapter, scenario.expect.count)
                .await
                .map_err(|e| format!("run_continue_batch 失败: {}", e))?;
            Some(r.chapters.iter().any(|c| c.revised))
        }
        other => return Err(format!("不支持的 flow: {}", other)),
    };

    // 2) 断言（失败信息进 details）
    if let Some(expected) = scenario.expect.revised {
        if revised_actual != Some(expected) {
            return Err(format!("revised 期望 {} 实际 {:?}", expected, revised_actual));
        }
    }
    if let Some(status) = &scenario.expect.run_status {
        let run = AgencyRepository::new(pool.clone())
            .get_run(&run_id)
            .map_err(|e| format!("读 run 失败: {}", e))?
            .ok_or_else(|| format!("run 不存在: {}", run_id))?;
        if &run.status != status {
            return Err(format!("run_status 期望 {} 实际 {}", status, run.status));
        }
    }
    let reviews = BlackboardService::new(pool.clone())
        .list_zone(&run_id, BoardZone::Review)
        .map_err(|e| format!("读审查区失败: {}", e))?;
    let gate_items: Vec<_> = reviews.iter().filter(|i| i.item_type == "gate").collect();
    let gate_outcomes: Vec<String> = gate_items
        .iter()
        .map(|i| {
            serde_json::from_str::<serde_json::Value>(&i.content)
                .ok()
                .and_then(|v| v.get("outcome")?.as_str().map(String::from))
                .unwrap_or_else(|| "<unparsed>".to_string())
        })
        .collect();
    if !scenario.expect.gate_outcomes.is_empty() && gate_outcomes != scenario.expect.gate_outcomes {
        return Err(format!(
            "gate_outcomes 期望 {:?} 实际 {:?}",
            scenario.expect.gate_outcomes, gate_outcomes
        ));
    }
    if gate_items.len() < scenario.expect.min_gate_items {
        return Err(format!(
            "gate 条目数 {} 少于期望最少 {}",
            gate_items.len(),
            scenario.expect.min_gate_items
        ));
    }
    if let Some(n) = scenario.expect.scenes_created {
        let scenes = crate::db::repositories::SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .map_err(|e| format!("读场景失败: {}", e))?;
        if scenes.len() != n {
            return Err(format!("scenes_created 期望 {} 实际 {}", n, scenes.len()));
        }
    }
    Ok(())
}

/// 种子落库（复用 T4/T5 测试种子模式：StoryRepository 建书 + characters 直
/// 接 SQL + WorldBuildingRepository/SceneRepository）。
fn seed_db(pool: &DbPool, scenario: &EvalScenario) -> Result<String, String> {
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: scenario.seed.story.title.clone(),
            description: Some(scenario.description.clone()),
            genre: scenario.seed.story.genre.clone(),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .map_err(|e| format!("seed story: {}", e))?;
    if !scenario.seed.characters.is_empty() {
        let conn = pool.get().map_err(|e| format!("pool: {}", e))?;
        for (i, c) in scenario.seed.characters.iter().enumerate() {
            conn.execute(
                "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'agency', 1, '2026-01-01', '2026-01-01')",
                rusqlite::params![
                    format!("eval-char-{}-{}", scenario.id, i),
                    story.id,
                    c.name,
                    c.background,
                    c.personality,
                    c.goals
                ],
            )
            .map_err(|e| format!("seed character {}: {}", c.name, e))?;
        }
    }
    if let Some(concept) = &scenario.seed.world {
        crate::db::repositories::WorldBuildingRepository::new(pool.clone())
            .create(&story.id, concept)
            .map_err(|e| format!("seed world: {}", e))?;
    }
    for s in &scenario.seed.scenes {
        let repo = crate::db::repositories::SceneRepository::new(pool.clone());
        let scene = repo
            .create(&story.id, s.sequence_number, s.title.as_deref())
            .map_err(|e| format!("seed scene: {}", e))?;
        repo.update(
            &scene.id,
            &crate::db::repositories::SceneUpdate {
                content: Some(s.content.clone()),
                ..Default::default()
            },
        )
        .map_err(|e| format!("seed scene content: {}", e))?;
    }
    Ok(story.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_scenarios_from_repo_dir() {
        let dir = evals_dir();
        let scenarios = load_scenarios(&dir).unwrap();
        assert!(scenarios.len() >= 3, "仓库应内置 ≥3 个场景: {}", scenarios.len());
        assert!(scenarios.iter().any(|s| s.id == "gate-pass-basic"));
    }

    #[tokio::test]
    async fn test_run_all_shipped_scenarios_deterministic() {
        // CI 断言：全部内置场景确定性通过（pass^1 回归门）
        let scenarios = load_scenarios(&evals_dir()).unwrap();
        let mut outcomes = Vec::new();
        for scenario in &scenarios {
            let pool = crate::db::create_test_pool().unwrap();
            outcomes.push(run_scenario(&pool, scenario).await);
        }
        let failures: Vec<String> = outcomes
            .iter()
            .filter(|o| !o.passed)
            .map(|o| format!("{}: {}", o.scenario_id, o.details))
            .collect();
        assert!(failures.is_empty(), "eval 场景失败:\n{}", failures.join("\n"));
        // baseline 回归门：曾通过的场景不得转失败
        let baseline_text = std::fs::read_to_string(evals_dir().join("baseline.json")).unwrap();
        let baseline: Baseline = serde_json::from_str(&baseline_text).unwrap();
        let regressions = check_against_baseline(&outcomes, &baseline);
        assert!(regressions.is_empty(), "eval 回归:\n{}", regressions.join("\n"));
    }

    #[test]
    fn test_baseline_regression_detection() {
        let baseline: Baseline = serde_json::from_str(r#"{"gate-pass-basic": {"passed": true}}"#).unwrap();
        let outcomes = vec![
            EvalOutcome { scenario_id: "gate-pass-basic".into(), passed: false, details: "x".into(), duration_ms: 1 },
            EvalOutcome { scenario_id: "new-scenario".into(), passed: true, details: "y".into(), duration_ms: 1 },
        ];
        let regressions = check_against_baseline(&outcomes, &baseline);
        assert_eq!(regressions.len(), 1);
        assert!(regressions[0].contains("gate-pass-basic"));
        // 新场景不算回归
        assert!(!regressions.iter().any(|r| r.contains("new-scenario")));
    }

    #[test]
    fn test_pass_at_k_and_pass_pow_k() {
        assert!((pass_at_k(&[true, false, true], 3) - 1.0).abs() < 0.001); // 3 次至少 1 次过
        assert!((pass_at_k(&[false, false], 3) - 0.0).abs() < 0.001);
        assert!((pass_pow_k(&[true, true, true]) - 1.0).abs() < 0.001);
        assert!((pass_pow_k(&[true, false, true]) - 0.0).abs() < 0.001);
    }
}
