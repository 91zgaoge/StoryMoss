use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::agency::board::BlackboardService;
use crate::agency::models::*;
use crate::agency::repository::AgencyRepository;
use crate::agency::roles::spec_for;
use crate::agency::tool_loop::{LoopLlm, ToolLoop};
use crate::agency::tools::{ToolContext, ToolRegistry};
use crate::db::dto::CreateStoryRequest;
use crate::db::repositories::{SceneRepository, SceneUpdate, StoryRepository};
use crate::db::DbPool;
use crate::error::AppError;
use crate::llm::LlmService;
use crate::router::TaskType;

pub const EVENT_RUN_PROGRESS: &str = "agency-run-progress";
/// P1 串行：至多 1 轮修订（第二轮审查后无论结果放行）。
const MAX_REVISION_PASSES: usize = 1;

// ---- 取消注册表（镜像 narrative/pipeline.rs 模式） ----

static AGENCY_CANCEL_FLAGS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_agency_cancel(run_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut flags = AGENCY_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    flags.insert(run_id.to_string(), flag.clone());
    flag
}

pub fn cancel_agency_run(run_id: &str) -> bool {
    let flags = AGENCY_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(flag) = flags.get(run_id) {
        flag.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

pub fn unregister_agency_cancel(run_id: &str) {
    let mut flags = AGENCY_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    flags.remove(run_id);
}

// ---- LoopLlm 生产实现：全部 LLM 调用经 LlmService（路由/健康/成本落表保留） ----

pub struct AgencyLlm {
    llm: LlmService,
}

impl AgencyLlm {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { llm: LlmService::new(app_handle) }
    }
}

#[async_trait::async_trait]
impl LoopLlm for AgencyLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        let (_request_id, result) = self.llm
            .generate_for_task_with_system_prompt(
                task,
                user_prompt.to_string(),
                Some(max_tokens),
                None,
                Some("agency"),
                Some(system_prompt.to_string()),
                None,
            )
            .await;
        result.map(|r| r.content)
    }
}

// ---- 结果类型 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorVerdict {
    pub verdict: String, // pass | revise
    #[serde(default)]
    pub blocking_issues: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub comments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgencyGenesisResult {
    pub run_id: String,
    pub story_id: String,
    pub scene_id: String,
    pub revised: bool,
    pub verdict: EditorVerdict,
    pub chapter_chars: usize,
}

#[derive(Debug, Deserialize)]
struct ConceptOut {
    title: Option<String>,
    genre: Option<String>,
}

/// 宽容 JSON 提取：截取首个 '{' 与末个 '}' 之间解析。
pub(crate) fn parse_lenient<T: for<'de> Deserialize<'de>>(raw: &str) -> Option<T> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&raw[start..=end]).ok()
}

// ---- 协调器 ----

pub struct AgencyCoordinator {
    app_handle: Option<AppHandle>,
    pool: DbPool,
    llm: Arc<dyn LoopLlm>,
}

impl AgencyCoordinator {
    pub fn new(app_handle: AppHandle, pool: DbPool) -> Self {
        let llm: Arc<dyn LoopLlm> = Arc::new(AgencyLlm::new(app_handle.clone()));
        Self { app_handle: Some(app_handle), pool, llm }
    }

    /// 测试/无界面环境构造：不发 Tauri 事件。
    pub fn for_test(pool: DbPool, llm: Arc<dyn LoopLlm>) -> Self {
        Self { app_handle: None, pool, llm }
    }

    /// 创世 2.0 串行端到端：concept → assets(producer) → writing(writer)
    /// → review(editor) → [revision ≤1] → assembly(Scene 装配)。
    pub async fn run_genesis(&self, run_id: &str, premise: &str) -> Result<AgencyGenesisResult, AppError> {
        let repo = AgencyRepository::new(self.pool.clone());
        let cancel = register_agency_cancel(run_id);
        let result = self.run_genesis_inner(run_id, premise, &repo, &cancel).await;
        unregister_agency_cancel(run_id);
        match &result {
            Ok(r) => {
                let json = serde_json::to_string(r).unwrap_or_default();
                let _ = repo.finish_run(run_id, "completed", Some(&json), None);
                self.emit_progress(run_id, "assembly", "completed", "创世完成");
            }
            Err(e) => {
                let status = if cancel.load(Ordering::SeqCst) { "cancelled" } else { "failed" };
                let _ = repo.finish_run(run_id, status, None, Some(&e.to_string()));
                self.emit_progress(run_id, "assembly", status, &e.to_string());
            }
        }
        result
    }

    async fn run_genesis_inner(
        &self,
        run_id: &str,
        premise: &str,
        repo: &AgencyRepository,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AgencyGenesisResult, AppError> {
        repo.create_run(&AgencyRun::new(run_id, premise)).map_err(AppError::from)?;
        repo.update_run_phase(run_id, "running", "concept").map_err(AppError::from)?;
        self.emit_progress(run_id, "concept", "running", "正在构思故事概念");

        // 1) 概念：标题与类型
        let concept_raw = self.llm.complete(
            "你是小说策划。只输出 JSON。",
            &format!("故事前提：{}\n\n输出 JSON：{{\"title\":\"书名\",\"genre\":\"类型\",\"logline\":\"一句话简介\"}}", premise),
            TaskType::Brainstorming,
            1024,
        ).await?;
        let concept: Option<ConceptOut> = parse_lenient(&concept_raw);
        let title = concept.as_ref().and_then(|c| c.title.clone())
            .unwrap_or_else(|| premise.chars().take(12).collect::<String>());
        let genre = concept.as_ref().and_then(|c| c.genre.clone());

        // 2) 建故事
        let pool = self.pool.clone();
        let title_c = title.clone();
        let genre_c = genre.clone();
        let premise_c = premise.to_string();
        let story = tokio::task::spawn_blocking(move || {
            StoryRepository::new(pool).create(CreateStoryRequest {
                title: title_c,
                description: Some(premise_c),
                genre: genre_c,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
        }).await.map_err(|e| AppError::from(format!("create story join error: {}", e)))?
            .map_err(AppError::from)?;
        let story_id = story.id.clone();
        repo.set_run_story(run_id, &story_id).map_err(AppError::from)?;
        self.check_cancel(cancel)?;

        // 3) 管理：资产生产
        repo.update_run_phase(run_id, "running", "assets").map_err(AppError::from)?;
        self.emit_progress(run_id, "assets", "running", "管理 Agent 正在生产创作资产");
        let board = self.board();
        let registry = Arc::new(ToolRegistry::agency_default());
        let producer_out = self.run_role(
            AgentRole::Producer, &board, &registry, run_id, &story_id, premise,
            "请为本故事生产创世资产：世界观、至少 2 张角色卡（真名/欲望/阻力）、第一卷大纲、伏笔清单。逐条写入资产区。",
        ).await.map_err(|e| AppError::from(format!("管理 Agent 阶段失败: {}", e)))?;
        if producer_out.aborted {
            return Err(AppError::from("管理 Agent 被熔断，资产生产未完成"));
        }
        self.check_cancel(cancel)?;

        // 4) 主创：首章写作
        repo.update_run_phase(run_id, "running", "writing").map_err(AppError::from)?;
        self.emit_progress(run_id, "writing", "running", "主创 Agent 正在写作第一章");
        let writer_out = self.run_role(
            AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise,
            "基于资产区创作第一章正文（1500-2500 字）。先用 board_read 读资产，再用 board_write 把完整正文写入 draft 区（item_type=chapter, key=第一章）。",
        ).await.map_err(|e| AppError::from(format!("主创 Agent 阶段失败: {}", e)))?;
        if writer_out.aborted {
            return Err(AppError::from("主创 Agent 被熔断，首章未完成"));
        }
        let mut draft = self.latest_draft(&board, run_id)?;
        self.check_cancel(cancel)?;

        // 5) 编辑审计 + 至多 MAX_REVISION_PASSES 轮修订
        let mut revised = false;
        let mut revision_passes = 0usize;
        let verdict = loop {
            repo.update_run_phase(run_id, "running", "review").map_err(AppError::from)?;
            self.emit_progress(run_id, "review", "running", "编辑审计 Agent 正在审查草稿");
            let editor_out = self.run_role(
                AgentRole::EditorAuditor, &board, &registry, run_id, &story_id, premise,
                &format!("审查 draft 区的最新章节草稿（当前版本：{}）。按系统提示词出具裁决 JSON。", draft.key),
            ).await.map_err(|e| AppError::from(format!("编辑审计 Agent 阶段失败: {}", e)))?;
            if editor_out.aborted {
                return Err(AppError::from("编辑审计 Agent 被熔断，审查未完成"));
            }
            let verdict: EditorVerdict = parse_lenient(&editor_out.output).unwrap_or(EditorVerdict {
                verdict: "pass".to_string(),
                blocking_issues: vec![],
                suggestions: vec![],
                comments: format!("（裁决解析失败，默认放行）原文：{}", editor_out.output.chars().take(200).collect::<String>()),
            });
            // 裁决落审查区（编辑审计为审查区 owner，active）
            let summary = format!("{}：{}", verdict.verdict, verdict.comments.chars().take(60).collect::<String>());
            board.write(run_id, &story_id, AgentRole::EditorAuditor, BoardZone::Review,
                "verdict", &format!("{}-v{}", draft.key, draft.version),
                &editor_out.output, &summary)?;
            if verdict.verdict == "revise" && !verdict.blocking_issues.is_empty() && revision_passes < MAX_REVISION_PASSES {
                revision_passes += 1;
                revised = true;
                repo.update_run_phase(run_id, "running", "revision").map_err(AppError::from)?;
                self.emit_progress(run_id, "revision", "running", "主创 Agent 正在按审查意见修订");
                let issues = verdict.blocking_issues.join("；");
                let revise_out = self.run_role(
                    AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise,
                    &format!("修订「{}」。审查阻断问题：{}。先 board_read 读草稿与资产，再把修订后的完整正文用 board_write 写入 draft 区（同 key）。", draft.key, issues),
                ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
                if revise_out.aborted {
                    return Err(AppError::from("主创 Agent 修订轮被熔断"));
                }
                draft = self.latest_draft(&board, run_id)?;
                self.check_cancel(cancel)?;
                continue; // 修订后再审一次（P1 第二轮无论结果都放行）
            }
            break verdict;
        };

        // 6) 装配：草稿 → Scene 真源（统一输出装配器 P1 形态）
        repo.update_run_phase(run_id, "running", "assembly").map_err(AppError::from)?;
        self.emit_progress(run_id, "assembly", "running", "正在装配正式稿");
        let pool = self.pool.clone();
        let sid = story_id.clone();
        let content = draft.content.clone();
        let scene = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
            let repo = SceneRepository::new(pool);
            let scene = repo.create(&sid, 1, Some("第一章")).map_err(AppError::from)?;
            repo.update(&scene.id, &SceneUpdate {
                content: Some(content),
                ..Default::default()
            }).map_err(AppError::from)?;
            Ok(scene)
        }).await.map_err(|e| AppError::from(format!("scene assembly join error: {}", e)))??;

        Ok(AgencyGenesisResult {
            run_id: run_id.to_string(),
            story_id,
            scene_id: scene.id,
            revised,
            verdict,
            chapter_chars: draft.content.chars().count(),
        })
    }

    async fn run_role(
        &self,
        role: AgentRole,
        board: &BlackboardService,
        registry: &Arc<ToolRegistry>,
        run_id: &str,
        story_id: &str,
        premise: &str,
        task: &str,
    ) -> Result<crate::agency::tool_loop::LoopResult, AppError> {
        let spec = spec_for(role);
        let system_prompt = self.resolve_role_prompt(spec.prompt_id, premise);
        let ctx = ToolContext {
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            role,
            board: board.clone(),
            pool: self.pool.clone(),
        };
        ToolLoop::new(self.llm.clone(), registry.clone())
            .with_max_turns(spec.max_turns)
            .run(role, &ctx, &system_prompt, task)
            .await
    }

    fn latest_draft(&self, board: &BlackboardService, run_id: &str) -> Result<BoardItem, AppError> {
        let drafts = board.list_zone(run_id, BoardZone::Draft)?;
        drafts.into_iter().last()
            .filter(|d| !d.content.is_empty())
            .ok_or_else(|| AppError::from("草稿区为空：主创未产出正文"))
    }

    fn board(&self) -> BlackboardService {
        match &self.app_handle {
            Some(app) => BlackboardService::with_events(self.pool.clone(), app),
            None => BlackboardService::new(self.pool.clone()),
        }
    }

    /// 角色系统提示词：优先 PromptRegistry（支持用户覆盖），注册表不可用时回退内置短提示。
    fn resolve_role_prompt(&self, prompt_id: &str, premise: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("premise".to_string(), premise.to_string());
        let resolved = crate::prompts::registry::resolve_prompt_with_vars(&self.pool, prompt_id, &vars);
        resolved.unwrap_or_else(|_| format!("{}\n\n当前故事前提：{}", default_role_prompt(prompt_id), premise))
    }

    fn check_cancel(&self, cancel: &Arc<AtomicBool>) -> Result<(), AppError> {
        if cancel.load(Ordering::SeqCst) {
            Err(AppError::from("创世已取消"))
        } else {
            Ok(())
        }
    }

    fn emit_progress(&self, run_id: &str, phase: &str, status: &str, message: &str) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(EVENT_RUN_PROGRESS, serde_json::json!({
                "run_id": run_id,
                "phase": phase,
                "status": status,
                "message": message,
            }));
        }
    }
}

fn default_role_prompt(prompt_id: &str) -> &'static str {
    match prompt_id {
        "agency_lead_writer_system" => "你是「主创」：基于黑板资产创作小说正文，草稿写入 draft 区。",
        "agency_producer_system" => "你是「管理」：生产世界观/角色/大纲/伏笔资产，写入 asset 区。",
        "agency_editor_auditor_system" => "你是「编辑审计」：审查草稿，输出裁决 JSON（verdict/blocking_issues/suggestions/comments）。",
        _ => "你是创作团队的一员。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::*;
    use crate::agency::repository::AgencyRepository;
    use crate::db::create_test_pool;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockLlm {
        responses: Mutex<VecDeque<String>>,
    }

    impl MockLlm {
        fn scripted(lines: Vec<&str>) -> Arc<Self> {
            Arc::new(Self { responses: Mutex::new(lines.into_iter().map(String::from).collect()) })
        }
    }

    #[async_trait::async_trait]
    impl LoopLlm for MockLlm {
        async fn complete(&self, _s: &str, _u: &str, _t: crate::router::TaskType, _m: i32) -> Result<String, AppError> {
            self.responses.lock().unwrap().pop_front()
                .ok_or_else(|| AppError::validation_failed("mock exhausted", None::<String>))
        }
    }

    /// 一次通过（verdict=pass）的完整脚本：concept → producer(tool,final) → writer(tool,final) → editor(final)
    fn pass_script() -> Arc<MockLlm> {
        MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星废土","summary":"双星废土"}}"#,
            r#"{"type":"final","content":"资产就绪"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"第一章正文：风沙中的拾荒者。","summary":"拾荒者登场"}}"#,
            r#"{"type":"final","content":"第一章完成"}"#,
            r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[\"可加强嗅觉描写\"],\"comments\":\"合格的首章\"}"}"#,
        ])
    }

    #[tokio::test]
    async fn test_genesis_end_to_end_pass() {
        let pool = create_test_pool().unwrap();
        let coordinator = AgencyCoordinator::for_test(pool.clone(), pass_script());
        let result = coordinator.run_genesis("r1", "星海拾荒者的故事").await.unwrap();
        assert!(!result.revised);
        assert_eq!(result.verdict.verdict, "pass");
        // run 状态 completed
        let repo = AgencyRepository::new(pool.clone());
        let run = repo.get_run("r1").unwrap().unwrap();
        assert_eq!(run.status, "completed");
        assert_eq!(run.story_id.as_deref(), Some(result.story_id.as_str()));
        // 黑板三分区都有内容
        let board = crate::agency::board::BlackboardService::new(pool.clone());
        let snap = board.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 1);
        assert_eq!(snap.drafts.len(), 1);
        assert_eq!(snap.reviews.len(), 1);
        // Scene 已装配，正文来自草稿
        let scene = SceneRepository::new(pool.clone()).get_by_id(&result.scene_id).unwrap().unwrap();
        assert_eq!(scene.content.as_deref(), Some("第一章正文：风沙中的拾荒者。"));
        assert!(result.chapter_chars > 0);
    }

    #[tokio::test]
    async fn test_genesis_revision_path() {
        let pool = create_test_pool().unwrap();
        let llm = MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产就绪"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"初稿。","summary":"初稿"}}"#,
            r#"{"type":"final","content":"初稿完成"}"#,
            r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"主角动机缺失\"],\"suggestions\":[],\"comments\":\"须修订\"}"}"#,
            // 修订轮
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"修订稿：他为了生存而拾荒。","summary":"修订稿"}}"#,
            r#"{"type":"final","content":"修订完成"}"#,
            // 修订后的第二轮审查（P1 无论结果放行）
            r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"修订后合格\"}"}"#,
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let result = coordinator.run_genesis("r2", "星海拾荒者的故事").await.unwrap();
        assert!(result.revised);
        let scene = SceneRepository::new(pool.clone()).get_by_id(&result.scene_id).unwrap().unwrap();
        assert_eq!(scene.content.as_deref(), Some("修订稿：他为了生存而拾荒。"));
    }

    #[tokio::test]
    async fn test_genesis_aborts_when_producer_fails() {
        let pool = create_test_pool().unwrap();
        let llm = MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
            "不是 JSON", "还不是", "依然不是", // producer 连续解析失败 → aborted
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let err = coordinator.run_genesis("r3", "前提").await.unwrap_err();
        assert!(err.to_string().contains("管理") || err.to_string().contains("producer") || err.to_string().contains("熔断"));
        let repo = AgencyRepository::new(pool.clone());
        let run = repo.get_run("r3").unwrap().unwrap();
        assert_eq!(run.status, "failed");
    }

    #[tokio::test]
    async fn test_genesis_aborts_when_editor_aborted() {
        let pool = create_test_pool().unwrap();
        let llm = MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产就绪"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"初稿。","summary":"初稿"}}"#,
            r#"{"type":"final","content":"初稿完成"}"#,
            "不是 JSON", "还不是", "依然不是", // editor 连续解析失败 → aborted → run failed（不得默认放行）
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let err = coordinator.run_genesis("r4", "前提").await.unwrap_err();
        assert!(err.to_string().contains("编辑审计") || err.to_string().contains("熔断"));
        let repo = AgencyRepository::new(pool.clone());
        let run = repo.get_run("r4").unwrap().unwrap();
        assert_eq!(run.status, "failed");
    }

    #[test]
    fn test_parse_lenient_json() {
        let v: EditorVerdict = parse_lenient("前言{\"verdict\":\"revise\",\"blocking_issues\":[\"a\"]}后缀").unwrap();
        assert_eq!(v.verdict, "revise");
        assert!(parse_lenient::<EditorVerdict>("无 JSON").is_none());
    }
}
