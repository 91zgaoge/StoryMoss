//! 真机功能探针（v0.66.0–v0.72.0 的机器级验收）：**真机库副本 + 真机模型**。
//!
//! 与 golden harness 的分工：golden 用固定 fixture
//! 量「续写质量」；本模块用**真机库的
//! 副本**（《帝国的烟火》等真实故事与真实向量库）跑**新功能**的端到端验收——
//! 检索注入、手法落库、卷纲注入、纲层重规划、判官一致率落库。
//!
//! ## 怎么跑（全部 `#[ignore]`，CI 不执行）
//!
//! ```bash
//! cd src-tauri
//! cargo test --lib real_machine_probe -- --ignored --nocapture
//! ```
//!
//! 可选环境变量：
//! - `STORYMOSS_DB`           真机库路径（默认 `~/Library/Application
//!   Support/com.storymoss.app/cinema_ai.db`）
//! - `STORYMOSS_PROBE_MODEL`  profile id 或 model 名（默认取配置的
//!   creative_model_id → deepseek → 第一个 OpenAI 兼容）
//! - `STORYMOSS_PROBE_TOOL_MODEL`    覆盖生产档（Tool：章节纲/规划/世界构建）
//! - `STORYMOSS_PROBE_EDITOR_MODEL`  覆盖编辑档（Background：质量门裁决）
//! - `STORYMOSS_PROBE_CONFIG_DIR` 覆盖配置目录
//!
//! **安全**：库与向量库一律复制到临时目录后再操作；真机文件只读。读不到的配置/
//! 模型/数据 一律打印跳过原因并 return（不红），与项目既有真机探针同款。

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::{coordinator::AgencyCoordinator, persist::PersistMode};
use crate::{
    config::settings::{AppConfig, LlmProfile, LlmProvider},
    db::DbPool,
    error::AppError,
    llm::{
        adapter::{GenerateRequest, LlmAdapter, ResponseFormat},
        anthropic::AnthropicAdapter,
        ollama::OllamaAdapter,
        openai::OpenAiAdapter,
    },
    router::TaskType,
};

const ENV_DB: &str = "STORYMOSS_DB";
const ENV_MODEL: &str = "STORYMOSS_PROBE_MODEL";
const ENV_TOOL_MODEL: &str = "STORYMOSS_PROBE_TOOL_MODEL";
const ENV_EDITOR_MODEL: &str = "STORYMOSS_PROBE_EDITOR_MODEL";
const ENV_CONFIG_DIR: &str = "STORYMOSS_PROBE_CONFIG_DIR";

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn real_db_src() -> PathBuf {
    if let Some(path) = env_var(ENV_DB) {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!(
        "{home}/Library/Application Support/com.storymoss.app/cinema_ai.db"
    ))
}

fn app_config_dir() -> PathBuf {
    if let Some(dir) = env_var(ENV_CONFIG_DIR) {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.storymoss.app")
}

/// 真机 AppConfig（SQLite 配置源）。
fn real_config() -> Option<AppConfig> {
    let dir = app_config_dir();
    match AppConfig::load(&dir) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            println!("[真机探针] 跳过：读取配置失败 dir={dir:?}: {e}");
            None
        }
    }
}

/// 选目标模型：env → creative_model_id → 含 deepseek 者 → 第一个 OpenAI 兼容。
fn pick_profile(cfg: &AppConfig, wanted: Option<&str>) -> Result<LlmProfile, String> {
    let mut all: Vec<LlmProfile> = cfg.llm_profiles.values().cloned().collect();
    all.sort_by(|a, b| a.id.cmp(&b.id));
    if all.is_empty() {
        return Err("配置里没有任何 llm_profiles".to_string());
    }
    if let Some(want) = wanted {
        let lower = want.to_lowercase();
        return all
            .iter()
            .find(|p| p.id == want || p.model.eq_ignore_ascii_case(&lower))
            .cloned()
            .ok_or_else(|| format!("STORYMOSS_PROBE_MODEL={want} 未匹配任何 profile"));
    }
    if let Some(id) = cfg.creative_model_id.as_deref().filter(|s| !s.is_empty()) {
        if let Some(p) = all.iter().find(|p| p.id == id) {
            return Ok(p.clone());
        }
    }
    if let Some(p) = all
        .iter()
        .find(|p| p.model.to_lowercase().contains("deepseek"))
    {
        return Ok(p.clone());
    }
    // 其余 provider（OpenAI/Azure/DeepSeek/Qwen/Custom）都走 OpenAI
    // 兼容适配器。
    all.iter()
        .find(|p| !matches!(p.provider, LlmProvider::Anthropic | LlmProvider::Ollama))
        .cloned()
        .ok_or_else(|| "没有可用的 OpenAI 兼容 profile".to_string())
}

fn build_adapter(profile: &LlmProfile) -> Box<dyn LlmAdapter> {
    let max_tokens = if profile.max_tokens > 0 {
        profile.max_tokens
    } else {
        8192
    };
    match profile.provider {
        LlmProvider::Anthropic => Box::new(AnthropicAdapter::new(
            profile.api_key.clone(),
            profile.model.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds,
            60, // v0.72.3: 与生产默认一致（10s 对本地 MoE 大提示词的
            // 非流式调用过短——首响应头即被误判连接超时）
            300, // 真机慢模型首块超时放宽（探针专用；生产走配置/candidate 机制）
        )),
        LlmProvider::Ollama => Box::new(OllamaAdapter::new(
            profile.api_key.clone(),
            profile.model.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds,
            60, // v0.72.3: 与生产默认一致（10s 对本地 MoE 大提示词的
            // 非流式调用过短——首响应头即被误判连接超时）
            300, // 真机慢模型首块超时放宽（探针专用；生产走配置/candidate 机制）
        )),
        _ => Box::new(OpenAiAdapter::new(
            profile.api_key.clone(),
            profile.model.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds,
            60, // v0.72.3: 与生产默认一致（10s 对本地 MoE 大提示词的
            // 非流式调用过短——首响应头即被误判连接超时）
            300, // 真机慢模型首块超时放宽（探针专用；生产走配置/candidate 机制）
        )),
    }
}

/// 真实模型 + 调用记录（断言 prompt 内容用）。
///
/// 按 `TaskType` 模拟应用的**三档角色路由**（生产为 `llm_for_run(role)` 按
/// context_label 映射 Creative/Tool/Background）：
/// - 编辑审查（Proofreading/Editing）→ Background 档（`agency_editor`）
/// - 生产/规划（WorldBuilding/Analysis）→ Tool 档（`agency_producer`，
///   章节纲/卷纲/世界构建）
/// - 其余（CreativeWriting 等写作）→ Creative 档（`agency_writer`）
struct ProbeLlm {
    creative: Box<dyn LlmAdapter>,
    tool: Box<dyn LlmAdapter>,
    editor: Box<dyn LlmAdapter>,
    temperature: f32,
    calls: Mutex<Vec<ProbeCall>>,
    responses: Mutex<Vec<(String, usize, String)>>,
    errors: Mutex<Vec<String>>,
}

/// 一次探针模型调用的记录（kind + 实际服务档 + user prompt 原文）。
struct ProbeCall {
    kind: String,
    route: &'static str,
    user: String,
}

impl ProbeLlm {
    fn new(
        creative: Box<dyn LlmAdapter>,
        tool: Box<dyn LlmAdapter>,
        editor: Box<dyn LlmAdapter>,
        temperature: f32,
    ) -> Arc<Self> {
        Arc::new(Self {
            creative,
            tool,
            editor,
            temperature,
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
        })
    }

    fn adapter_for(&self, task: TaskType) -> (&'static str, &dyn LlmAdapter) {
        match task {
            TaskType::Proofreading | TaskType::Editing => ("editor(Background)", &*self.editor),
            TaskType::WorldBuilding | TaskType::Analysis => ("tool(Tool)", &*self.tool),
            _ => ("creative(Creative)", &*self.creative),
        }
    }

    async fn call(
        &self,
        system: &str,
        user: &str,
        task: TaskType,
        max_tokens: i32,
        json: bool,
        kind: &'static str,
    ) -> Result<String, AppError> {
        let (route, adapter) = self.adapter_for(task);
        {
            let mut calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
            calls.push(ProbeCall {
                kind: kind.to_string(),
                route,
                user: user.to_string(),
            });
        }
        let request = GenerateRequest {
            prompt: user.to_string(),
            max_tokens: Some(max_tokens),
            temperature: Some(self.temperature),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            response_format: if json {
                Some(ResponseFormat::JsonObject)
            } else {
                None
            },
            system_prompt: Some(system.to_string()),
            trace_id: None,
            tools: None,
        };
        match adapter.generate(request).await {
            Ok(resp) => {
                self.responses
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push((
                        kind.to_string(),
                        resp.content.chars().count(),
                        resp.content.chars().take(160).collect::<String>(),
                    ));
                Ok(resp.content)
            }
            Err(e) => {
                self.errors
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(format!("[{kind}] {e}"));
                Err(AppError::from(format!("真机模型调用失败: {e}")))
            }
        }
    }

    fn last_response(&self, kind: &str) -> Option<(usize, String)> {
        self.responses
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .rev()
            .find(|(k, _, _)| k == kind)
            .map(|(_, n, head)| (*n, head.clone()))
    }

    fn last_error(&self) -> Option<String> {
        self.errors
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last()
            .cloned()
    }

    fn last_user_prompt_of(&self, kind: &str) -> Option<String> {
        self.calls
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .rev()
            .find(|c| c.kind == kind)
            .map(|c| c.user.clone())
    }

    /// 每次调用实际服务的档位（kind → route），供探针打印路由证据。
    fn routes(&self) -> Vec<(String, &'static str)> {
        self.calls
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|c| (c.kind.clone(), c.route))
            .collect()
    }
}

#[async_trait::async_trait]
impl super::tool_loop::LoopLlm for ProbeLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        self.call(
            system_prompt,
            user_prompt,
            _task,
            max_tokens,
            false,
            "complete",
        )
        .await
    }

    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        self.call(
            system_prompt,
            user_prompt,
            _task,
            max_tokens,
            true,
            "complete_json",
        )
        .await
    }
}

/// 编辑档模型：env 覆盖 → `background_model_id`（应用把编辑路由到 Background
/// 档）→ 写作档；超时同样放宽到 300s。
fn resolve_editor_profile(cfg: &AppConfig, creative: &LlmProfile) -> LlmProfile {
    let mut picked = env_var(ENV_EDITOR_MODEL)
        .and_then(|want| pick_profile(cfg, Some(want.as_str())).ok())
        .or_else(|| {
            cfg.background_model_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|id| cfg.llm_profiles.get(id).cloned())
        })
        .unwrap_or_else(|| creative.clone());
    if picked.timeout_seconds < 300 {
        picked.timeout_seconds = 300;
    }
    picked
}

/// 生产档模型：env 覆盖 → `tool_model_id`（应用把 `agency_producer`
/// 的章节纲/规划/世界构建路由到 Tool 档）→ 写作档；超时同样放宽到 300s。
fn resolve_tool_profile(cfg: &AppConfig, creative: &LlmProfile) -> LlmProfile {
    let mut picked = env_var(ENV_TOOL_MODEL)
        .and_then(|want| pick_profile(cfg, Some(want.as_str())).ok())
        .or_else(|| {
            cfg.tool_model_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|id| cfg.llm_profiles.get(id).cloned())
        })
        .unwrap_or_else(|| creative.clone());
    if picked.timeout_seconds < 300 {
        picked.timeout_seconds = 300;
    }
    picked
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// 真机库 + 向量库复制到临时目录；返回 (副本目录, 副本库路径, 副本向量库路径)。
fn copy_real_state() -> (PathBuf, PathBuf, Option<PathBuf>) {
    let tmp = std::env::temp_dir().join(format!(
        "storymoss-probe-{}",
        chrono::Local::now().timestamp_millis()
    ));
    std::fs::create_dir_all(&tmp).expect("建临时目录");
    let src = real_db_src();
    assert!(src.exists(), "真机库不存在: {}", src.display());
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", src.display()));
        if from.exists() {
            std::fs::copy(&from, tmp.join(format!("cinema_ai.db{suffix}"))).expect("复制库");
        }
    }
    let src_vs = src.parent().map(|p| p.join("vector_db"));
    let dst_vs = tmp.join("vector_db");
    let vs = match src_vs {
        Some(dir) if dir.exists() => {
            copy_dir_all(&dir, &dst_vs).expect("复制向量库");
            Some(dst_vs)
        }
        _ => None,
    };
    (tmp.clone(), tmp.join("cinema_ai.db"), vs)
}

/// 最新有正文的故事（scene 数最多者）→ (story_id, scene_id, chapter_number,
/// content)。
fn pick_latest_story(pool: &DbPool) -> Option<(String, String, i32, String)> {
    let conn = pool.get().ok()?;
    conn.query_row(
        "SELECT s.story_id, s.id, s.sequence_number, COALESCE(s.content, '') \
         FROM scenes s JOIN stories st ON st.id = s.story_id \
         WHERE COALESCE(s.content,'') != '' \
         ORDER BY (SELECT COUNT(*) FROM scenes x WHERE x.story_id = s.story_id) DESC, \
                  st.updated_at DESC, s.sequence_number DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .ok()
}

/// 确保「当前章之前」的章节在副本向量库里有记录（缺失则以正文前 500 字补建，
/// 与生产投影形态一致），返回补建条数。
async fn ensure_prior_chapters_indexed(
    pool: &DbPool,
    store: &Arc<dyn crate::ports::VectorStore>,
    story_id: &str,
    current_chapter: i32,
) -> usize {
    let prior: Vec<(i32, String)> = {
        let Ok(conn) = pool.get() else {
            return 0;
        };
        let mut stmt = match conn.prepare(
            "SELECT sequence_number, COALESCE(content, '') FROM scenes \
             WHERE story_id = ?1 AND sequence_number < ?2 AND COALESCE(content,'') != '' \
             ORDER BY sequence_number",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return 0,
        };
        let rows = stmt.query_map(rusqlite::params![story_id, current_chapter], |r| {
            Ok((r.get::<_, i32>(0)?, r.get::<_, String>(1)?))
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => return 0,
        }
    };
    let mut built = 0usize;
    for (n, content) in prior {
        let plain = crate::utils::death_text::strip_editor_markup(&content);
        let text = format!(
            "第{n}章: {}",
            plain.trim().chars().take(500).collect::<String>()
        );
        let existing = match crate::embeddings::embed_text_async(text.clone()).await {
            Ok(emb) => store.search(story_id, emb, 1).await.unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let already = existing.iter().any(|r| {
            r.chapter_number == n && r.text.contains(&text.chars().take(24).collect::<String>())
        });
        if already {
            continue;
        }
        let Ok(embedding) = crate::embeddings::embed_text_async(text.clone()).await else {
            continue;
        };
        let record = crate::vector::VectorRecord {
            id: format!("{story_id}_probe_ch{n}"),
            story_id: story_id.to_string(),
            chapter_id: String::new(),
            chapter_number: n,
            text,
            record_type: "chapter_summary".to_string(),
            metadata: None,
            embedding,
        };
        if store.upsert(record).await.is_ok() {
            built += 1;
        }
    }
    built
}

/// 第 `n` 章正文里最长的高区分度片段（做检索指令用）。
fn distinctive_detail(pool: &DbPool, story_id: &str, n: i32) -> Option<String> {
    let conn = pool.get().ok()?;
    let content: String = conn
        .query_row(
            "SELECT COALESCE(content,'') FROM scenes WHERE story_id = ?1 AND sequence_number = ?2 \
             ORDER BY rowid LIMIT 1",
            rusqlite::params![story_id, n],
            |r| r.get(0),
        )
        .ok()?;
    let plain = crate::utils::death_text::strip_editor_markup(&content);
    crate::memory::continuity::distinctive_fragment(plain.trim(), 8)
}

/// 探针 1：真机续写——检索注入 + 手法落库 + 判官裁决落库（v0.66/0.68/0.69）。
#[tokio::test]
#[ignore]
async fn real_machine_probe_continue_retrieval_technique_agreement() {
    let Some(cfg) = real_config() else { return };
    let mut profile = match pick_profile(&cfg, env_var(ENV_MODEL).as_deref()) {
        Ok(p) => p,
        Err(e) => {
            println!("[真机探针] 跳过：{e}");
            return;
        }
    };
    // 真机慢模型（deepseek-flash 单次生成可到 90s+）：探针放宽单次超时。
    if profile.timeout_seconds < 300 {
        profile.timeout_seconds = 300;
    }
    println!(
        "[真机探针] 模型: id={} provider={:?} model={} base={:?}",
        profile.id, profile.provider, profile.model, profile.api_base
    );
    crate::embeddings::provider::init_global_provider(&cfg);
    // 本地 provider 回退路径会用到哈希嵌入缓存（与生产启动同款初始化）。
    crate::embeddings::embedding::init_embedding_model().ok();

    let (tmp, db_path, vs_path) = copy_real_state();
    println!("[真机探针] 副本目录: {}", tmp.display());
    let pool = crate::db::connection::init_db(&tmp, None).expect("打开副本库并迁移");

    let Some((story_id, scene_id, chapter, content)) = pick_latest_story(&pool) else {
        println!("[真机探针] 跳过：副本库里没有带正文的场景");
        return;
    };
    println!(
        "[真机探针] 目标：story={story_id} scene={scene_id} 第{chapter}章（{} 字）",
        content.chars().count()
    );

    // 向量库副本（没有就跳过检索断言）。
    let store: Option<Arc<dyn crate::ports::VectorStore>> = match vs_path {
        Some(path) => {
            let store: Arc<dyn crate::ports::VectorStore> = Arc::new(
                crate::vector::LanceVectorStore::new(path.to_string_lossy().to_string()),
            );
            match store.init().await {
                Ok(()) => Some(store),
                Err(e) => {
                    println!("[真机探针] 向量库副本初始化失败（检索断言将跳过）: {e}");
                    None
                }
            }
        }
        None => None,
    };
    let mut indexed = 0usize;
    if let Some(store) = &store {
        indexed = ensure_prior_chapters_indexed(&pool, store, &story_id, chapter).await;
        println!("[真机探针] 前序章索引补建 {indexed} 条（缺则从正文前 500 字补）");
    }

    // 指令带第 1 章的高区分度细节（逼近「写第 N 章时召回第 1 章」的真实场景）。
    let detail = distinctive_detail(&pool, &story_id, 1).unwrap_or_default();
    let instruction = if detail.is_empty() {
        "续写".to_string()
    } else {
        let short: String = detail.chars().take(30).collect();
        println!("[真机探针] 指令细节（第1章）: {short}");
        format!("续写，并呼应第一章的细节：{short}")
    };

    let editor_profile = resolve_editor_profile(&cfg, &profile);
    let tool_profile = resolve_tool_profile(&cfg, &profile);
    println!(
        "[真机探针] 生产档（章节纲/规划）模型: {}（{}）",
        tool_profile.model, tool_profile.id
    );
    println!(
        "[真机探针] 编辑档模型: {}（{}）",
        editor_profile.model, editor_profile.id
    );
    let llm = ProbeLlm::new(
        build_adapter(&profile),
        build_adapter(&tool_profile),
        build_adapter(&editor_profile),
        cfg.continuation_temperature.unwrap_or(profile.temperature),
    );
    let coordinator = match &store {
        Some(store) => {
            AgencyCoordinator::for_test_with_store(pool.clone(), llm.clone(), store.clone())
        }
        None => AgencyCoordinator::for_test(pool.clone(), llm.clone()),
    };
    let budget = Arc::new(crate::agency::budget::AgencyBudget::new(
        crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
    ));
    let _ = &budget;
    let started = std::time::Instant::now();
    let mut run_result = None;
    let mut used_run_id = String::new();
    for attempt in 1..=2 {
        let run_id = format!("probe-continue-{attempt}");
        let current = crate::db::repositories::SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .ok()
            .flatten()
            .and_then(|s| s.content)
            .unwrap_or_else(|| content.clone());
        match coordinator
            .run_continue(
                &run_id,
                &story_id,
                PersistMode::Append {
                    scene_id: scene_id.clone(),
                },
                &instruction,
                Some(&current),
            )
            .await
        {
            Ok(run) => {
                run_result = Some(run);
                used_run_id = run_id;
                break;
            }
            Err(e) => {
                println!("[真机探针] run_continue 第 {attempt} 次失败: {e}");
                // 失败路径不会关 run（生产由前端/超时收口）：探针显式释放 V109
                // 并发护栏。
                if let Ok(conn) = pool.get() {
                    let _ = conn.execute(
                        "UPDATE agency_runs SET status='failed', error_message=?2, updated_at=?3 WHERE id=?1",
                        rusqlite::params![run_id, e.to_string(), chrono::Local::now().to_rfc3339()],
                    );
                }
                if attempt == 2 {
                    println!("[真机探针] 两次均失败（模型侧瞬时故障）——本轮探针结束");
                    return;
                }
            }
        }
    }
    let run = match run_result {
        Some(run) => run,
        None => return,
    };
    let elapsed = started.elapsed();
    println!(
        "[真机探针] 续写完成：增量 {} 字，耗时 {:.1}s，总调用 {} 次",
        run.increment.chars().count(),
        elapsed.as_secs_f64(),
        llm.calls.lock().unwrap_or_else(|p| p.into_inner()).len()
    );

    // ---- v0.66.0：检索块出现在 writer prompt ----
    let writer_prompt = llm.last_user_prompt_of("complete").unwrap_or_default();
    let block_start = writer_prompt.find("【前文相关原文（检索");
    match block_start {
        Some(idx) => {
            let block: String = writer_prompt[idx..]
                .lines()
                .take(6)
                .collect::<Vec<_>>()
                .join(" / ");
            println!(
                "[真机探针] 检索块：{}",
                block.chars().take(400).collect::<String>()
            );
            assert!(
                block.contains("第1章") || block.contains("第2章"),
                "检索块应召回到当前章之前的章节"
            );
        }
        None => {
            if store.is_some() && indexed > 0 {
                panic!("向量库已补建索引但 writer prompt 未注入检索块");
            }
            println!("[真机探针] 检索块未注入（向量库副本不可用或该故事无索引）——按降级路径放行");
        }
    }

    // ---- v0.69.0：每拍手法落库 + 统计可读 ----
    // 落库走 detach 的 spawn_blocking（best-effort）：轮询等待至多 10s。
    let mut recent = Vec::new();
    for _ in 0..20 {
        recent = crate::agency::technique_feedback::recent_technique_ids(&pool, &story_id, 1)
            .unwrap_or_default();
        if !recent.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    println!("[真机探针] 本拍手法落库: {recent:?}");
    assert!(!recent.is_empty(), "每拍必须落一行手法");
    let stats =
        crate::agency::technique_feedback::technique_stats(&pool, &story_id).unwrap_or_default();
    println!(
        "[真机探针] 手法统计（8 条）：{}",
        stats
            .iter()
            .map(|s| format!("{}={}/{}", s.label, s.accepts, s.rejects))
            .collect::<Vec<_>>()
            .join(" ")
    );

    // ---- v0.68.0：同步跑一次质量门（后台 QC 在无 app_handle
    // 的测试环境被显式跳过）， 验证 gate → editor_gate_records → 判官一致率
    // 的落库链路（编辑调用走真机模型）----
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let draft = board
        .list_zone(&used_run_id, crate::agency::models::BoardZone::Draft)
        .unwrap_or_default()
        .into_iter()
        .filter(|i| i.item_type == "chapter" && i.status == "active")
        .next_back();
    match draft {
        Some(draft) => {
            let registry = Arc::new(crate::agency::tools::ToolRegistry::agency_default());
            let gate_budget = Arc::new(crate::agency::budget::AgencyBudget::new(
                crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
            ));
            let mut gate_failed_without_verdict = false;
            match coordinator
                .evaluate_gate(
                    &gate_budget,
                    &board,
                    &registry,
                    &used_run_id,
                    &story_id,
                    "续写《帝国的烟火》第3章",
                    &draft,
                    1,
                )
                .await
            {
                Ok(crate::agency::coordinator::GateOutcome::Failed { reason }) => {
                    println!(
                        "[真机探针] 质量门 Failed（模型未产出可解析裁决，应用真机由 Background 档承担）：{reason}"
                    );
                    gate_failed_without_verdict = true;
                }
                Ok(outcome) => println!("[真机探针] 质量门判定: {outcome:?}"),
                Err(e) => {
                    println!("[真机探针] 质量门失败（跳过一致率断言）: {e}");
                    gate_failed_without_verdict = true;
                }
            }
            // 落库走 detach 的 spawn_blocking（best-effort）：轮询等待至多
            // 10s， 与手法落库同款（v0.72.3：修探针读到「0
            // 行」的时序假象）。
            let mut gate_rows: i64 = 0;
            for _ in 0..20 {
                gate_rows = pool
                    .get()
                    .ok()
                    .and_then(|conn| {
                        conn.query_row(
                            "SELECT COUNT(*) FROM editor_gate_records WHERE story_id = ?1",
                            rusqlite::params![story_id],
                            |r| r.get::<_, i64>(0),
                        )
                        .ok()
                    })
                    .unwrap_or(0);
                if gate_rows > 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            println!("[真机探针] editor_gate_records 行数: {gate_rows}");
            if !gate_failed_without_verdict {
                assert!(
                    gate_rows > 0,
                    "质量门判定必须落 editor_gate_records（v0.68 链路）"
                );
            } else if gate_rows == 0 {
                println!("[真机探针] 编辑档模型未产出裁决 → 无 findings 可落（v0.68 链路已由单测覆盖；真机需 Background 档模型遵循 JSON）");
            }
            let report =
                crate::agency::editor_agreement::compute_editor_agreement(&pool, &story_id)
                    .unwrap_or_default();
            println!(
                "[真机探针] 判官一致率：命中 {} 条 / 可判 {} / 采纳 {}，总采纳率 {:.0}%",
                report.total_findings,
                report.total_judgeable,
                report.total_adopted,
                report.adoption_rate * 100.0
            );
            for c in &report.chapters {
                println!(
                    "  - 第{}章：命中 {} / 可判 {} / 采纳 {} / 你的修改率 {:?}",
                    c.chapter_number, c.findings, c.judgeable, c.adopted, c.modification_ratio
                );
            }
        }
        None => println!("[真机探针] 黑板草稿区没有 active chapter 项（跳过 gate 断言）"),
    }
}

/// 探针 2：真机卷纲生成 + 章纲注入（v0.70.0）。
#[tokio::test]
#[ignore]
async fn real_machine_probe_volume_plan_and_outline_injection() {
    let Some(cfg) = real_config() else { return };
    let mut profile = match pick_profile(&cfg, env_var(ENV_MODEL).as_deref()) {
        Ok(p) => p,
        Err(e) => {
            println!("[真机探针] 跳过：{e}");
            return;
        }
    };
    // 真机慢模型（deepseek-flash 单次生成可到 90s+）：探针放宽单次超时。
    if profile.timeout_seconds < 300 {
        profile.timeout_seconds = 300;
    }
    println!(
        "[真机探针] 模型: id={} provider={:?} model={}",
        profile.id, profile.provider, profile.model
    );
    let (tmp, _db, _vs) = copy_real_state();
    let pool = crate::db::connection::init_db(&tmp, None).expect("打开副本库并迁移");
    let Some((story_id, _scene, chapter, _content)) = pick_latest_story(&pool) else {
        println!("[真机探针] 跳过：没有带正文的场景");
        return;
    };
    let editor_profile = resolve_editor_profile(&cfg, &profile);
    let tool_profile = resolve_tool_profile(&cfg, &profile);
    println!(
        "[真机探针] 生产档（章节纲/规划）模型: {}（{}）",
        tool_profile.model, tool_profile.id
    );
    println!(
        "[真机探针] 编辑档模型: {}（{}）",
        editor_profile.model, editor_profile.id
    );
    let llm = ProbeLlm::new(
        build_adapter(&profile),
        build_adapter(&tool_profile),
        build_adapter(&editor_profile),
        cfg.continuation_temperature.unwrap_or(profile.temperature),
    );
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
    let budget = Arc::new(crate::agency::budget::AgencyBudget::new(
        crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
    ));

    let plan = match coordinator
        .generate_volume_plan(
            "probe-vol-1",
            &story_id,
            &budget,
            1,
            1,
            Some("本卷以镇北王府的夺印对峙与渡口易主收束"),
        )
        .await
    {
        Ok(plan) => plan,
        Err(e) => {
            println!("[真机探针] 卷纲生成失败（跳过）: {e}");
            return;
        }
    };
    println!(
        "[真机探针] 卷纲：第 {} 卷（第 {}–{} 章）\n  卷目标：{}\n  卷末高潮：{}\n  须兑现：{:?}\n  线索预算：{}",
        plan.volume_number,
        plan.start_chapter,
        plan.end_chapter,
        plan.goal,
        plan.climax,
        plan.foreshadowing_due,
        plan.new_thread_budget
    );
    assert!(!plan.goal.trim().is_empty(), "卷目标不得为空");
    assert!(!plan.climax.trim().is_empty(), "卷末高潮不得为空");
    assert!(plan.end_chapter >= plan.start_chapter);
    crate::story_system::volume_plan::save_volume_plan(&pool, &story_id, &plan)
        .expect("卷纲落库（副本）");

    // 章纲生成应带上本卷计划（v0.70 注入；真实模型实跑）。
    let outline = coordinator
        .generate_chapter_outline(
            "probe-outline-1",
            &story_id,
            "续写《帝国的烟火》下一章",
            &budget,
            chapter.max(3),
            "【故事大纲】",
            "",
        )
        .await;
    println!("[真机探针] 章纲产出 {} 字", outline.chars().count());
    if outline.trim().is_empty() {
        let why = llm
            .last_error()
            .unwrap_or_else(|| "无调用错误（应答过短被 <50 字门丢弃）".to_string());
        match llm.last_response("complete") {
            Some((n, head)) => {
                println!("[真机探针] 章纲产出为空（模型侧）：{why}｜应答 {n} 字，开头：{head}")
            }
            None => println!("[真机探针] 章纲产出为空（模型侧）：{why}｜无应答记录"),
        }
    }
    // v0.70 的契约是「章纲 prompt 必须带本卷计划」——prompt
    // 在本地组装，与模型是否 成功无关（本机模型 8192
    // 上下文可能装不下长提示，那是模型容量问题不是注入缺陷）。
    let outline_prompt = llm.last_user_prompt_of("complete").unwrap_or_default();
    assert!(
        outline_prompt.contains("【本卷计划（第 1 卷：第 1–"),
        "章纲 prompt 必须注入本卷计划（实际未找到）"
    );
    let goal_line = plan.goal.chars().take(12).collect::<String>();
    assert!(
        outline_prompt.contains(&goal_line),
        "章纲 prompt 应包含卷目标片段「{goal_line}」"
    );
    println!(
        "[真机探针] 章纲 prompt 已含本卷计划与卷目标 ✓（prompt {} 字，开头：{}）",
        outline_prompt.chars().count(),
        outline_prompt.chars().take(60).collect::<String>()
    );
}

/// 探针 3：真机纲层重规划预览（v0.67.0）。
#[tokio::test]
#[ignore]
async fn real_machine_probe_outline_replan_preview() {
    let Some(cfg) = real_config() else { return };
    let mut profile = match pick_profile(&cfg, env_var(ENV_MODEL).as_deref()) {
        Ok(p) => p,
        Err(e) => {
            println!("[真机探针] 跳过：{e}");
            return;
        }
    };
    // 真机慢模型（deepseek-flash 单次生成可到 90s+）：探针放宽单次超时。
    if profile.timeout_seconds < 300 {
        profile.timeout_seconds = 300;
    }
    let (tmp, _db, _vs) = copy_real_state();
    let pool = crate::db::connection::init_db(&tmp, None).expect("打开副本库并迁移");
    let Some((story_id, _scene, chapter, _content)) = pick_latest_story(&pool) else {
        println!("[真机探针] 跳过：没有带正文的场景");
        return;
    };
    let editor_profile = resolve_editor_profile(&cfg, &profile);
    let tool_profile = resolve_tool_profile(&cfg, &profile);
    println!(
        "[真机探针] 生产档（章节纲/规划）模型: {}（{}）",
        tool_profile.model, tool_profile.id
    );
    println!(
        "[真机探针] 编辑档模型: {}（{}）",
        editor_profile.model, editor_profile.id
    );
    let llm = ProbeLlm::new(
        build_adapter(&profile),
        build_adapter(&tool_profile),
        build_adapter(&editor_profile),
        cfg.continuation_temperature.unwrap_or(profile.temperature),
    );
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let report =
        match crate::agency::outline_replan::preview(&coordinator, &pool, &story_id, chapter, 3)
            .await
        {
            Ok(report) => report,
            Err(e) => {
                println!("[真机探针] 重规划预览失败（跳过）: {e}");
                return;
            }
        };
    println!(
        "[真机探针] 重规划候选 {} 章，本轮预览 {} 条",
        report.total,
        report.items.len()
    );
    if report.total == 0 {
        println!("[真机探针] 当前章之后没有「有章纲」的章——无需重规划（数据形态如此）");
        return;
    }
    for item in &report.items {
        println!(
            "  - 第{}章：旧纲 {} 字 → 新纲 {:?}",
            item.chapter_number,
            item.old_plan.chars().count(),
            item.new_plan
                .as_deref()
                .map(|p| format!("{} 字", p.chars().count()))
        );
    }
    let produced = report.items.iter().any(|i| {
        i.new_plan
            .as_deref()
            .map(|p| p.trim().chars().count() >= 50)
            .unwrap_or(false)
    });
    if produced {
        println!("[真机探针] 真机模型产出了可用的新章纲（≥50 字）✓");
    } else {
        let why = report
            .items
            .iter()
            .filter_map(|i| i.skipped.clone())
            .next()
            .unwrap_or_else(|| "模型应答过短".to_string());
        println!("[真机探针] 本轮预览未产出可用新章纲（模型侧，契约部分仍断言）：{why}");
    }
    // 预览不落库：副本库里的旧纲应保持原样。
    let unchanged =
        crate::agency::outline_replan::chapter_outline_candidates(&pool, &story_id, chapter)
            .unwrap_or_default();
    assert!(
        unchanged.iter().all(|c| report
            .items
            .iter()
            .any(|i| i.chapter_number == c.chapter_number)),
        "预览不得写入（章纲候选应仍是原有集合）"
    );
    println!("[真机探针] 预览未落库 ✓（勾选后才写）");
}

/// 探针 4：**角色换模矩阵**——决定 `creative/tool/background_model_id` 怎么换。
///
/// 对配置里的每个 profile 跑三条与生产同形的短任务（全部真机端点直连）：
/// 1) 连通：33 字提示 / 32 token（复刻应用启动时的模型健康探测，推理模型在
///    这类短任务上最易暴露「content 为空」）；
/// 2) **章节纲产出**：真机故事上下文 + `scene_outline` 同款 system
///    （`TaskType::Analysis`、2048 token；生产由 Tool 档承担）——≥50 字才算产出
///    （与 `generate_chapter_outline` 的门一致）；
/// 3) **编辑裁决 JSON**：直接调用生产函数
///    `editor_verdict_prose_fallback`（质量门最后一道兜底；
///    `agency_editor_auditor_system`
///    + 只输出 JSON 强约束 + `parse_lenient` 解析）——能解析出 `EditorVerdict`
///    才算「该模型可当编辑（Background）档」。
///
/// 只打印矩阵、不红（死端点/空产出记 0 字）；角色该怎么换由本矩阵证据决定，
/// 换完用探针 1/2 复跑验收（`STORYMOSS_PROBE_TOOL_MODEL` /
/// `STORYMOSS_PROBE_EDITOR_MODEL` 可先试穿再落地）。
#[tokio::test]
#[ignore]
async fn real_machine_probe_role_model_viability() {
    let Some(cfg) = real_config() else { return };
    let (tmp, _db, _vs) = copy_real_state();
    let pool = crate::db::connection::init_db(&tmp, None).expect("打开副本库并迁移");
    let Some((story_id, _scene, chapter, content)) = pick_latest_story(&pool) else {
        println!("[真机探针] 跳过：没有带正文的场景");
        return;
    };
    println!(
        "[真机探针] 角色换模矩阵：story={story_id} 第{chapter}章（草稿 {} 字）",
        content.chars().count()
    );

    // 与生产 `generate_chapter_outline`
    // 同源的上下文（故事大纲/人物/前序进度）。
    let story_outline: String = {
        use crate::db::repositories::StoryOutlineRepository;
        StoryOutlineRepository::new(pool.clone())
            .get_by_story(&story_id)
            .ok()
            .flatten()
            .map(|o| o.content.chars().take(800).collect())
            .unwrap_or_default()
    };
    let characters: String = {
        use crate::db::repositories::CharacterRepository;
        CharacterRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap_or_default()
            .iter()
            .take(8)
            .map(|c| {
                format!(
                    "- {}：性格{}",
                    c.name,
                    c.personality.as_deref().unwrap_or("-")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let outline_user = format!(
        "【故事大纲】\n{story_outline}\n\n【主要人物】\n{characters}\n\n【本章】第{chapter}章\n\n请生成本章大纲（200-400 字），只输出正文。"
    );
    let outline_system =
        "你是专业的小说场景规划师，只输出本场景的执行大纲，不要输出 JSON 或整本书弧线规划。";
    let budget = Arc::new(crate::agency::budget::AgencyBudget::new(
        crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
    ));

    let mut profiles: Vec<LlmProfile> = cfg.llm_profiles.values().cloned().collect();
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    let role_of = |id: &str| -> String {
        let mut roles = Vec::new();
        if cfg.creative_model_id.as_deref() == Some(id) {
            roles.push("当前=Creative(写作)");
        }
        if cfg.tool_model_id.as_deref() == Some(id) {
            roles.push("当前=Tool(章节纲/规划)");
        }
        if cfg.background_model_id.as_deref() == Some(id) {
            roles.push("当前=Background(编辑)");
        }
        if cfg.active_llm_profile.as_deref() == Some(id) {
            roles.push("当前=active");
        }
        if roles.is_empty() {
            "当前=未指派".to_string()
        } else {
            roles.join(" / ")
        }
    };

    for profile in &profiles {
        let mut p = profile.clone();
        if p.timeout_seconds < 300 {
            p.timeout_seconds = 300;
        }
        println!(
            "\n===== {} | model={} @ {:?} | {} =====",
            p.id,
            p.model,
            p.api_base,
            role_of(&p.id)
        );

        // (1) 连通（短任务）
        let started = std::time::Instant::now();
        let ping = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            build_adapter(&p).generate(GenerateRequest {
                prompt: "请回复两个字：收到".to_string(),
                max_tokens: Some(32),
                temperature: Some(0.2),
                system_prompt: Some("你是助手，直接回复用户要求的文字。".to_string()),
                ..Default::default()
            }),
        )
        .await;
        match ping {
            Ok(Ok(resp)) => println!(
                "  [连通] content {} 字 / {:.1}s：{}",
                resp.content.chars().count(),
                started.elapsed().as_secs_f64(),
                resp.content.chars().take(40).collect::<String>()
            ),
            Ok(Err(e)) => println!("  [连通] 失败：{e}"),
            Err(_) => println!("  [连通] 超时（>120s）"),
        }

        // (2) 章节纲产出（生产由 Tool 档承担）
        let started = std::time::Instant::now();
        let outline = tokio::time::timeout(
            std::time::Duration::from_secs(180),
            build_adapter(&p).generate(GenerateRequest {
                prompt: outline_user.clone(),
                max_tokens: Some(2048),
                temperature: Some(0.7),
                system_prompt: Some(outline_system.to_string()),
                ..Default::default()
            }),
        )
        .await;
        match outline {
            Ok(Ok(resp)) => {
                let n = resp.content.trim().chars().count();
                if n >= 50 {
                    println!(
                        "  [章纲] ✅ 产出 {n} 字 / {:.1}s：{}…",
                        started.elapsed().as_secs_f64(),
                        resp.content.chars().take(80).collect::<String>()
                    );
                } else {
                    println!(
                        "  [章纲] ❌ 仅 {n} 字（<50 字会被生产丢弃）/ {:.1}s：{:?}",
                        started.elapsed().as_secs_f64(),
                        resp.content.chars().take(60).collect::<String>()
                    );
                }
            }
            Ok(Err(e)) => println!("  [章纲] ❌ 失败：{e}"),
            Err(_) => println!("  [章纲] ❌ 超时（>180s）"),
        }

        // (3) 编辑裁决（生产函数：质量门的散文回退；
        // 能解析出裁决才算可当编辑档）
        let llm3 = ProbeLlm::new(
            build_adapter(&p),
            build_adapter(&p),
            build_adapter(&p),
            cfg.continuation_temperature.unwrap_or(0.5),
        );
        let llm_dyn: Arc<dyn super::tool_loop::LoopLlm> = llm3.clone();
        let draft = crate::agency::models::BoardItem::new(
            "probe-viability",
            &story_id,
            crate::agency::models::BoardZone::Draft,
            "chapter",
            format!("第{chapter}章"),
            content.clone(),
            "探针草稿",
            crate::agency::models::AgentRole::LeadWriter,
            "active",
        );
        let started = std::time::Instant::now();
        let verdict = tokio::time::timeout(
            std::time::Duration::from_secs(180),
            super::coordinator::editor_verdict_prose_fallback(
                &llm_dyn,
                &budget,
                &pool,
                &draft,
                "续写《帝国的烟火》下一章",
            ),
        )
        .await;
        match verdict {
            Ok(Ok(v)) => println!(
                "  [编辑裁决] ✅ 解析出裁决 / {:.1}s：verdict={} blocking={} score={:?} comments={}",
                started.elapsed().as_secs_f64(),
                v.verdict,
                v.blocking_issues.len(),
                v.score,
                v.comments.chars().take(40).collect::<String>()
            ),
            Ok(Err(e)) => {
                let head = llm3
                    .last_response("complete")
                    .map(|(n, h)| format!("应答 {n} 字：{h}"))
                    .unwrap_or_else(|| "无应答记录".to_string());
                println!("  [编辑裁决] ❌ 失败：{e}｜{head}");
            }
            Err(_) => println!("  [编辑裁决] ❌ 超时（>180s）"),
        }
    }
    println!("\n[真机探针] 矩阵完成：按 [章纲] 与 [编辑裁决] 两列的证据指定 tool_model_id / background_model_id。");
}
