//! Golden continue harness —— opt-in 的端到端续写质量测量（**不进 CI**）。
//!
//! ## 为什么在 `src/agency/` 而不是 `tests/`
//!
//! `tests/golden_continue_harness.rs` 这类集成测试只能看到 lib 的 `pub` 表面：
//! 要驱动真实续写需要 `LoopLlm`（签名引用 `crate::router::TaskType` /
//! `crate::error::AppError`，均为私有模块）、`AgencyCoordinator::for_test`、
//! `db::create_test_pool` 等 crate 内部件，集成测试无法构造。
//! 因此本 harness 以 `#[cfg(test)] mod` 形式放在 agency 内部（与
//! `src/agency/tests.rs` 同一测试构建），用 `#[ignore]` + 环境变量门控，
//! CI 永远不会执行它。
//!
//! ## 怎么跑
//!
//! ```bash
//! cd src-tauri
//! # 1) 指向已配置模型（profile id 或 model 名；必需，否则打印跳过原因并返回）
//! #    配置默认从真实 app 配置目录读取：
//! #    macOS  ~/Library/Application Support/com.storymoss.app
//! #    也可用 STORYMOSS_GOLDEN_CONFIG_DIR 指向别的目录
//! STORYMOSS_GOLDEN_MODEL=Qwen3.8-27B-Abliterated-8bit \
//!   cargo test --lib golden_continue -- --ignored --nocapture
//!
//! # 2) 落盘 metrics + 生成文本，便于逐轮 diff / 人工打分
//! STORYMOSS_GOLDEN_MODEL=<model> STORYMOSS_GOLDEN_OUT=/tmp/golden-1.json \
//!   cargo test --lib golden_continue -- --ignored --nocapture
//! ```
//!
//! 可选环境变量：
//! - `STORYMOSS_GOLDEN_PROFILE`  直接指定 profile id（优先于 MODEL）
//! - `STORYMOSS_GOLDEN_CONFIG_DIR` 覆盖配置目录
//! - `STORYMOSS_GOLDEN_BASE_URL` / `STORYMOSS_GOLDEN_API_KEY` 覆盖连接的
//!   endpoint / 密钥（密钥只用于建连，**绝不写进报告 JSON**）
//! - `STORYMOSS_GOLDEN_OUT`      报告 JSON 输出路径
//! - `STORYMOSS_GOLDEN_TIMEOUT_SECS` 单次 run 超时（默认 900）
//!
//! ## 它测什么（metrics）
//!
//! 固定 fixture（固定前提 + 3 个带情感属性的角色 + 固定 ~1200 字第一章正文
//! + 主线大纲 + 敌对关系 + 本场大纲）→ 真实 `run_continue(Append)` 一拍。报告：
//!
//! | 指标 | 含义 | 期望 |
//! | --- | --- | --- |
//! | `output_chars` | 本拍增量字数 | ≥200（落库门槛），接近目标拍长更好 |
//! | `self_repetition_ratio` | `trim_self_repetition` 裁剪比例 | < 0.08（超阈值会触发 anti-repeat 重试） |
//! | `prior_overlap_chars` / `prior_replay_detected` | 复述第一章正文的字数 | 0 / false |
//! | `opening_tail_replayed` | 是否原地复述开场末句 | false |
//! | `advanced` | ≥200 字 + 非复述 + 探针未报「未兑现必须改变」 | true |
//! | `probe_gaps` | `beat_state::probe_increment_ex` 的缺口 | 空数组 |
//! | `change_delta_honored` | 增量是否兑现本拍必须改变项 | true |
//!
//! 报告同时记录模型/耗时/每次 LLM 调用的空正文计数（推理模型把 token 烧在
//! `reasoning_content` 上时 `empty_responses > 0`），以及生成正文全文，
//! 供连续多轮 diff 与人工评分。
//!
//! ## 注意
//!
//! - 需要真实、已配置的模型与网络；跑一次通常几十秒到几分钟。
//! - 会读取用户真实配置（只读选择 profile）；若 StoryMoss 正在运行，SQLite
//!   可能存在锁竞争（有 busy_timeout 兜底）。
//! - 报告 JSON 只写 model / profile id / base url / 文本与指标，不写 api_key。
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;

use super::{coordinator::AgencyCoordinator, persist::PersistMode};
use crate::{
    config::settings::{AppConfig, LlmProfile, LlmProvider},
    db::{create_test_pool, repositories::SceneRepository},
    error::AppError,
    llm::{
        adapter::{GenerateRequest, LlmAdapter, ResponseFormat},
        anthropic::AnthropicAdapter,
        ollama::OllamaAdapter,
        openai::OpenAiAdapter,
    },
    router::TaskType,
    utils::text::TextUtils,
};

/// 环境变量：目标模型（profile id 或 model 名）。
const ENV_MODEL: &str = "STORYMOSS_GOLDEN_MODEL";
/// 环境变量：目标 profile id（优先于 [`ENV_MODEL`]）。
const ENV_PROFILE: &str = "STORYMOSS_GOLDEN_PROFILE";
/// 环境变量：配置目录覆盖。
const ENV_CONFIG_DIR: &str = "STORYMOSS_GOLDEN_CONFIG_DIR";
/// 环境变量：endpoint 覆盖。
const ENV_BASE_URL: &str = "STORYMOSS_GOLDEN_BASE_URL";
/// 环境变量：api key 覆盖（只用于建连，不进报告）。
const ENV_API_KEY: &str = "STORYMOSS_GOLDEN_API_KEY";
/// 环境变量：报告 JSON 输出路径。
const ENV_OUT: &str = "STORYMOSS_GOLDEN_OUT";
/// 环境变量：单次 run 超时秒数。
const ENV_TIMEOUT_SECS: &str = "STORYMOSS_GOLDEN_TIMEOUT_SECS";

/// 固定前提（与大纲/角色表同一世界观）。
const GOLDEN_PREMISE: &str = "青梧镇入冬后连灭七盏街灯，守夜人沈砚与药师阿苔沿着灯绳查向钟楼，\
     巡夜司校尉陆离带着一纸封文书横插进来。三人各怀旧债，镇上的夜比往年更冷。";

/// 固定第一章正文（~1200 字）。三名角色全部在场，末句留下未决状态。
const GOLDEN_OPENING: &str = concat!(
    "　　钟楼的铜舌在子时前一刻断了。没人听见它落地，只有守在石阶下的沈砚看见瓦缝里抖下一线灰。\n",
    "　　雨下了三个时辰。青梧镇的檐水顺着瓦当滴进石臼，声音空得像谁拿指节敲一具薄棺。沈砚今年二十三，做守夜人做过七年，认得镇上每一盏灯该亮的时辰，也认得上个月起，哪几盏灯再没亮过。他把灯绳一段段绕在左手腕上，绕到第七圈时停住——断口齐整，不是风撕的。\n",
    "　　“西街第三盏。”阿苔在药柜后面说，手里碾着一味发苦的根茎，药碾子转了半圈又停，“灯油也是满的。有人先剪绳，再添油。”她抬眼看沈砚，眼白里全是熬夜的血丝，“做这事的不是疯子，疯子不会添油。”\n",
    "　　“灯绳是新麻，绳心却掺了旧的。”阿苔把断头凑到灯下，指尖捻开一股，捻出几根发黄的纤维，“灯行的麻从来不掺旧。掺旧的是私坊，私坊的麻按斤称，称的时候要多抓一把压秤。”她说完就把那截麻丢进药碾子里，碾了两下，像是要把这句话也碾碎。她父亲留下的方子还压在药柜最下一层，纸角卷着，她至今没敢摊开。\n",
    "　　灯行的账房先生三天前死了，死在自家后院的井里，官府记的是失足。沈砚替他守过三夜灵，知道那人睡觉从不关窗，也知道井台边不该有那道拖痕。他没说。守夜人的规矩只有一条：看见不该看见的东西，先记住，再等它自己长出来。青梧镇每年入冬都要死一两个人，死在雪里的叫冻，死在井里的叫命，账本上从来都是一笔带过。\n",
    "　　沈砚没有立刻答话。他把铜钱从怀里取出来，一枚一枚数。七枚，正是七盏灯灭的那一夜之后，有人塞进他门缝的。铜钱上沾着同一种灰，灰里混着极细的金屑，风一吹就亮。他认得这种灰：去年冬天，巡夜司烧过一批没收来的私铸铜料，烧完的灰就是这个颜色。\n",
    "　　巡夜司是去年才在青梧镇设卡的。封文书用火漆压印，印面上是一只衔刀的夜枭，凡持此文书者夜入民宅不必叩门。镇上人怕巡夜司多过怕贼。沈砚替人写过三封信求情，两封被原样退回，一封换来了他父亲的死讯。那封信他收在灯罩底下，纸边被灯油熏得发黄，他每夜点灯都能看见。\n",
    "　　门是被推开的，不是被敲响的。陆离带着一身冷气进来，靴底在门槛上磕了两下，磕掉泥。他比沈砚矮半头，肩背却宽，巡夜司的封文书夹在腋下，用一根新的绳系着。\n",
    "　　陆离的靴底磕第二下泥的时候，阿苔已经把药碾子挪到了手边最近的位置。她认得这个人：去年封路那天，陆离在药铺门口站了一个时辰，最后什么药也没买，只把一张写着三个字的纸条留在柜台上。纸条上写的是她母亲的名字，还有一味她从未听说过的药。\n",
    "　　陆离开口，声音压得很低：“西街的灯，是你换的油。”\n",
    "　　阿苔的药碾子停了。石臼里那截根茎被碾断，断口渗出一点白汁。\n",
    "　　沈砚把七枚铜钱一枚枚摆到桌上，摆成一条短短的线。铜钱边缘的齿口对着陆离，像七只闭着的眼睛。他这才抬头，看着对方腋下那纸封文书，慢慢把手腕上的灯绳解下来，一圈，一圈，又一圈。\n",
    "　　“陆校尉，”他说，“你系文书的绳，和我手里这条，是同一股麻。”",
);
/// 固定角色表：(id, 姓名, 背景, 人格, 目标, 情感内核, 情感触发, 情感创伤,
/// 情感需求)。
const GOLDEN_CHARACTERS: [(&str, &str, &str, &str, &str, &str, &str, &str, &str); 3] = [
    (
        "gchar-shenyan",
        "沈砚",
        "青梧镇守夜人，做了七年，替人守夜也替人收尸。",
        "寡言，记性好，认死理。",
        "查清七盏灯是谁灭的，还清父亲留下的债。",
        "以守夜人的本分掩盖对父亲的恨。",
        "有人在他面前把责任推给死去的父亲",
        "十四岁那年父亲为还赌债把他抵给灯行",
        "被允许不欠任何人地活一次",
    ),
    (
        "gchar-atai",
        "阿苔",
        "镇东药铺的药师，外来人，用药极准，话极省。",
        "冷静，刻薄，护人护得不动声色。",
        "把父亲留下的旧方子验完，再决定要不要留在青梧镇。",
        "用替人治病来抵消当年没能救下母亲的自责。",
        "药铺里有人当着她的面拒绝服药等死",
        "母亲病重时她配错了最后一味药",
        "有人愿意让她把话说完",
    ),
    (
        "gchar-luli",
        "陆离",
        "巡夜司校尉，掌管青梧镇夜禁与私铸案。",
        "刚硬，守规矩，规矩之外寸步不让。",
        "把私铸铜料的案子结在青梧镇，赶在入冬封路前回司复命。",
        "把公事办完当成唯一能让自己睡着的东西。",
        "有人拿旧情要求他通融",
        "三年前他按律办了自己师门的案子",
        "有人告诉他，他做的是对的",
    ),
];

/// 固定主线大纲（`story_outlines.content`）。
const GOLDEN_STORY_OUTLINE: &str = "\
核心冲突：七盏街灯接连被灭，铜钱与私铸旧案把守夜人沈砚、药师阿苔、巡夜司校尉陆离拴在同一条灯绳上。\n\
第一幕：沈砚查灯，发现断口齐整、灯油被人补过；阿苔从药渣看出补油的人手上有旧伤。\n\
第二幕：陆离持封文书进镇，公开指认沈砚换油；三人第一次正面碰撞，各自隐瞒的那一部分旧事露出边角。\n\
第三幕：私铸案的经手人浮出水面，沈砚必须在还债与护人之间选一个。\n\
转折点：陆离发现文书上的印章是假的，而假印泥只出自镇东药铺。";

/// 固定第一章场景大纲（含 `【当前场大纲】` 与 `下一拍：`，驱动确定性 next
/// node）。
const GOLDEN_SCENE_OUTLINE: &str = "\
【当前场大纲】沈砚与阿苔在钟楼底层核对断灯绳与七枚铜钱，陆离持封文书闯入，\
三人就“谁添的灯油”第一次对峙。\n\
下一拍：陆离当众亮出封文书，逼沈砚交出铜钱并说出经手人";

/// 一次 LLM 调用的记录（报告用；不含 prompt 全文，避免报告过大）。
#[derive(Debug, Clone, Serialize)]
struct CallRecord {
    kind: &'static str,
    system_chars: usize,
    user_chars: usize,
    response_chars: usize,
    empty_response: bool,
    duration_ms: u128,
    error: Option<String>,
}

/// 真实模型 `LoopLlm`：把续写链路的两类调用转发到配置的适配器。
struct GoldenLlm {
    adapter: Box<dyn LlmAdapter>,
    temperature: f32,
    calls: Mutex<Vec<CallRecord>>,
}

impl GoldenLlm {
    async fn call(
        &self,
        system: &str,
        user: &str,
        max_tokens: i32,
        json: bool,
        kind: &'static str,
    ) -> Result<String, AppError> {
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
        let started = Instant::now();
        let outcome = self.adapter.generate(request).await;
        let record = CallRecord {
            kind,
            system_chars: system.chars().count(),
            user_chars: user.chars().count(),
            response_chars: outcome
                .as_ref()
                .map(|r| r.content.chars().count())
                .unwrap_or(0),
            empty_response: outcome
                .as_ref()
                .map(|r| r.content.trim().is_empty())
                .unwrap_or(false),
            duration_ms: started.elapsed().as_millis(),
            error: outcome.as_ref().err().map(|e| e.to_string()),
        };
        {
            let mut calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
            calls.push(record.clone());
        }
        // 空正文原样返回：让生产链路（write_beat_once 的过短重试 / 报错）
        // 按真实行为处理，harness 只负责记录。
        match outcome {
            Ok(resp) => Ok(resp.content),
            Err(e) => Err(AppError::from(format!("golden provider error: {e}"))),
        }
    }
}

#[async_trait::async_trait]
impl super::tool_loop::LoopLlm for GoldenLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        self.call(system_prompt, user_prompt, max_tokens, false, "complete")
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
            max_tokens,
            true,
            "complete_json",
        )
        .await
    }
}

/// 报告里的 probe 段。
#[derive(Debug, Clone, Serialize)]
struct ProbeReport {
    named_cast: usize,
    gaps: Vec<String>,
}

/// 质量指标（真实模型与 mock 自检共用同一计算路径）。
#[derive(Debug, Clone, Serialize)]
struct GoldenMetrics {
    /// 本拍增量字数。
    output_chars: usize,
    /// `trim_self_repetition` 的裁剪比例（≥0.08 会触发 anti-repeat 重试）。
    self_repetition_ratio: f64,
    self_repetition_would_retry: bool,
    /// 复述第一章正文的字数。
    prior_overlap_chars: usize,
    /// 是否复述了第一章正文（整段套用或前缀重叠 ≥25 归一化字）。
    prior_replay_detected: bool,
    /// 是否原地复述开场末句。
    opening_tail_replayed: bool,
    /// 本拍必须改变项（Rust 编译，0 LLM）。
    change_delta: String,
    /// 增量是否兑现必须改变项的关键词。
    change_delta_honored: bool,
    /// 综合判定：非复述 + 探针未报「未兑现必须改变」。
    advanced: bool,
    probe: ProbeReport,
    expected_cast: Vec<String>,
}

/// golden 报告（写入 `STORYMOSS_GOLDEN_OUT` 的 JSON）。
#[derive(Debug, Clone, Serialize)]
struct GoldenReport {
    harness: &'static str,
    started_at: String,
    story_id: String,
    profile_id: String,
    provider: String,
    model: String,
    api_base: Option<String>,
    /// 固定 fixture 的开篇字数（生成文本一并落盘，便于人工对比/评分）。
    opening_chars: usize,
    /// run 元数据。
    duration_ms: u128,
    run_error: Option<String>,
    llm_calls: Vec<CallRecord>,
    #[serde(flatten)]
    metrics: GoldenMetrics,
    generated_text: String,
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// 真实 app 配置目录候选（macOS/Linux/Windows 通用：`data_dir/<bundle id>`）。
fn app_config_dir_candidates() -> Vec<PathBuf> {
    if let Some(dir) = env_var(ENV_CONFIG_DIR) {
        return vec![PathBuf::from(dir)];
    }
    let mut out = Vec::new();
    if let Some(data) = dirs::data_dir() {
        out.push(data.join("com.storymoss.app"));
    }
    out
}

/// 在候选目录里挑第一个含配置文件/配置库的目录。
fn resolve_config_dir() -> Option<PathBuf> {
    let explicit = env_var(ENV_CONFIG_DIR).is_some();
    for dir in app_config_dir_candidates() {
        if explicit || dir.join("config.json").exists() || dir.join("cinema_ai.db").exists() {
            return Some(dir);
        }
    }
    app_config_dir_candidates().into_iter().next()
}

fn load_config() -> Option<(AppConfig, PathBuf)> {
    let dir = resolve_config_dir()?;
    match AppConfig::load(&dir) {
        Ok(cfg) => Some((cfg, dir)),
        Err(e) => {
            eprintln!("[golden] 读取配置失败 dir={dir:?}: {e}");
            None
        }
    }
}

/// 从配置里挑选目标 profile：`STORYMOSS_GOLDEN_PROFILE`（id）优先，
/// 其次 `STORYMOSS_GOLDEN_MODEL`（id 精确 → model 精确 → model
/// 包含，忽略大小写）。
fn select_profile(
    cfg: &AppConfig,
    wanted_profile: Option<&str>,
    wanted_model: Option<&str>,
) -> Result<LlmProfile, String> {
    let mut all: Vec<LlmProfile> = cfg.llm_profiles.values().cloned().collect();
    all.sort_by(|a, b| a.id.cmp(&b.id));
    if all.is_empty() {
        return Err("配置里没有任何 llm_profiles".to_string());
    }
    if let Some(id) = wanted_profile {
        return all
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("找不到 profile id={id}"));
    }
    let Some(wanted) = wanted_model else {
        return Err("未指定模型".to_string());
    };
    let lower = wanted.to_lowercase();
    let pick = all
        .iter()
        .find(|p| p.id == wanted)
        .or_else(|| all.iter().find(|p| p.model.eq_ignore_ascii_case(&lower)))
        .or_else(|| all.iter().find(|p| p.model.to_lowercase().contains(&lower)))
        .or_else(|| {
            // provider:model 形式（例如 openai:gemma4-e2b）
            lower.split_once(':').and_then(|(provider, model)| {
                all.iter().find(|p| {
                    p.provider.to_string().eq_ignore_ascii_case(provider)
                        && p.model.to_lowercase().contains(model)
                })
            })
        });
    match pick {
        Some(p) => Ok(p.clone()),
        None => Err(format!(
            "STORYMOSS_GOLDEN_MODEL={wanted} 未匹配任何 profile。可用：{}",
            all.iter()
                .map(|p| format!("{}[{}|{}]", p.id, p.provider, p.model))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn apply_env_overrides(profile: &mut LlmProfile) {
    if let Some(base) = env_var(ENV_BASE_URL) {
        profile.api_base = Some(base);
    }
    if let Some(key) = env_var(ENV_API_KEY) {
        profile.api_key = key;
    }
}

/// 按 provider 构造真实适配器（与生产 `LlmService` 同一批适配器实现）。
fn build_adapter(profile: &LlmProfile) -> Result<Box<dyn LlmAdapter>, String> {
    let max_tokens = if profile.max_tokens > 0 {
        profile.max_tokens
    } else {
        8192
    };
    let adapter: Box<dyn LlmAdapter> = match profile.provider {
        LlmProvider::Anthropic => Box::new(AnthropicAdapter::new(
            profile.api_key.clone(),
            profile.model.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds,
            10,
            60,
        )),
        LlmProvider::Ollama => Box::new(OllamaAdapter::new(
            profile.api_key.clone(),
            profile.model.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds,
            10,
            60,
        )),
        // OpenAI 兼容（含 Azure/DeepSeek/Qwen/Custom —— 项目内 profile 都按
        // OpenAI 兼容协议配置，见真机三档 Qwren127 / Gemma 4 / Qwen 3.8）。
        _ => Box::new(OpenAiAdapter::new(
            profile.api_key.clone(),
            profile.model.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds,
            10,
            60,
        )),
    };
    Ok(adapter)
}

/// 固定 fixture 的 id。
struct Fixture {
    story_id: String,
    scene_id: String,
}

/// 在临时 DB 里建固定故事：角色（含情感四元组）+ 敌对关系 + 世界观 +
/// 主线大纲 + 第一章（开篇正文 + 当前场大纲）。
fn seed_fixture(pool: &crate::db::DbPool) -> Fixture {
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "golden 固定样本".into(),
            description: Some(GOLDEN_PREMISE.into()),
            genre: Some("悬疑".into()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .expect("seed story");
    let story_id = story.id.clone();
    {
        let conn = pool.get().expect("seed conn");
        for (id, name, background, personality, goals, core, trigger, wound, need) in
            GOLDEN_CHARACTERS
        {
            conn.execute(
                "INSERT INTO characters (id, story_id, name, background, personality, goals, \
                 source, is_auto_generated, created_at, updated_at, emotional_core, \
                 emotional_trigger, emotional_wound, emotional_need) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'golden_harness', 0, '2026-01-01', '2026-01-01', \
                 ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    id,
                    story_id,
                    name,
                    background,
                    personality,
                    goals,
                    core,
                    trigger,
                    wound,
                    need,
                ],
            )
            .expect("seed character");
        }
        // 敌对关系：让 compile_conflict 产出「加压：陆离 与 沈砚 正面对峙」，
        // 从而 change_delta 有可检验的必须改变项。
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, \
             target_character_id, relationship_type, description, dynamic, emotional_bond, \
             emotional_intensity, reverse_emotional_bond, reverse_emotional_intensity, \
             created_at) VALUES ('grel-1', ?1, 'gchar-luli', 'gchar-shenyan', '对立', \
             '巡夜司对守夜人的例行盘查里夹着旧怨', '紧绷', '戒备与负罪', 8, '怨怼与依赖', 7, \
             '2026-01-01')",
            rusqlite::params![story_id],
        )
        .expect("seed relationship");
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, \
             source, is_auto_generated, created_at, updated_at) \
             VALUES ('gwb-1', ?1, '青梧镇：冬夜靠街灯与巡夜司维持秩序；私铸铜料是镇上的旧伤。', \
             '[\"街灯由守夜人换油，灯绳由灯行供货\",\"巡夜司持封文书可夜入民宅\",\"入冬封路后物资断绝\"]', \
             '去年冬天巡夜司查抄私铸铜料，烧了三天', '[]', 'golden_harness', 0, '2026-01-01', \
             '2026-01-01')",
            rusqlite::params![story_id],
        )
        .expect("seed world");
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, \
             total_scenes_estimate, created_at, updated_at) \
             VALUES ('gso-1', ?1, ?2, NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story_id, GOLDEN_STORY_OUTLINE],
        )
        .expect("seed story outline");
    }
    let scene_repo = SceneRepository::new(pool.clone());
    let ch1 = scene_repo
        .create(&story_id, 1, Some("第一章 铜舌断了"))
        .expect("seed scene");
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some(GOLDEN_OPENING.to_string()),
                outline_content: Some(GOLDEN_SCENE_OUTLINE.to_string()),
                setting_location: Some("青梧镇·钟楼底层".to_string()),
                ..Default::default()
            },
        )
        .expect("seed scene content");
    Fixture {
        story_id,
        scene_id: ch1.id,
    }
}

/// 归一化（去空白），用于"是否复述末句"这类包含判断。
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 本拍必须改变项的关键词（近似 `beat_state::change_delta_tokens`，
/// 用于报告"是否兑现"；判定本身仍以 probe 为准）。
fn delta_tokens(summary: &str) -> Vec<String> {
    const SKIP: &[&str] = &["必须", "本拍", "不得", "只靠", "对话", "过渡", "禁止", "与"];
    summary
        .split(|c: char| c.is_whitespace() || "，。；：、·—－-「」\"\"''（）()【】".contains(c))
        .map(str::trim)
        .filter(|s| s.chars().count() >= 2 && !SKIP.contains(s))
        .map(str::to_string)
        .collect()
}

/// 从落库正文 + fixture 计算质量指标（0 LLM，纯函数 + 一次 beat card 编译）。
fn compute_metrics(
    pool: &crate::db::DbPool,
    story_id: &str,
    opening: &str,
    increment: &str,
) -> GoldenMetrics {
    let output_chars = increment.chars().count();
    let trimmed = TextUtils::trim_self_repetition(increment);
    let trim_ratio =
        crate::agents::trim_utils::compute_trim_ratio(output_chars, trimmed.chars().count()) as f64;
    let stripped = TextUtils::strip_existing_overlap(increment, opening);
    let inc_compact = compact(increment);
    let open_compact = compact(opening);
    // `strip_existing_overlap` 在"几乎全是复述"时故意原样返回（<10 字残余
    // 不剥离），所以再补一条整段包含判断，避免全复述被误判成新内容。
    let fully_contained = inc_compact.chars().count() >= 20 && open_compact.contains(&inc_compact);
    let partially_stripped = stripped != increment;
    let prior_replay_detected = fully_contained || partially_stripped;
    let prior_overlap_chars = if partially_stripped {
        output_chars.saturating_sub(stripped.chars().count())
    } else if fully_contained {
        output_chars
    } else {
        0
    };
    // 复述开场末句：取开篇最后 60 字（归一化）判断是否原样出现在增量里。
    let opening_tail: String = {
        let total = opening.chars().count();
        opening.chars().skip(total.saturating_sub(60)).collect()
    };
    let opening_tail_replayed =
        !opening_tail.trim().is_empty() && inc_compact.contains(&compact(&opening_tail));

    let card = crate::agency::beat_card::compile_beat_card_located(
        pool,
        story_id,
        opening,
        Some("青梧镇·钟楼底层"),
    )
    .expect("compile beat card");
    let prior_tail = crate::agency::continue_assets::prior_tail_for_cast(opening);
    let state = crate::agency::beat_state::compile_beat_state(
        &card.cast.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        card.setting_location.as_deref(),
        &card.next_outline_node,
        &[],
        &prior_tail,
        &[],
    );
    // lock=None：与 write_beat_once 的 lock 参数相比只少了导演锁的三类
    // 缺口（同一人双身体/亲缘写反/死者行动），核心缺口与必须改变项一致。
    let probe = crate::agency::beat_state::probe_increment_ex(
        increment,
        &card,
        &state,
        &card.expansion_quota,
        None,
        &prior_tail,
        "novel",
    );
    let change_delta = format!(
        "{}：{}",
        card.change_delta.kind.as_zh(),
        card.change_delta.summary
    );
    let tokens = delta_tokens(&card.change_delta.summary);
    let change_delta_honored = tokens.is_empty() || tokens.iter().any(|t| increment.contains(t));
    let recap_gap = probe.gaps.iter().any(|g| g.contains("未兑现必须改变"));
    // 推进 = 有实质增量（≥落库门槛 200 字）+ 不复述 +
    // 探针未报「未兑现必须改变」。 空正文/过短正文（provider
    // 失败后的降级）绝不算推进。
    let advanced =
        output_chars >= 200 && !recap_gap && !prior_replay_detected && !opening_tail_replayed;
    GoldenMetrics {
        output_chars,
        self_repetition_ratio: trim_ratio,
        self_repetition_would_retry: crate::agents::trim_utils::should_retry_self_repetition(
            trim_ratio as f32,
            output_chars,
        ),
        prior_overlap_chars,
        prior_replay_detected,
        opening_tail_replayed,
        change_delta,
        change_delta_honored,
        advanced,
        probe: ProbeReport {
            named_cast: probe.named_cast,
            gaps: probe.gaps,
        },
        expected_cast: card.cast.iter().map(|c| c.name.clone()).collect(),
    }
}

/// 计算 + 打印 + （可选）落盘报告。
fn emit_report(report: GoldenReport, increment: &str) -> Result<(), String> {
    let out = env_var(ENV_OUT);
    if let Some(path) = out {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("创建报告目录失败 {parent:?}: {e}"))?;
            }
        }
        let json =
            serde_json::to_string_pretty(&report).map_err(|e| format!("序列化报告失败: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("写报告失败 {path:?}: {e}"))?;
        eprintln!("[golden] 报告已写入 {}", path.display());
    }
    // 打印人类可读摘要（--nocapture 可见）。
    eprintln!("================ golden continue report ================");
    eprintln!(
        "profile={} provider={} model={} api_base={:?}",
        report.profile_id, report.provider, report.model, report.api_base
    );
    eprintln!(
        "duration_ms={} llm_calls={} empty_responses={} run_error={:?}",
        report.duration_ms,
        report.llm_calls.len(),
        report.llm_calls.iter().filter(|c| c.empty_response).count(),
        report.run_error
    );
    let m = &report.metrics;
    eprintln!(
        "output_chars={} self_repetition_ratio={:.4} retry_would_trigger={}",
        m.output_chars, m.self_repetition_ratio, m.self_repetition_would_retry
    );
    eprintln!(
        "prior_overlap_chars={} prior_replay={} opening_tail_replayed={} advanced={}",
        m.prior_overlap_chars, m.prior_replay_detected, m.opening_tail_replayed, m.advanced
    );
    eprintln!(
        "change_delta={} honored={}",
        m.change_delta, m.change_delta_honored
    );
    eprintln!(
        "probe: named_cast={} gaps={:?}",
        m.probe.named_cast, m.probe.gaps
    );
    eprintln!("expected_cast={:?}", m.expected_cast);
    eprintln!(
        "---------------- generated text ({} chars) ----------------",
        increment.chars().count()
    );
    eprintln!("{increment}");
    eprintln!("-----------------------------------------------------------");
    eprintln!("opening_chars={}", report.opening_chars);
    Ok(())
}

/// Golden continue harness 主体：固定 fixture + 真实模型续写一拍 + 质量指标。
///
/// `#[ignore]`：CI 永不执行；需要环境变量与真实模型，见模块文档。
#[tokio::test]
#[ignore = "opt-in golden measurement: needs a real configured model (STORYMOSS_GOLDEN_MODEL)"]
async fn golden_continue_metrics_report() {
    let wanted_profile = env_var(ENV_PROFILE);
    let wanted_model = env_var(ENV_MODEL);
    if wanted_profile.is_none() && wanted_model.is_none() {
        eprintln!(
            "[golden] 跳过：未设置 {ENV_MODEL}（或 {ENV_PROFILE}）。\
             例：STORYMOSS_GOLDEN_MODEL=Qwen3.8-27B-Abliterated-8bit \
             cargo test --lib golden_continue -- --ignored --nocapture"
        );
        return;
    }
    let Some((cfg, config_dir)) = load_config() else {
        eprintln!("[golden] 跳过：读不到 app 配置。可用 {ENV_CONFIG_DIR} 指定目录");
        return;
    };
    let mut profile = match select_profile(&cfg, wanted_profile.as_deref(), wanted_model.as_deref())
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[golden] 跳过：{e}");
            return;
        }
    };
    apply_env_overrides(&mut profile);
    eprintln!(
        "[golden] config_dir={} profile={} model={} provider={}",
        config_dir.display(),
        profile.id,
        profile.model,
        profile.provider
    );
    let adapter = match build_adapter(&profile) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[golden] 跳过：{e}");
            return;
        }
    };

    let temperature = cfg.continuation_temperature.unwrap_or(profile.temperature);
    let llm = Arc::new(GoldenLlm {
        adapter,
        temperature,
        calls: Mutex::new(Vec::new()),
    });

    // ===== 固定 fixture（临时内存 DB，每次运行完全一致）=====
    let pool = create_test_pool().expect("test pool");
    let fixture = seed_fixture(&pool);
    eprintln!(
        "[golden] fixture story={} scene={} opening_chars={}",
        fixture.story_id,
        fixture.scene_id,
        GOLDEN_OPENING.chars().count()
    );

    let started_at = chrono::Local::now().to_rfc3339();
    let timeout_secs: u64 = env_var(ENV_TIMEOUT_SECS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(900);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
    let started = Instant::now();
    let run = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        coordinator.run_continue(
            "golden-run-1",
            &fixture.story_id,
            PersistMode::Append {
                scene_id: fixture.scene_id.clone(),
            },
            "续写",
            Some(GOLDEN_OPENING),
        ),
    )
    .await;
    let duration_ms = started.elapsed().as_millis();

    let (run_error, increment) = match run {
        Err(_) => (
            Some(format!("run 超时（{timeout_secs}s）；harness 不判定质量")),
            String::new(),
        ),
        Ok(Ok(result)) => (None, result.increment),
        Ok(Err(e)) => (Some(format!("run_continue 失败: {e}")), String::new()),
    };

    // run 失败时也从库里取回落库正文（附录/降级路径可能已写入）。
    let increment = if increment.trim().is_empty() {
        SceneRepository::new(pool.clone())
            .get_by_id(&fixture.scene_id)
            .ok()
            .flatten()
            .and_then(|s| s.content)
            .map(|c| {
                c.strip_prefix(GOLDEN_OPENING)
                    .map(|rest| rest.to_string())
                    .unwrap_or_default()
            })
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(increment)
    } else {
        increment
    };

    // ===== 质量指标 =====
    let metrics = compute_metrics(&pool, &fixture.story_id, GOLDEN_OPENING, &increment);

    let report = GoldenReport {
        harness: "golden_continue_harness",
        started_at,
        story_id: fixture.story_id.clone(),
        profile_id: profile.id.clone(),
        provider: profile.provider.to_string(),
        model: profile.model.clone(),
        api_base: profile.api_base.clone(),
        opening_chars: GOLDEN_OPENING.chars().count(),
        duration_ms,
        run_error,
        llm_calls: llm.calls.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        metrics,
        generated_text: increment.clone(),
    };

    // 报告失败不致命（无 STORYMOSS_GOLDEN_OUT 时连目录都不碰）。
    if let Err(e) = emit_report(report, &increment) {
        eprintln!("[golden] 报告写出失败：{e}");
    }
}

/// 固定 fixture 的 mock 正文：推进一拍（不新编角色、离开当前场景、兑现冲突
/// 加压），用于在 CI 里守护 harness 自身（fixture 可用 + 指标计算不自欺）。
const MOCK_BEAT_PROSE: &str = concat!(
    "　　陆离把封文书拍到柜台上，绳头弹起来，扫过那七枚铜钱。“巡夜司今夜起接管钟楼。”\n",
    "　　沈砚没有让开。他往前半步，肩膀抵住对方的肩，雨声从门缝里挤进来，把两个人的呼吸压在同一个节拍上。“文书上的印，”他说，“是湿的。”\n",
    "　　陆离的指节在封皮上收紧了一瞬。这一瞬就是全部的对峙——他和沈砚之间隔着一张柜台，也隔着三年前那桩没人肯提的旧案，赌注在谁先眨眼。\n",
    "　　阿苔已经绕到药柜内侧。她离开柜台时碰倒了石臼，碾到一半的根茎滚出来，白汁洒在铜钱上，齿口里那层金屑被冲开一线——不是新铸的铜，是旧钱刮出来的。\n",
    "　　陆离的目光在那道金线上停了半息。他忽然把文书收回腋下，转身走向门口，靴底磕在门槛上：“跟我走。私铸案的经手人还活着，他手上有一模一样的灰。”",
);
/// 脚本化 mock LLM：只服务 harness 自检，队列耗尽即报错（与生产 mock 同形）。
struct ScriptedLlm {
    responses: Mutex<std::collections::VecDeque<String>>,
}

#[async_trait::async_trait]
impl super::tool_loop::LoopLlm for ScriptedLlm {
    async fn complete(
        &self,
        _system_prompt: &str,
        _user_prompt: &str,
        _task: TaskType,
        _max_tokens: i32,
    ) -> Result<String, AppError> {
        self.responses
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front()
            .ok_or_else(|| AppError::validation_failed("scripted llm exhausted", None::<String>))
    }
}

/// harness 自检（**随 CI 跑**）：固定 fixture 能被真实续写链路消费，
/// 指标对"推进的一拍"给出 advanced、对"复述整段正文"给出 replay。
///
/// 这不是质量门（正文是脚本化的）；它守护的是 harness 本身不烂掉：
/// fixture 建得起来、beat card/probe/指标接线正确、报告可序列化。
#[tokio::test]
async fn golden_fixture_metrics_detect_advance_and_replay() {
    let pool = create_test_pool().expect("test pool");
    let fixture = seed_fixture(&pool);

    // 1) 指标自检：整段复述开场正文必须被判为 replay（不得侥幸通过）。
    let recap: String = GOLDEN_OPENING
        .chars()
        .skip(GOLDEN_OPENING.chars().count() - 200)
        .collect();
    let recap_metrics = compute_metrics(&pool, &fixture.story_id, GOLDEN_OPENING, &recap);
    assert!(
        recap_metrics.prior_replay_detected,
        "整段复述不得被算成新内容"
    );
    assert!(recap_metrics.opening_tail_replayed);
    assert!(!recap_metrics.advanced, "复述不得判为推进");

    // 1b) 空正文（provider 失败后的降级）绝不算推进。
    let empty_metrics = compute_metrics(&pool, &fixture.story_id, GOLDEN_OPENING, "");
    assert_eq!(empty_metrics.output_chars, 0);
    assert!(!empty_metrics.advanced, "空正文不得判为推进");

    // 2) 固定 fixture 走真实续写链路（脚本化正文），指标应判为推进。
    let llm = Arc::new(ScriptedLlm {
        responses: Mutex::new(std::collections::VecDeque::from(vec![
            MOCK_BEAT_PROSE.to_string()
        ])),
    });
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let result = coordinator
        .run_continue(
            "golden-mock-1",
            &fixture.story_id,
            PersistMode::Append {
                scene_id: fixture.scene_id.clone(),
            },
            "续写",
            Some(GOLDEN_OPENING),
        )
        .await
        .expect("固定 fixture 必须能走完 Append 续写");
    assert!(
        result.increment.chars().count() >= 200,
        "fixture 的脚本正文需满足落库门槛: {}",
        result.increment.chars().count()
    );
    let metrics = compute_metrics(&pool, &fixture.story_id, GOLDEN_OPENING, &result.increment);
    assert!(metrics.output_chars >= 200);
    assert!(!metrics.prior_replay_detected, "新正文不得判为复述");
    assert!(!metrics.opening_tail_replayed);
    assert!(metrics.change_delta_honored, "脚本正文兑现了加压/对峙");
    assert!(
        metrics.advanced,
        "推进的一拍应判为 advanced；probe gaps={:?}",
        metrics.probe.gaps
    );
    assert!(
        metrics.probe.gaps.is_empty(),
        "固定 fixture + 脚本正文不应有探针缺口: {:?}",
        metrics.probe.gaps
    );
    assert!(
        metrics.expected_cast.iter().any(|n| n == "陆离")
            && metrics.expected_cast.iter().any(|n| n == "沈砚"),
        "卡阵容应包含开场在场的角色: {:?}",
        metrics.expected_cast
    );

    // 3) 报告可序列化，且带生成文本与指标（人工打分的最小输入）。
    let report = GoldenReport {
        harness: "golden_continue_harness",
        started_at: "self-check".into(),
        story_id: fixture.story_id.clone(),
        profile_id: "mock".into(),
        provider: "mock".into(),
        model: "mock".into(),
        api_base: None,
        opening_chars: GOLDEN_OPENING.chars().count(),
        duration_ms: 1,
        run_error: None,
        llm_calls: vec![],
        metrics,
        generated_text: result.increment.clone(),
    };
    let json = serde_json::to_string_pretty(&report).expect("报告必须可序列化");
    assert!(json.contains("\"generated_text\""));
    assert!(json.contains("\"advanced\""));
    assert!(json.contains("陆离"));
}

/// 固定 fixture 自身的守卫：开篇约 1200 字、含三名角色、章节/大纲落库。
#[test]
fn golden_fixture_is_fixed_and_complete() {
    let pool = create_test_pool().expect("test pool");
    let fixture = seed_fixture(&pool);
    let opening_chars = GOLDEN_OPENING.chars().count();
    assert!(
        (1100..1500).contains(&opening_chars),
        "固定开篇应约 1200 字（当前 {opening_chars}）"
    );
    for (_, name, ..) in GOLDEN_CHARACTERS {
        assert!(GOLDEN_OPENING.contains(name), "开篇应含角色 {name}");
    }
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&fixture.scene_id)
        .expect("scene lookup")
        .expect("chapter 1 exists");
    assert_eq!(scene.sequence_number, 1);
    assert_eq!(scene.content.as_deref(), Some(GOLDEN_OPENING));
    let card = crate::agency::beat_card::compile_beat_card_located(
        &pool,
        &fixture.story_id,
        GOLDEN_OPENING,
        Some("青梧镇·钟楼底层"),
    )
    .expect("beat card compiles");
    assert!(
        card.cast.len() >= 3,
        "开篇三名在场角色都应进阵容: {:?}",
        card.cast
    );
    assert!(
        card.next_outline_node.contains("封文书") || card.next_outline_node.contains("铜钱"),
        "next node 应来自固定场景大纲的「下一拍」: {}",
        card.next_outline_node
    );
    assert!(card.change_delta.summary.contains("加压"));
}
