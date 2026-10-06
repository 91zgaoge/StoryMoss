#![allow(dead_code)]
//! 指导书提炼器：LLM 状态机
//!
//! 流程：元信息（→10%）→ 分块提炼（10→70%，并发）→ 合并去重（→85%）→
//! 结构化方法论（→100%）。方法论 JSON 解析失败重试一次，仍失败则整体 Failed。

use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};

use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

use super::models::*;
use crate::{
    book_deconstruction::{
        analyzer::parse_json_response,
        chunker::extract_sample,
        models::{AnalysisError, TextChunk},
    },
    db::DbPool,
    llm::LlmService,
    router::{Complexity, Priority, RoutingRequest, TaskType},
};

pub struct GuidebookDistiller {
    llm_service: LlmService,
    app_handle: AppHandle,
    pool: DbPool,
    semaphore: Arc<Semaphore>,
    active_requests: Arc<AtomicI32>,
}

impl GuidebookDistiller {
    pub fn new(
        llm_service: LlmService,
        app_handle: AppHandle,
        pool: DbPool,
        concurrency: usize,
    ) -> Self {
        Self {
            llm_service,
            app_handle,
            pool,
            semaphore: Arc::new(Semaphore::new(concurrency.max(1).min(100))),
            active_requests: Arc::new(AtomicI32::new(0)),
        }
    }

    pub async fn distill(
        &self,
        guidebook_id: &str,
        chunks: &[TextChunk],
        fold_in: Option<&CustomMethodology>,
        heartbeat_callback: Option<Box<dyn Fn() + Send + Sync>>,
        cancel_check: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    ) -> Result<DistillationOutput, AnalysisError> {
        let check_cancel = || -> Result<(), AnalysisError> {
            if let Some(ref cb) = cancel_check {
                if cb() {
                    return Err(AnalysisError::Cancelled("用户取消提炼".to_string()));
                }
            }
            Ok(())
        };
        let heartbeat = || {
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
        };

        // Step 1: 元信息（→10%）
        self.emit_progress(guidebook_id, "extracting", 5, "正在识别指导书元信息...")
            .await;
        heartbeat();
        check_cancel()?;
        let sample = extract_sample(
            &chunks
                .first()
                .map(|c| c.content.clone())
                .unwrap_or_default(),
            3000,
        );
        let metadata = self.extract_metadata(&sample).await?;
        let book_title = metadata
            .title
            .clone()
            .unwrap_or_else(|| "未命名".to_string());
        self.emit_progress(
            guidebook_id,
            "distilling",
            10,
            &format!("识别完成：《{}》", book_title),
        )
        .await;
        heartbeat();
        check_cancel()?;

        // Step 2: 分块提炼（10→70%，并发）
        let total = chunks.len();
        self.emit_progress(
            guidebook_id,
            "distilling",
            12,
            &format!("正在分块提炼创作资产（共 {} 块）...", total),
        )
        .await;
        let assets = self
            .distill_chunks(guidebook_id, chunks, &cancel_check)
            .await?;
        heartbeat();
        check_cancel()?;

        // Step 3: 合并去重（→85%）；fold-in 时与现有方法论资产融合
        let merged = if let Some(existing_cm) = fold_in {
            self.emit_progress(guidebook_id, "merging", 72, "正在与现有方法论融合...")
                .await;
            self.foldin_assets(existing_cm, &assets).await?
        } else {
            self.emit_progress(guidebook_id, "merging", 72, "正在分类合并创作资产...")
                .await;
            self.merge_assets(&assets).await?
        };
        heartbeat();
        check_cancel()?;

        // Step 4: 结构化方法论（→100%），JSON 失败重试一次
        self.emit_progress(guidebook_id, "merging", 88, "正在生成创作方法论...")
            .await;
        let methodology = match self
            .generate_methodology(&merged.principles, &book_title)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "[GuidebookDistiller] methodology 首次生成失败，重试一次: {}",
                    e
                );
                self.generate_methodology(&merged.principles, &book_title)
                    .await?
            }
        };
        self.emit_progress(guidebook_id, "merging", 100, "提炼完成")
            .await;
        heartbeat();

        Ok(DistillationOutput {
            metadata,
            methodology,
            techniques: merged.techniques,
            cheatsheet: Cheatsheet {
                decision_rules: merged.decision_rules,
                anti_patterns: merged.anti_patterns,
            },
        })
    }

    // ==================== 各步骤实现 ====================

    fn render_prompt(&self, id: &str, vars: &[(&str, String)]) -> Option<String> {
        let tpl = crate::prompts::registry::resolve_prompt(&self.pool, id)
            .ok()
            .or_else(|| crate::prompts::registry::resolve_prompt_default(id))?;
        let mut map = std::collections::HashMap::new();
        for (k, v) in vars {
            map.insert(k.to_string(), v.clone());
        }
        Some(crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &map))
    }

    async fn extract_metadata(
        &self,
        sample_text: &str,
    ) -> Result<LlmGuidebookMetadataResponse, AnalysisError> {
        let prompt = self
            .render_prompt("distill_metadata", &[("text", sample_text.to_string())])
            .ok_or_else(|| AnalysisError::LlmError("prompt distill_metadata 未注册".into()))?;
        let resp = call_llm(
            &self.llm_service,
            "guidebook_metadata",
            prompt,
            Some(500),
            Some(0.3),
        )
        .await?;
        parse_json_response(&resp)
    }

    async fn distill_chunks(
        &self,
        guidebook_id: &str,
        chunks: &[TextChunk],
        cancel_check: &Option<Box<dyn Fn() -> bool + Send + Sync>>,
    ) -> Result<ChunkAssets, AnalysisError> {
        let total = chunks.len();
        let processed = Arc::new(AtomicI32::new(0));
        let mut set = tokio::task::JoinSet::new();

        for chunk in chunks {
            if let Some(cb) = cancel_check {
                if cb() {
                    return Err(AnalysisError::Cancelled("用户取消提炼".to_string()));
                }
            }
            let sem = self.semaphore.clone();
            let llm = self.llm_service.clone();
            let pool = self.pool.clone();
            let active = self.active_requests.clone();
            let processed = processed.clone();
            let app = self.app_handle.clone();
            let gid = guidebook_id.to_string();
            let content = chunk.content.clone();
            let total = total;

            set.spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| AnalysisError::LlmError(format!("Semaphore error: {}", e)))?;
                active.fetch_add(1, Ordering::Relaxed);
                let result = async {
                    let tpl = crate::prompts::registry::resolve_prompt(&pool, "distill_chunk")
                        .ok()
                        .or_else(|| {
                            crate::prompts::registry::resolve_prompt_default("distill_chunk")
                        })
                        .ok_or_else(|| {
                            AnalysisError::LlmError("prompt distill_chunk 未注册".into())
                        })?;
                    let mut vars = std::collections::HashMap::new();
                    vars.insert("text".to_string(), content);
                    let prompt =
                        crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars);
                    let resp =
                        call_llm(&llm, "guidebook_chunk", prompt, Some(3000), Some(0.3)).await?;
                    let parsed: LlmDistillChunkResponse = parse_json_response(&resp)?;
                    Ok::<LlmDistillChunkResponse, AnalysisError>(parsed)
                }
                .await;
                active.fetch_sub(1, Ordering::Relaxed);
                let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
                // 10→70 按分块进度线性推进
                let progress = 10 + (60 * done / total.max(1) as i32);
                let _ = app.emit(
                    "guidebook-distillation-progress",
                    DistillationProgressEvent {
                        guidebook_id: gid,
                        status: "distilling".to_string(),
                        progress,
                        current_step: format!("分块提炼中 {}/{}", done, total),
                        message: None,
                        active_threads: active.load(Ordering::Relaxed),
                    },
                );
                result
            });
        }

        let mut all = ChunkAssets::default();
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(chunk_assets)) => all.extend(chunk_assets),
                Ok(Err(e)) => {
                    // 单块失败不致命：记录并继续
                    log::warn!("[GuidebookDistiller] 单块提炼失败，跳过: {}", e);
                }
                Err(e) => {
                    return Err(AnalysisError::LlmError(format!("Join error: {}", e)));
                }
            }
        }
        Ok(all)
    }

    async fn merge_assets(
        &self,
        assets: &ChunkAssets,
    ) -> Result<LlmDistillMergeResponse, AnalysisError> {
        if assets.is_empty() {
            return Err(AnalysisError::LlmError(
                "全书未提炼出任何创作要点".to_string(),
            ));
        }
        let joined = build_merge_input(assets);
        let prompt = self
            .render_prompt("distill_merge", &[("points", joined)])
            .ok_or_else(|| AnalysisError::LlmError("prompt distill_merge 未注册".into()))?;
        // max_tokens 8000：推理模型（deepseek 等）会把数千 token 烧在
        // reasoning_content/CoT 上，4000 预算曾导致 content 为空直接解析失败
        // （v0.36.0 merge 卡死案）。失败重试一次与 generate_methodology
        // 同模式。
        let attempt = || async {
            let resp = call_llm(
                &self.llm_service,
                "guidebook_merge",
                prompt.clone(),
                Some(8000),
                Some(0.3),
            )
            .await?;
            let parsed: LlmDistillMergeResponse = parse_json_response(&resp)?;
            if parsed.principles.is_empty() {
                return Err(AnalysisError::LlmError("合并后原则为空".to_string()));
            }
            Ok(parsed)
        };
        match attempt().await {
            Ok(m) => Ok(m),
            Err(e) => {
                log::warn!("[GuidebookDistiller] merge 首次失败，重试一次: {}", e);
                attempt().await
            }
        }
    }

    /// fold-in：新资产与现有方法论资产融合（输出与 merge 同构）
    async fn foldin_assets(
        &self,
        existing: &CustomMethodology,
        new_assets: &ChunkAssets,
    ) -> Result<LlmDistillMergeResponse, AnalysisError> {
        if new_assets.is_empty() {
            return Err(AnalysisError::LlmError(
                "全书未提炼出任何创作要点".to_string(),
            ));
        }
        let prompt = self
            .render_prompt(
                "distill_foldin",
                &[
                    (
                        "existing",
                        clip_chars(&build_existing_assets_input(existing), 3000),
                    ),
                    ("new", clip_chars(&build_merge_input(new_assets), 3000)),
                ],
            )
            .ok_or_else(|| AnalysisError::LlmError("prompt distill_foldin 未注册".into()))?;
        let attempt = || async {
            let resp = call_llm(
                &self.llm_service,
                "guidebook_foldin",
                prompt.clone(),
                Some(8000),
                Some(0.3),
            )
            .await?;
            let parsed: LlmDistillMergeResponse = parse_json_response(&resp)?;
            if parsed.principles.is_empty() {
                return Err(AnalysisError::LlmError("融合后原则为空".to_string()));
            }
            Ok(parsed)
        };
        match attempt().await {
            Ok(m) => Ok(m),
            Err(e) => {
                log::warn!("[GuidebookDistiller] foldin 首次失败，重试一次: {}", e);
                attempt().await
            }
        }
    }

    async fn generate_methodology(
        &self,
        principles: &[String],
        book_title: &str,
    ) -> Result<LlmMethodologyResponse, AnalysisError> {
        let prompt = self
            .render_prompt(
                "distill_methodology",
                &[
                    ("principles", principles.join("\n")),
                    ("book_title", book_title.to_string()),
                ],
            )
            .ok_or_else(|| AnalysisError::LlmError("prompt distill_methodology 未注册".into()))?;
        let resp = call_llm(
            &self.llm_service,
            "guidebook_methodology",
            prompt,
            Some(4000),
            Some(0.5),
        )
        .await?;
        let parsed: LlmMethodologyResponse = parse_json_response(&resp)?;
        validate_methodology(parsed)
    }

    async fn emit_progress(&self, guidebook_id: &str, status: &str, progress: i32, message: &str) {
        let _ = self.app_handle.emit(
            "guidebook-distillation-progress",
            DistillationProgressEvent {
                guidebook_id: guidebook_id.to_string(),
                status: status.to_string(),
                progress,
                current_step: message.to_string(),
                message: Some(message.to_string()),
                active_threads: self.active_requests.load(Ordering::Relaxed),
            },
        );
    }
}

/// 分块结构化资产的聚合容器
#[derive(Debug, Default)]
pub struct ChunkAssets {
    pub points: Vec<String>,
    pub techniques: Vec<Technique>,
    pub decision_rules: Vec<String>,
    pub anti_patterns: Vec<AntiPattern>,
}

impl ChunkAssets {
    pub fn extend(&mut self, r: LlmDistillChunkResponse) {
        self.points.extend(r.points);
        self.techniques.extend(r.techniques);
        self.decision_rules.extend(r.decision_rules);
        self.anti_patterns.extend(r.anti_patterns);
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
            && self.techniques.is_empty()
            && self.decision_rules.is_empty()
            && self.anti_patterns.is_empty()
    }
}

/// 截断到 max 字（chars）
fn clip_chars(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() > max {
        t.chars().take(max).collect()
    } else {
        t.to_string()
    }
}

/// 构建 merge 输入：四类资产分区，单条 200 字、总量 6000 字截断。
/// 总量上限曾取 12000，实测 prompt 达 8248 tokens 超出 8192 ctx 模型
/// （Gemma 等）直接 400（v0.36.0 merge 卡死案诱因之一），降至 6000。
fn build_merge_input(assets: &ChunkAssets) -> String {
    let mut sections = Vec::new();
    if !assets.points.is_empty() {
        let lines = assets
            .points
            .iter()
            .map(|p| clip_chars(p, 200))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("【要点】\n{}", lines));
    }
    if !assets.techniques.is_empty() {
        let lines = assets
            .techniques
            .iter()
            .map(|t| {
                clip_chars(
                    &format!("{}｜何时用：{}｜怎么做：{}", t.name, t.when_to_use, t.how),
                    200,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("【技巧】\n{}", lines));
    }
    if !assets.decision_rules.is_empty() {
        let lines = assets
            .decision_rules
            .iter()
            .map(|r| clip_chars(r, 200))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("【决策规则】\n{}", lines));
    }
    if !assets.anti_patterns.is_empty() {
        let lines = assets
            .anti_patterns
            .iter()
            .map(|a| clip_chars(&format!("{}｜{}", a.what, a.why), 200))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("【反模式】\n{}", lines));
    }
    clip_chars(&sections.join("\n\n"), 6000)
}

/// 现有 CM 资产渲染为 foldin 输入（复用四类分区格式，复用 steps 的 principles
/// 位）
fn build_existing_assets_input(cm: &CustomMethodology) -> String {
    let assets = ChunkAssets {
        points: cm
            .steps
            .iter()
            .map(|s| format!("{}：{}", s.title, s.instruction))
            .collect(),
        techniques: cm.patterns.clone(),
        decision_rules: cm.cheatsheet.decision_rules.clone(),
        anti_patterns: cm.cheatsheet.anti_patterns.clone(),
    };
    build_merge_input(&assets)
}

/// foldin 完整输入（带两大段标题，总量 12000 字截断）
fn build_foldin_input(existing: &CustomMethodology, new_assets: &ChunkAssets) -> String {
    let combined = format!(
        "【现有方法论资产】\n{}\n\n【新提炼资产】\n{}",
        build_existing_assets_input(existing),
        build_merge_input(new_assets)
    );
    clip_chars(&combined, 12000)
}

/// 校验提炼产物：名称非空、至少一个步骤、每个步骤有 instruction
fn validate_methodology(
    m: LlmMethodologyResponse,
) -> Result<LlmMethodologyResponse, AnalysisError> {
    if m.name.trim().is_empty() {
        return Err(AnalysisError::LlmError("方法论名称为空".to_string()));
    }
    if m.steps.is_empty() {
        return Err(AnalysisError::LlmError("方法论步骤为空".to_string()));
    }
    if m.steps.iter().any(|s| s.instruction.trim().is_empty()) {
        return Err(AnalysisError::LlmError(
            "存在空 instruction 的步骤".to_string(),
        ));
    }
    Ok(m)
}

/// LLM 调用（与 book_deconstruction/analyzer.rs call_llm 相同的路由方式，
/// RoutingRequest/Complexity/Priority/TaskType 来自 crate::router）
async fn call_llm(
    llm_service: &LlmService,
    context_label: &str,
    prompt: String,
    max_tokens: Option<i32>,
    temperature: Option<f32>,
) -> Result<String, AnalysisError> {
    let request = RoutingRequest {
        task: TaskType::Analysis,
        complexity: Complexity::Medium,
        budget_priority: Priority::Low,
        speed_priority: Priority::Low,
        estimated_input_tokens: 0,
        constraints: vec![],
    };
    llm_service
        .generate_for_request(
            request,
            prompt,
            max_tokens,
            temperature,
            Some(context_label),
        )
        .await
        .map(|r| r.content)
        .map_err(|e| AnalysisError::LlmError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_assets_aggregate_extends_all_categories() {
        let mut a = ChunkAssets::default();
        a.extend(LlmDistillChunkResponse {
            points: vec!["p1".into()],
            techniques: vec![Technique {
                name: "t1".into(),
                when_to_use: String::new(),
                how: String::new(),
            }],
            decision_rules: vec!["r1".into()],
            anti_patterns: vec![],
        });
        a.extend(LlmDistillChunkResponse {
            points: vec!["p2".into()],
            techniques: vec![],
            decision_rules: vec![],
            anti_patterns: vec![AntiPattern {
                what: "w".into(),
                why: String::new(),
            }],
        });
        assert_eq!(a.points, vec!["p1", "p2"]);
        assert_eq!(a.techniques.len(), 1);
        assert_eq!(a.decision_rules.len(), 1);
        assert_eq!(a.anti_patterns.len(), 1);
        assert!(!a.is_empty());
        assert!(ChunkAssets::default().is_empty());
    }

    #[test]
    fn merge_input_contains_four_sections_and_truncates() {
        let mut a = ChunkAssets::default();
        a.points.push("要点".repeat(300)); // 超长条 → 截断
        a.techniques.push(Technique {
            name: "雪花写作法".into(),
            when_to_use: "搭大纲".into(),
            how: "逐步扩展".into(),
        });
        a.decision_rules.push("当X时做Y，因为Z".into());
        a.anti_patterns.push(AntiPattern {
            what: "流水账".into(),
            why: "无冲突".into(),
        });
        let input = build_merge_input(&a);
        assert!(input.contains("【要点】"));
        assert!(input.contains("【技巧】"));
        assert!(input.contains("雪花写作法"));
        assert!(input.contains("【决策规则】"));
        assert!(input.contains("【反模式】"));
        assert!(input.contains("流水账"));
        // 单条 200 字截断：拼接行不含完整 300 字重复
        assert!(!input.contains(&"要点".repeat(300)));
    }

    #[test]
    fn merge_input_total_capped_at_6000_chars() {
        // v0.36.0 卡死案回归：merge prompt 总量 12000 字符（实测 8248 tokens）
        // 超出 8192 ctx 模型直接 400，上限降至 6000 字符
        let mut a = ChunkAssets::default();
        for i in 0..200 {
            a.points.push(format!("要点{}：{}", i, "长".repeat(150)));
            a.decision_rules
                .push(format!("规则{}：{}", i, "长".repeat(150)));
        }
        let input = build_merge_input(&a);
        assert!(input.chars().count() <= 6000);
    }

    #[test]
    fn validate_methodology_accepts_valid() {
        let m = LlmMethodologyResponse {
            name: "三幕冲突法".into(),
            description: Some("d".into()),
            steps: vec![LlmMethodologyStepResponse {
                title: "建冲突".into(),
                instruction: "先确立核心冲突".into(),
                checklist: vec!["冲突是否明确？".into()],
            }],
        };
        assert!(validate_methodology(m).is_ok());
    }

    fn sample_cm() -> CustomMethodology {
        CustomMethodology {
            id: "custom_t".into(),
            guidebook_id: None,
            name: "旧方法论".into(),
            description: None,
            steps: vec![MethodologyStep {
                title: "旧步骤".into(),
                instruction: "旧指令".into(),
                checklist: vec![],
            }],
            patterns: vec![Technique {
                name: "旧技巧".into(),
                when_to_use: "w".into(),
                how: "h".into(),
            }],
            cheatsheet: Cheatsheet {
                decision_rules: vec!["旧规则".into()],
                anti_patterns: vec![AntiPattern {
                    what: "旧反模式".into(),
                    why: "why".into(),
                }],
            },
            enabled: true,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
        }
    }

    #[test]
    fn foldin_input_contains_existing_and_new_sections() {
        let cm = sample_cm();
        let mut new_assets = ChunkAssets::default();
        new_assets.points.push("新要点".into());
        new_assets.techniques.push(Technique {
            name: "新技巧".into(),
            when_to_use: String::new(),
            how: String::new(),
        });
        let input = build_foldin_input(&cm, &new_assets);
        assert!(input.contains("【现有方法论资产】"));
        assert!(input.contains("旧技巧"));
        assert!(input.contains("旧规则"));
        assert!(input.contains("旧反模式"));
        assert!(input.contains("【新提炼资产】"));
        assert!(input.contains("新要点"));
        assert!(input.contains("新技巧"));
    }

    #[test]
    fn foldin_input_truncates_to_budget() {
        let mut cm = sample_cm();
        cm.patterns = (0..200)
            .map(|i| Technique {
                name: format!("技巧{}", i),
                when_to_use: "w".repeat(100),
                how: "h".repeat(200),
            })
            .collect();
        let input = build_foldin_input(&cm, &ChunkAssets::default());
        assert!(input.chars().count() <= 12050); // 12000 + 段标题余量
    }

    #[test]
    fn validate_methodology_rejects_empty() {
        let no_name = LlmMethodologyResponse {
            name: "  ".into(),
            description: None,
            steps: vec![LlmMethodologyStepResponse {
                title: "t".into(),
                instruction: "i".into(),
                checklist: vec![],
            }],
        };
        assert!(validate_methodology(no_name).is_err());

        let no_steps = LlmMethodologyResponse {
            name: "n".into(),
            description: None,
            steps: vec![],
        };
        assert!(validate_methodology(no_steps).is_err());

        let empty_instruction = LlmMethodologyResponse {
            name: "n".into(),
            description: None,
            steps: vec![LlmMethodologyStepResponse {
                title: "t".into(),
                instruction: " ".into(),
                checklist: vec![],
            }],
        };
        assert!(validate_methodology(empty_instruction).is_err());
    }
}
