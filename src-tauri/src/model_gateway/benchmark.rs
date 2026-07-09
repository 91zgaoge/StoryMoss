//! 流式基准测试模块（v0.15.0）
//!
//! 用 OpenAiAdapter::generate_stream 跑两套实测基准：
//! - 短任务（200 token 输入 / 150 token 输出，模拟摘要/意图识别）
//! - 长任务（600 token 输入 / 1200 token 输出，模拟 800 字续写）
//!
//! 流式回调记录第一个 chunk 时间戳作为真实 TTFB（替代旧的魔法数 duration-10）。
//! 持续 token/s 由后续 chunk 累计计算。

use std::time::Instant;

use crate::{
    config::settings::{AppConfig, LlmProfile},
    db::DbPool,
    llm::{
        adapter::{GenerateRequest, LlmAdapter},
        openai::OpenAiAdapter,
    },
    model_gateway::types::BenchmarkResult,
};

const SHORT_BENCHMARK_PROMPT: &str = concat!(
    "请将下面这段对话内容总结为一句话（不超过 50 字），只输出总结本身。\n",
    "对话：\n",
    "甲：这个季度的销售额下降了 12%，主要原因是华东区域供应链中断。\n",
    "乙：我们已经联系了新的供应商，预计下个月恢复。\n",
);

const LONG_BENCHMARK_PROMPT: &str = concat!(
    "你是一名优秀的小说写作助手。请根据以下已有内容，自然地续写下一段（约 500 字）。",
    "不要添加任何说明，直接输出续写内容。\n\n",
    "【已有内容】\n",
    "夜风穿过古城墙的残垣，发出呜咽般的低鸣。林思远站在断壁上，",
    "望着远方那片被晚霞染红的雪山。他攥紧了腰间那柄随身十年的青铜剑——",
    "剑身已经斑驳，剑柄上的红绳褪了色。\n",
    "\"该走了。\"身后传来沈青鸢的声音，清冷如山泉。",
    "她披着一件墨色斗篷，眉间一颗朱砂痣在夕阳下泛着微光。\n",
    "林思远没有回头：\"再等一会儿。\"\n",
    "风更大了。雪山深处隐隐传来鹰鸣。\n\n【续写】\n",
);

fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 * 0.25).ceil() as u32
}

pub struct StreamBenchmark {
    pool: DbPool,
}

impl StreamBenchmark {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn run_benchmark(&self, profile: &LlmProfile, is_long: bool) -> BenchmarkResult {
        let (prompt, max_tokens): (&str, i32) = if is_long {
            (LONG_BENCHMARK_PROMPT, 1200)
        } else {
            (SHORT_BENCHMARK_PROMPT, 150)
        };
        let input_tokens = estimate_tokens(prompt);

        let adapter = OpenAiAdapter::new(
            profile.api_key.clone(),
            profile.name.clone(),
            profile.api_base.clone(),
            max_tokens,
            profile.temperature,
            profile.timeout_seconds.max(10u64),
            10,
            60,
        );

        let request = GenerateRequest {
            prompt: prompt.to_string(),
            max_tokens: Some(max_tokens),
            temperature: Some(0.2),
            ..Default::default()
        };

        let overall_start = Instant::now();
        match adapter.generate_stream(request).await {
            Ok(mut rx) => {
                let mut total_tokens: u32 = 0;
                let mut first_chunk_at: Option<Instant> = None;
                loop {
                    match rx.recv().await {
                        Some(Ok(chunk)) => {
                            if first_chunk_at.is_none() {
                                first_chunk_at = Some(Instant::now());
                            }
                            total_tokens += (chunk.len() as f64 * 0.67).ceil() as u32;
                        }
                        Some(Err(e)) => {
                            log::warn!("[StreamBenchmark] chunk error for {}: {}", profile.id, e);
                            continue;
                        }
                        None => break,
                    }
                }
                let duration = overall_start.elapsed();
                let duration_ms = duration.as_millis() as u64;
                let ttfb_ms =
                    first_chunk_at.map(|t| t.duration_since(overall_start).as_millis() as u64);
                let sustained_tps = ttfb_ms.and_then(|ttfb| {
                    if total_tokens > 0 && duration_ms > ttfb {
                        Some(total_tokens as f64 / ((duration_ms - ttfb) as f64 / 1000.0))
                    } else {
                        None
                    }
                });
                BenchmarkResult {
                    success: true,
                    real_ttfb_ms: ttfb_ms,
                    duration_ms,
                    output_tokens: total_tokens,
                    input_tokens,
                    sustained_tps,
                    error: None,
                }
            }
            Err(e) => {
                let duration_ms = overall_start.elapsed().as_millis() as u64;
                BenchmarkResult {
                    success: false,
                    real_ttfb_ms: None,
                    duration_ms,
                    output_tokens: 0,
                    input_tokens,
                    sustained_tps: None,
                    error: Some(format!("{}", e)),
                }
            }
        }
    }

    /// 从 AppConfig 加载所有启用的 chat 类 profile
    pub fn load_enabled_profiles(app_dir: &std::path::Path) -> Vec<LlmProfile> {
        match AppConfig::load(app_dir) {
            Ok(config) => config
                .llm_profiles
                .into_values()
                .filter(|p| p.enabled)
                .collect(),
            Err(_) => vec![],
        }
    }
}
