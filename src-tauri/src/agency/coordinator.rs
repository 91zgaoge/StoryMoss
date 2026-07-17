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

// ---- 在途 LLM request_id 注册表（定点取消用） ----

/// 运行中 run 的在途 LLM request_id 注册表（定点取消用）。
static AGENCY_REQUEST_REGISTRY: Lazy<Mutex<HashMap<String, std::collections::HashSet<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_request(run_id: &str, request_id: &str) {
    let mut registry = AGENCY_REQUEST_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    registry.entry(run_id.to_string()).or_default().insert(request_id.to_string());
}

pub fn unregister_request(run_id: &str, request_id: &str) {
    let mut registry = AGENCY_REQUEST_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(set) = registry.get_mut(run_id) {
        set.remove(request_id);
        if set.is_empty() {
            registry.remove(run_id);
        }
    }
}

/// 取走并清空某 run 的全部在途 request_id。
pub fn drain_requests(run_id: &str) -> Vec<String> {
    let mut registry = AGENCY_REQUEST_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    registry.remove(run_id).map(|s| s.into_iter().collect()).unwrap_or_default()
}

/// 定点取消：仅取消该 run 的在途 LLM 调用（对已完成 id 是 no-op）。
pub fn cancel_requests_for_run(llm: &LlmService, run_id: &str) {
    for request_id in drain_requests(run_id) {
        llm.cancel_generation(&request_id);
    }
}

/// 创世/续写前提校验：非空白且 ≤2000 字符。
pub fn validate_premise(premise: &str) -> Result<(), AppError> {
    let trimmed = premise.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_failed("前提不能为空", None::<String>));
    }
    if trimmed.chars().count() > 2000 {
        return Err(AppError::validation_failed("前提过长（≤2000 字符）", None::<String>));
    }
    Ok(())
}

// ---- LoopLlm 生产实现：全部 LLM 调用经 LlmService（路由/健康/成本落表保留） ----
// 每次调用登记 request_id 到 run 注册表，支持按 run 定点取消。

pub struct AgencyLlm {
    llm: LlmService,
    run_id: String,
}

impl AgencyLlm {
    pub fn new(app_handle: AppHandle, run_id: impl Into<String>) -> Self {
        Self { llm: LlmService::new(app_handle), run_id: run_id.into() }
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
        let request_id = uuid::Uuid::new_v4().to_string();
        register_request(&self.run_id, &request_id);
        let routing = crate::router::RoutingRequest {
            task,
            ..Default::default()
        };
        let (_rid, result) = self.llm
            .generate_for_request_with_request_id(
                routing,
                user_prompt.to_string(),
                Some(max_tokens),
                None,
                Some("agency"),
                Some(request_id.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(system_prompt.to_string()),
                None,
            )
            .await;
        unregister_request(&self.run_id, &request_id);
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

/// 质量门判定结果（取代 P1 的 fail-open 默认放行）。
#[derive(Debug)]
pub enum GateOutcome {
    Passed { verdict: EditorVerdict },
    RevisionRequired { verdict: EditorVerdict, issues: Vec<String> },
    Failed { reason: String },
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgencyContinueResult {
    pub run_id: String,
    pub story_id: String,
    pub scene_id: String,
    pub chapter_number: i32,
    pub revised: bool,
    pub verdict: EditorVerdict,
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
    llm: Option<Arc<dyn LoopLlm>>,
}

impl AgencyCoordinator {
    pub fn new(app_handle: AppHandle, pool: DbPool) -> Self {
        Self { app_handle: Some(app_handle), pool, llm: None }
    }

    /// 测试/无界面环境构造：不发 Tauri 事件，使用注入的 mock LLM。
    pub fn for_test(pool: DbPool, llm: Arc<dyn LoopLlm>) -> Self {
        Self { app_handle: None, pool, llm: Some(llm) }
    }

    /// 按 run 取得生产 LLM（带定点取消注册）；测试时返回注入的 mock。
    fn llm_for_run(&self, run_id: &str) -> Arc<dyn LoopLlm> {
        match &self.llm {
            Some(llm) => llm.clone(),
            None => Arc::new(AgencyLlm::new(
                self.app_handle.as_ref().expect("生产 coordinator 必有 app_handle").clone(),
                run_id,
            )),
        }
    }

    /// 同步 DB 调用一律经 spawn_blocking，避免阻塞 tokio 运行时线程。
    async fn db<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| AppError::from(format!("agency db join error: {}", e)))?
    }

    /// run 阶段推进（协调器运行期间 status 恒为 running）。
    async fn update_phase(&self, repo: &AgencyRepository, run_id: &str, phase: &str) -> Result<(), AppError> {
        let repo = repo.clone();
        let run_id = run_id.to_string();
        let phase = phase.to_string();
        self.db(move || repo.update_run_phase(&run_id, "running", &phase).map_err(AppError::from)).await
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
                let repo_c = repo.clone();
                let rid = run_id.to_string();
                let _ = self.db(move || repo_c.finish_run(&rid, "completed", Some(&json), None).map_err(AppError::from)).await;
                self.emit_progress(run_id, "assembly", "completed", "创世完成");
            }
            Err(e) => {
                let status = if cancel.load(Ordering::SeqCst) { "cancelled" } else { "failed" };
                // 失败/取消事件的 phase 取 run 当前落库阶段（不再硬编码 assembly）
                let repo_c = repo.clone();
                let rid = run_id.to_string();
                let phase = self.db(move || repo_c.get_run(&rid).map_err(AppError::from)).await
                    .ok().flatten().map(|r| r.phase).unwrap_or_else(|| "unknown".to_string());
                let repo_c = repo.clone();
                let rid = run_id.to_string();
                let msg = e.to_string();
                let _ = self.db(move || repo_c.finish_run(&rid, status, None, Some(&msg)).map_err(AppError::from)).await;
                self.emit_progress(run_id, &phase, status, &e.to_string());
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
        let llm = self.llm_for_run(run_id);
        let run = AgencyRun::new(run_id, premise);
        let repo_c = repo.clone();
        self.db(move || repo_c.create_run(&run).map_err(AppError::from)).await?;
        self.update_phase(repo, run_id, "concept").await?;
        self.emit_progress(run_id, "concept", "running", "正在构思故事概念");

        // 1) 概念：标题与类型
        let concept_raw = llm.complete(
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
        let repo_c = repo.clone();
        let rid = run_id.to_string();
        let sid = story_id.clone();
        self.db(move || repo_c.set_run_story(&rid, &sid).map_err(AppError::from)).await?;
        self.check_cancel(cancel)?;

        // 3) 管理：资产生产
        self.update_phase(repo, run_id, "assets").await?;
        self.emit_progress(run_id, "assets", "running", "管理 Agent 正在生产创作资产");
        let board = self.board();
        let registry = Arc::new(ToolRegistry::agency_default());
        let producer_out = self.run_role_with_llm(
            &llm, AgentRole::Producer, &board, &registry, run_id, &story_id, premise,
            "请为本故事生产创世资产：世界观、至少 2 张角色卡（真名/欲望/阻力）、第一卷大纲、伏笔清单。逐条写入资产区。",
        ).await.map_err(|e| AppError::from(format!("管理 Agent 阶段失败: {}", e)))?;
        if producer_out.aborted {
            return Err(AppError::from("管理 Agent 被熔断，资产生产未完成"));
        }
        self.check_cancel(cancel)?;

        // producer 完成后落库（黑板资产区 → characters/world_buildings/story_outlines）
        {
            let pool = self.pool.clone();
            let sid = story_id.clone();
            let assets = board.list_zone(run_id, BoardZone::Asset)?;
            let inserted = tokio::task::spawn_blocking(move || {
                crate::agency::materialize::materialize_assets(&pool, &sid, &assets)
            }).await.map_err(|e| AppError::from(format!("materialize join error: {}", e)))?;
            log::info!("agency: 资产落库 {} 条", inserted);
        }

        // 4) 主创：首章写作
        self.update_phase(repo, run_id, "writing").await?;
        self.emit_progress(run_id, "writing", "running", "主创 Agent 正在写作第一章");
        let writer_out = self.run_role_with_llm(
            &llm, AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise,
            "基于资产区创作第一章正文（1500-2500 字）。先用 board_read 读资产，再用 board_write 把完整正文写入 draft 区（item_type=chapter, key=第一章）。",
        ).await.map_err(|e| AppError::from(format!("主创 Agent 阶段失败: {}", e)))?;
        if writer_out.aborted {
            return Err(AppError::from("主创 Agent 被熔断，首章未完成"));
        }
        let mut draft = self.latest_draft(&board, run_id).await?;
        self.check_cancel(cancel)?;

        // 5) 质量门 + 至多 1 轮修订（第二轮审查后无论结果放行，Failed 除外）
        let mut revised = false;
        let final_verdict = loop {
            self.update_phase(repo, run_id, "review").await?;
            self.emit_progress(run_id, "review", "running", "质量门评估中");
            let outcome = self.evaluate_gate(&llm, &board, &registry, run_id, &story_id, premise, &draft).await?;
            match outcome {
                GateOutcome::Passed { verdict } => break verdict,
                GateOutcome::RevisionRequired { issues, .. } if !revised => {
                    revised = true;
                    self.update_phase(repo, run_id, "revision").await?;
                    self.emit_progress(run_id, "revision", "running", "主创 Agent 正在按审查意见修订");
                    let task = Self::build_revision_task(&draft, &issues);
                    let revise_out = self.run_role_with_llm(
                        &llm, AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise, &task,
                    ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
                    if revise_out.aborted {
                        return Err(AppError::from("主创 Agent 修订轮被熔断"));
                    }
                    draft = self.latest_draft(&board, run_id).await?;
                    self.check_cancel(cancel)?;
                    // 复审：无论结果都进入装配（Failed 除外）
                    let second = self.evaluate_gate(&llm, &board, &registry, run_id, &story_id, premise, &draft).await?;
                    match second {
                        GateOutcome::Passed { verdict } => break verdict,
                        GateOutcome::RevisionRequired { verdict, .. } => break verdict, // 第二轮放行
                        GateOutcome::Failed { reason } => {
                            return Err(AppError::from(format!("质量门未通过: {}", reason)));
                        }
                    }
                }
                GateOutcome::RevisionRequired { verdict, .. } => break verdict,
                GateOutcome::Failed { reason } => {
                    return Err(AppError::from(format!("质量门未通过: {}", reason)));
                }
            }
        };
        self.check_cancel(cancel)?;

        // 6) 装配：草稿 → Scene 真源（统一输出装配器 P1 形态）
        self.update_phase(repo, run_id, "assembly").await?;
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
        // 装配完成后、交付结果前再查一次：确保 cancelled 不被 completed 覆盖
        self.check_cancel(cancel)?;

        Ok(AgencyGenesisResult {
            run_id: run_id.to_string(),
            story_id,
            scene_id: scene.id,
            revised,
            verdict: final_verdict,
            chapter_chars: draft.content.chars().count(),
        })
    }

    /// 续写循环（串行）：资产确认/补齐 → 写作 → 质量门 → 装配。
    pub async fn run_continue(
        &self,
        run_id: &str,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<AgencyContinueResult, AppError> {
        let repo = AgencyRepository::new(self.pool.clone());
        let cancel = register_agency_cancel(run_id);
        let result = self.run_continue_inner(run_id, story_id, chapter_number, &repo, &cancel).await;
        unregister_agency_cancel(run_id);
        match &result {
            Ok(r) => {
                let json = serde_json::to_string(r).unwrap_or_default();
                let _ = repo.finish_run(run_id, "completed", Some(&json), None);
                self.emit_progress(run_id, "assembly", "completed", "续写完成");
            }
            Err(e) => {
                let status = if cancel.load(std::sync::atomic::Ordering::SeqCst) { "cancelled" } else { "failed" };
                let _ = repo.finish_run(run_id, status, None, Some(&e.to_string()));
                self.emit_progress(run_id, "assembly", status, &e.to_string());
            }
        }
        result
    }

    async fn run_continue_inner(
        &self,
        run_id: &str,
        story_id: &str,
        chapter_number: i32,
        repo: &AgencyRepository,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AgencyContinueResult, AppError> {
        let llm = self.llm_for_run(run_id);
        let title = self.story_title(story_id).await.unwrap_or_else(|| "未命名".to_string());
        let premise = format!("续写《{}》第{}章", title, chapter_number);
        repo.create_run(&AgencyRun::new(run_id, &premise)).map_err(AppError::from)?;
        repo.set_run_story(run_id, story_id).map_err(AppError::from)?;
        repo.update_run_phase(run_id, "running", "assets").map_err(AppError::from)?;
        self.emit_progress(run_id, "assets", "running", "正在确认创作资产");

        // 1) 资产确认/补齐：先查 characters 表；为空则先从本 story 历史黑板条目落库，仍无再让 producer 现场补齐
        let character_count = {
            let pool = self.pool.clone();
            let sid = story_id.to_string();
            tokio::task::spawn_blocking(move || -> Result<i64, AppError> {
                let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
                conn.query_row("SELECT COUNT(*) FROM characters WHERE story_id = ?1",
                    rusqlite::params![sid], |r| r.get(0)).map_err(AppError::from)
            }).await.map_err(|e| AppError::from(format!("asset check join error: {}", e)))??
        };
        if character_count == 0 {
            // 先尝试从本 story 历史黑板条目落库（免费路径）
            let pool = self.pool.clone();
            let sid = story_id.to_string();
            let history_items = repo.list_items_for_story(story_id, Some(BoardZone::Asset)).map_err(AppError::from)?;
            let inserted = tokio::task::spawn_blocking(move || {
                crate::agency::materialize::materialize_assets(&pool, &sid, &history_items)
            }).await.map_err(|e| AppError::from(format!("materialize join error: {}", e)))?;
            if inserted == 0 {
                // 仍无资产：producer 现场补齐
                let board = self.board();
                let registry = Arc::new(ToolRegistry::agency_default());
                let producer_out = self.run_role_with_llm(
                    &llm, AgentRole::Producer, &board, &registry, run_id, story_id, &premise,
                    "为这部已有故事补齐创作资产：先 story_info 与 asset_query 了解现状，再生产世界观/角色卡（JSON 格式）/大纲，写入资产区。",
                ).await.map_err(|e| AppError::from(format!("管理 Agent 资产补齐失败: {}", e)))?;
                if producer_out.aborted {
                    return Err(AppError::from("管理 Agent 被熔断，资产补齐未完成"));
                }
                let assets = board.list_zone(run_id, BoardZone::Asset)?;
                let pool = self.pool.clone();
                let sid = story_id.to_string();
                tokio::task::spawn_blocking(move || {
                    crate::agency::materialize::materialize_assets(&pool, &sid, &assets)
                }).await.map_err(|e| AppError::from(format!("materialize join error: {}", e)))?;
            }
        }
        self.check_cancel(cancel)?;

        // 2) 写作
        repo.update_run_phase(run_id, "running", "writing").map_err(AppError::from)?;
        self.emit_progress(run_id, "writing", "running", &format!("主创 Agent 正在写作第{}章", chapter_number));
        let board = self.board();
        let registry = Arc::new(ToolRegistry::agency_default());
        let key = format!("第{}章", chapter_number);
        let writer_out = self.run_role_with_llm(
            &llm, AgentRole::LeadWriter, &board, &registry, run_id, story_id, &premise,
            &format!(
                "续写{}（1500-2500 字）。先 board_read 读资产区、asset_query(kind=scenes) 读最近场景保持连贯，再用 board_write 把完整正文写入 draft 区（item_type=chapter, key={}）。",
                key, key
            ),
        ).await.map_err(|e| AppError::from(format!("主创 Agent 阶段失败: {}", e)))?;
        if writer_out.aborted {
            return Err(AppError::from("主创 Agent 被熔断，本章未完成"));
        }
        let mut draft = self.latest_draft(&board, run_id).await?;
        self.check_cancel(cancel)?;

        // 3) 质量门 + 至多 1 轮修订（与 genesis 同门径）
        let mut revised = false;
        let final_verdict = loop {
            repo.update_run_phase(run_id, "running", "review").map_err(AppError::from)?;
            self.emit_progress(run_id, "review", "running", "质量门评估中");
            let outcome = self.evaluate_gate(&llm, &board, &registry, run_id, story_id, &premise, &draft).await?;
            match outcome {
                GateOutcome::Passed { verdict } => break verdict,
                GateOutcome::RevisionRequired { issues, .. } if !revised => {
                    revised = true;
                    repo.update_run_phase(run_id, "running", "revision").map_err(AppError::from)?;
                    let task = Self::build_revision_task(&draft, &issues);
                    let revise_out = self.run_role_with_llm(
                        &llm, AgentRole::LeadWriter, &board, &registry, run_id, story_id, &premise, &task,
                    ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
                    if revise_out.aborted {
                        return Err(AppError::from("主创 Agent 修订轮被熔断"));
                    }
                    draft = self.latest_draft(&board, run_id).await?;
                    self.check_cancel(cancel)?;
                    let second = self.evaluate_gate(&llm, &board, &registry, run_id, story_id, &premise, &draft).await?;
                    match second {
                        GateOutcome::Passed { verdict } => break verdict,
                        GateOutcome::RevisionRequired { verdict, .. } => break verdict,
                        GateOutcome::Failed { reason } => return Err(AppError::from(format!("质量门未通过: {}", reason))),
                    }
                }
                GateOutcome::RevisionRequired { verdict, .. } => break verdict,
                GateOutcome::Failed { reason } => return Err(AppError::from(format!("质量门未通过: {}", reason))),
            }
        };

        // 4) 装配
        repo.update_run_phase(run_id, "running", "assembly").map_err(AppError::from)?;
        let pool = self.pool.clone();
        let sid = story_id.to_string();
        let content = draft.content.clone();
        let title_c = key.clone();
        let scene = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
            let repo = crate::db::repositories::SceneRepository::new(pool);
            let scene = repo.create(&sid, chapter_number, Some(&title_c)).map_err(AppError::from)?;
            repo.update(&scene.id, &crate::db::repositories::SceneUpdate {
                content: Some(content),
                ..Default::default()
            }).map_err(AppError::from)?;
            Ok(scene)
        }).await.map_err(|e| AppError::from(format!("scene assembly join error: {}", e)))??;

        Ok(AgencyContinueResult {
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            scene_id: scene.id,
            chapter_number,
            revised,
            verdict: final_verdict,
        })
    }

    async fn story_title(&self, story_id: &str) -> Option<String> {
        let pool = self.pool.clone();
        let sid = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().ok()?;
            conn.query_row("SELECT title FROM stories WHERE id = ?1",
                rusqlite::params![sid], |r| r.get::<_, String>(0)).ok()
        }).await.ok().flatten()
    }

    /// 质量门：editor 裁决（解析失败重试 1 次）→ pass 后再经规则复检。
    /// 行为规格：aborted → Failed；裁决解析重试后仍失败 → Failed；
    /// revise+blocking → RevisionRequired；pass → 规则复检 High+ → RevisionRequired，否则 Passed；
    /// 每次判定（含 Failed）落审查区 item_type="gate"。
    pub(crate) async fn evaluate_gate(
        &self,
        llm: &Arc<dyn LoopLlm>,
        board: &BlackboardService,
        registry: &Arc<ToolRegistry>,
        run_id: &str,
        story_id: &str,
        premise: &str,
        draft: &BoardItem,
    ) -> Result<GateOutcome, AppError> {
        // 1) editor 裁决（解析失败重试一次）
        let mut verdict: Option<EditorVerdict> = None;
        let mut last_raw = String::new();
        for attempt in 0..2 {
            let editor_out = self.run_role_with_llm(
                llm, AgentRole::EditorAuditor, board, registry, run_id, story_id, premise,
                &format!("审查 draft 区的最新章节草稿（{}）。按系统提示词出具裁决 JSON。", draft.key),
            ).await.map_err(|e| AppError::from(format!("编辑审计 Agent 阶段失败: {}", e)))?;
            if editor_out.aborted {
                let outcome = GateOutcome::Failed { reason: "编辑审计 Agent 被熔断".to_string() };
                self.record_gate(board, run_id, story_id, draft, &outcome).await?;
                return Ok(outcome);
            }
            last_raw = editor_out.output.clone();
            if let Some(v) = parse_lenient::<EditorVerdict>(&editor_out.output) {
                verdict = Some(v);
                break;
            }
            log::warn!("agency gate: 裁决解析失败（第 {} 次）", attempt + 1);
        }
        let verdict = match verdict {
            Some(v) => v,
            None => {
                let outcome = GateOutcome::Failed {
                    reason: format!(
                        "裁决解析失败（重试 1 次后仍失败）: {}",
                        last_raw.chars().take(120).collect::<String>()
                    ),
                };
                self.record_gate(board, run_id, story_id, draft, &outcome).await?;
                return Ok(outcome);
            }
        };
        // 2) 判定：revise+blocking 直接 RevisionRequired；否则确定性规则复检（LLM 说 pass 也要过规则）
        let outcome = if verdict.verdict == "revise" && !verdict.blocking_issues.is_empty() {
            GateOutcome::RevisionRequired { issues: verdict.blocking_issues.clone(), verdict }
        } else {
            let board_c = board.clone();
            let rid = run_id.to_string();
            let hints = self.db(move || {
                Ok(board_c.list_zone(&rid, BoardZone::Asset)?
                    .into_iter()
                    .filter(|i| i.item_type == "foreshadowing")
                    .map(|i| i.summary)
                    .collect::<Vec<_>>())
            }).await?;
            let pool = self.pool.clone();
            let sid = story_id.to_string();
            let ctx = tokio::task::spawn_blocking(move || {
                crate::agency::gate::build_review_context(&pool, &sid, &hints)
            }).await.map_err(|e| AppError::from(format!("gate ctx join error: {}", e)))?;
            let notes = crate::agents::subagents::run_subagent_review(&ctx, &draft.content).await;
            let merged = crate::agency::gate::merge_rule_issues(&notes);
            if merged.is_empty() {
                GateOutcome::Passed { verdict }
            } else {
                GateOutcome::RevisionRequired { issues: merged, verdict }
            }
        };
        // 3) 判定落审查区（编辑审计为审查区 owner，active）
        self.record_gate(board, run_id, story_id, draft, &outcome).await?;
        Ok(outcome)
    }

    /// 供 Task 2 修订路径与测试使用的指令生成（纯函数）。
    pub(crate) fn build_revision_task(draft: &BoardItem, issues: &[String]) -> String {
        format!(
            "修订「{}」。先用 board_revise 直接修订该条目（item_id={}, expected_version={}），content 为完整修订稿。审查阻断问题：{}",
            draft.key, draft.id, draft.version, issues.join("；")
        )
    }

    /// 门判定落审查区：item_type="gate"，content=裁决 JSON + 规则问题数，status=active。
    async fn record_gate(
        &self,
        board: &BlackboardService,
        run_id: &str,
        story_id: &str,
        draft: &BoardItem,
        outcome: &GateOutcome,
    ) -> Result<(), AppError> {
        let (kind, detail, issues) = match outcome {
            GateOutcome::Passed { .. } => ("pass", String::new(), Vec::new()),
            GateOutcome::RevisionRequired { issues, .. } => {
                ("revise", format!("{} 条问题", issues.len()), issues.clone())
            }
            GateOutcome::Failed { reason } => ("failed", reason.clone(), Vec::new()),
        };
        let content = serde_json::json!({
            "outcome": kind,
            "verdict": gate_verdict(outcome),
            "rule_issue_count": issues.len(),
            "issues": issues,
            "comments": verdict_comments(outcome),
        }).to_string();
        let summary = format!("gate:{} {}", kind, detail).chars().take(80).collect::<String>();
        let board_c = board.clone();
        let rid = run_id.to_string();
        let sid = story_id.to_string();
        let key = format!("gate-{}", draft.key);
        self.db(move || board_c.write(&rid, &sid, AgentRole::EditorAuditor, BoardZone::Review,
            "gate", &key, &content, &summary)).await?;
        Ok(())
    }

    async fn run_role_with_llm(
        &self,
        llm: &Arc<dyn LoopLlm>,
        role: AgentRole,
        board: &BlackboardService,
        registry: &Arc<ToolRegistry>,
        run_id: &str,
        story_id: &str,
        premise: &str,
        task: &str,
    ) -> Result<crate::agency::tool_loop::LoopResult, AppError> {
        let spec = spec_for(role);
        let system_prompt = self.resolve_role_prompt(spec.prompt_id, premise).await;
        let ctx = ToolContext {
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            role,
            board: board.clone(),
            pool: self.pool.clone(),
        };
        ToolLoop::new(llm.clone(), registry.clone())
            .with_max_turns(spec.max_turns)
            .run(role, &ctx, &system_prompt, task)
            .await
    }

    /// 最新有效草稿：从尾部反向查找最后一条 content 非空的 active draft
    ///（最新条为空不再报错；proposed 提案不参与，绕过仲裁的写入不得被消费）。
    async fn latest_draft(&self, board: &BlackboardService, run_id: &str) -> Result<BoardItem, AppError> {
        let board = board.clone();
        let run_id = run_id.to_string();
        self.db(move || {
            let drafts = board.list_zone(&run_id, BoardZone::Draft)?;
            drafts.into_iter().rev()
                .find(|d| d.status == "active" && !d.content.is_empty())
                .ok_or_else(|| AppError::from("草稿区为空：主创未产出正文"))
        }).await
    }

    fn board(&self) -> BlackboardService {
        match &self.app_handle {
            Some(app) => BlackboardService::with_events(self.pool.clone(), app),
            None => BlackboardService::new(self.pool.clone()),
        }
    }

    /// 角色系统提示词：优先 PromptRegistry（支持用户覆盖），注册表不可用时回退内置短提示。
    /// 注册表走 DB，同样经 db() 防阻塞。
    async fn resolve_role_prompt(&self, prompt_id: &str, premise: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("premise".to_string(), premise.to_string());
        let pool = self.pool.clone();
        let pid = prompt_id.to_string();
        let resolved = self.db(move || crate::prompts::registry::resolve_prompt_with_vars(&pool, &pid, &vars)).await;
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

fn verdict_comments(outcome: &GateOutcome) -> String {
    match outcome {
        GateOutcome::Passed { verdict } => verdict.comments.clone(),
        GateOutcome::RevisionRequired { verdict, .. } => verdict.comments.clone(),
        GateOutcome::Failed { .. } => String::new(),
    }
}

fn gate_verdict(outcome: &GateOutcome) -> Option<&EditorVerdict> {
    match outcome {
        GateOutcome::Passed { verdict } => Some(verdict),
        GateOutcome::RevisionRequired { verdict, .. } => Some(verdict),
        GateOutcome::Failed { .. } => None,
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

    /// concept 响应后立即置取消 flag 的 mock（模拟用户在概念完成后取消）。
    struct CancelAfterConceptLlm {
        inner: Arc<MockLlm>,
        run_id: String,
        fired: AtomicBool,
    }

    #[async_trait::async_trait]
    impl LoopLlm for CancelAfterConceptLlm {
        async fn complete(&self, s: &str, u: &str, t: crate::router::TaskType, m: i32) -> Result<String, AppError> {
            let out = self.inner.complete(s, u, t, m).await?;
            if !self.fired.swap(true, Ordering::SeqCst) {
                assert!(cancel_agency_run(&self.run_id), "取消 flag 应已注册");
            }
            Ok(out)
        }
    }

    #[tokio::test]
    async fn test_genesis_cancel_not_overwritten_by_completed() {
        let pool = create_test_pool().unwrap();
        let llm = Arc::new(CancelAfterConceptLlm {
            inner: pass_script(),
            run_id: "r5".to_string(),
            fired: AtomicBool::new(false),
        });
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let err = coordinator.run_genesis("r5", "星海拾荒者的故事").await.unwrap_err();
        assert!(err.to_string().contains("取消"), "应返回取消错误: {}", err);
        let repo = AgencyRepository::new(pool.clone());
        let run = repo.get_run("r5").unwrap().unwrap();
        assert_eq!(run.status, "cancelled");
        // 终态守护：cancelled 不得被 completed 覆盖
        repo.finish_run("r5", "completed", Some("{}"), None).unwrap();
        let run = repo.get_run("r5").unwrap().unwrap();
        assert_eq!(run.status, "cancelled");
    }

    #[test]
    fn test_parse_lenient_json() {
        let v: EditorVerdict = parse_lenient("前言{\"verdict\":\"revise\",\"blocking_issues\":[\"a\"]}后缀").unwrap();
        assert_eq!(v.verdict, "revise");
        assert!(parse_lenient::<EditorVerdict>("无 JSON").is_none());
    }

    #[test]
    fn test_request_registry_lifecycle() {
        let run = "run-registry-test";
        register_request(run, "req-1");
        register_request(run, "req-2");
        register_request("other-run", "req-x");
        // 收集并清空目标 run 的全部 request_id
        let drained = drain_requests(run);
        assert_eq!(drained.len(), 2);
        assert!(drained.contains(&"req-1".to_string()));
        assert!(drained.contains(&"req-2".to_string()));
        // 已清空，再取为空
        assert!(drain_requests(run).is_empty());
        // 其他 run 不受影响
        assert_eq!(drain_requests("other-run"), vec!["req-x".to_string()]);
    }

    #[test]
    fn test_unregister_request() {
        register_request("run-u", "req-a");
        unregister_request("run-u", "req-a");
        assert!(drain_requests("run-u").is_empty());
    }

    #[test]
    fn test_validate_premise() {
        assert!(validate_premise("一个关于星海拾荒者的故事").is_ok());
        assert!(validate_premise("").is_err());
        assert!(validate_premise("   ").is_err());
        let too_long = "长".repeat(2001);
        assert!(validate_premise(&too_long).is_err());
        let at_limit = "长".repeat(2000);
        assert!(validate_premise(&at_limit).is_ok());
    }

    #[tokio::test]
    async fn test_gate_fails_after_verdict_parse_retry() {
        let pool = create_test_pool().unwrap();
        // concept + producer(tool,final) + writer(tool,final) + editor 两次非法裁决
        let llm = MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产就绪"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"正文。","summary":"初稿"}}"#,
            r#"{"type":"final","content":"完成"}"#,
            r#"{"type":"final","content":"这根本不是JSON裁决"}"#,
            r#"{"type":"final","content":"依然不是JSON"}"#,
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let err = coordinator.run_genesis("r-gate-1", "前提").await.unwrap_err();
        assert!(err.to_string().contains("质量门") || err.to_string().contains("裁决") || err.to_string().contains("审查"));
        let repo = AgencyRepository::new(pool.clone());
        assert_eq!(repo.get_run("r-gate-1").unwrap().unwrap().status, "failed");
        // 规格 5：门判定（含 Failed）落审查区 item_type="gate"
        let board = crate::agency::board::BlackboardService::new(pool.clone());
        let snap = board.snapshot("r-gate-1").unwrap();
        assert!(snap.reviews.iter().any(|i| i.item_type == "gate"), "Failed 判定也应写 gate 条目");
    }

    /// 修订指令（纯函数）须携带 item_id 与 expected_version，供 board_revise 原地修订。
    #[test]
    fn test_build_revision_task_contains_item_ref() {
        let draft = BoardItem::new(
            "r", "s", BoardZone::Draft, "chapter", "第一章", "初稿。", "初稿",
            AgentRole::LeadWriter, "active",
        );
        let task = AgencyCoordinator::build_revision_task(&draft, &["动机缺失".to_string()]);
        assert!(task.contains("board_revise"));
        assert!(task.contains(&format!("item_id={}", draft.id)));
        assert!(task.contains("expected_version=1"));
        assert!(task.contains("动机缺失"));
    }

    #[tokio::test]
    async fn test_continue_chapter_end_to_end() {
        let pool = create_test_pool().unwrap();
        // 预置故事 + 一个角色 + 第一章场景
        let story = crate::db::repositories::StoryRepository::new(pool.clone()).create(crate::db::dto::CreateStoryRequest {
            title: "续写书".into(), description: Some("前提".into()), genre: None,
            style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
        }).unwrap();
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
                 VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
                rusqlite::params![story.id],
            ).unwrap();
        }
        let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
        let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
        scene_repo.update(&ch1.id, &crate::db::repositories::SceneUpdate {
            content: Some("第一章正文。".to_string()),
            ..Default::default()
        }).unwrap();

        let llm = MockLlm::scripted(vec![
            // writer: 查前文 + 写第二章
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第二章","content":"第二章正文：星舰苏醒。","summary":"星舰苏醒"}}"#,
            r#"{"type":"final","content":"第二章完成"}"#,
            // editor: pass
            r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let result = coordinator.run_continue("rc-1", &story.id, 2).await.unwrap();
        assert_eq!(result.chapter_number, 2);
        let scene = crate::db::repositories::SceneRepository::new(pool.clone())
            .get_by_id(&result.scene_id).unwrap().unwrap();
        assert_eq!(scene.content.as_deref(), Some("第二章正文：星舰苏醒。"));
        let run = AgencyRepository::new(pool.clone()).get_run("rc-1").unwrap().unwrap();
        assert_eq!(run.status, "completed");
    }

    #[tokio::test]
    async fn test_continue_fails_without_assets_and_producer_aborts() {
        // 无资产且 producer 熔断 → failed（验证资产补齐路径的熔断传播）
        let pool = create_test_pool().unwrap();
        let story = crate::db::repositories::StoryRepository::new(pool.clone()).create(crate::db::dto::CreateStoryRequest {
            title: "无资产书".into(), description: None, genre: None,
            style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
        }).unwrap();
        let llm = MockLlm::scripted(vec!["不是 JSON", "还不是", "依然不是"]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let err = coordinator.run_continue("rc-2", &story.id, 1).await.unwrap_err();
        assert!(err.to_string().contains("管理") || err.to_string().contains("熔断") || err.to_string().contains("资产"));
        assert_eq!(AgencyRepository::new(pool.clone()).get_run("rc-2").unwrap().unwrap().status, "failed");
    }

    /// T3 遗留修复：build_review_context 填充 previous_chapters 后，
    /// 规则复检（ContinuityAgent 重复开头检查 → High）必须能拦截 editor 放行的草稿。
    #[tokio::test]
    async fn test_gate_rule_recheck_blocks_repeated_opening() {
        let pool = create_test_pool().unwrap();
        let story = crate::db::repositories::StoryRepository::new(pool.clone()).create(crate::db::dto::CreateStoryRequest {
            title: "复检书".into(), description: None, genre: None,
            style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
        }).unwrap();
        // 预置第一章场景，后续草稿开头与其高度重复
        let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
        let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
        scene_repo.update(&ch1.id, &crate::db::repositories::SceneUpdate {
            content: Some("风沙掠过双星废土的清晨，阿苔在残骸中醒来，耳边是磁力风暴的低鸣。".to_string()),
            ..Default::default()
        }).unwrap();

        let repo = AgencyRepository::new(pool.clone());
        repo.create_run(&AgencyRun::new("rg-1", "续写")).unwrap();
        let board = crate::agency::board::BlackboardService::new(pool.clone());
        let draft = board.write(
            "rg-1", &story.id, AgentRole::LeadWriter, BoardZone::Draft, "chapter", "第二章",
            "风沙掠过双星废土的清晨，阿苔在残骸中醒来，这一次她抬头看到了星环。", "第二章草稿",
        ).unwrap();

        // editor 放行（pass）；门应被规则复检拦下 → RevisionRequired
        let llm: Arc<dyn LoopLlm> = MockLlm::scripted(vec![
            r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
        let registry = Arc::new(ToolRegistry::agency_default());
        let outcome = coordinator
            .evaluate_gate(&llm, &board, &registry, "rg-1", &story.id, "续写", &draft)
            .await
            .unwrap();
        match outcome {
            GateOutcome::RevisionRequired { issues, .. } => {
                assert!(
                    issues.iter().any(|i| i.contains("重复")),
                    "规则复检应报告重复开头问题: {:?}",
                    issues
                );
            }
            other => panic!("规则复检应拦截重复开头的草稿，实际: {:?}", other),
        }
    }
}
