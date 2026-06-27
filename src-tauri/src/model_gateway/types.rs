//! Model Gateway — 通用类型定义
//!
//! v0.14.0: 为模型网关提供任务、健康快照、路由决策等共享类型。

use serde::{Deserialize, Serialize};

use crate::{
    llm::adapter::ResponseFormat,
    router::{Complexity, Priority, RoutingDecision, TaskType},
};

/// 模型健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// 尚未探测
    Unknown,
    /// 探测成功且近期成功率达标
    Healthy,
    /// 可用但指标下降（如 TTFB 过高、成功率偏低）
    Degraded,
    /// 探测失败或近期成功率过低
    Unhealthy,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// 单个模型的健康快照
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelHealthSnapshot {
    pub model_id: String,
    pub model_name: String,
    pub status: HealthStatus,
    /// 首 token 时间（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<u64>,
    /// 生成速度（tokens / second）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tps: Option<f64>,
    /// 最近 24 小时成功率（0.0–1.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_rate_24h: Option<f64>,
    /// 平均延迟（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_latency_ms: Option<u64>,
    /// 最近一次错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// 最近一次探测时间（ISO 8601）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked_at: Option<String>,
    /// 是否被用户启用
    pub enabled: bool,
    /// 是否被当前任务选为主模型
    #[serde(default)]
    pub is_primary: bool,
    /// 若为 fallback 目标，显示将 fallback 到该模型
    #[serde(default)]
    pub is_fallback: bool,
}

/// 轻量探测结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeResult {
    pub success: bool,
    /// 首 token 时间（毫秒）
    pub ttft_ms: u64,
    /// 总输出 token 数
    pub total_tokens: i32,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// tokens per second（不包含 TTFT）
    pub tps: f64,
    pub error: Option<String>,
}

/// 网关生成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    /// 原始 prompt
    pub prompt: String,
    /// 发起调用的 Agent 或模块标识
    pub agent_id: String,
    /// 任务类型
    pub task: TaskType,
    /// 可选的复杂度覆盖；None 时由 dispatcher 动态评估
    pub complexity: Option<Complexity>,
    /// 成本优先级
    pub budget_priority: Priority,
    /// 速度优先级
    pub speed_priority: Priority,
    /// 估计输入 token 数
    pub estimated_input_tokens: u32,
    /// 期望最大输出 token 数
    pub max_tokens: Option<i32>,
    /// 温度
    pub temperature: Option<f32>,
    /// 是否流式输出
    pub stream: bool,
    /// 请求 ID（用于日志和取消）
    pub request_id: String,
    /// 上下文标签（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_label: Option<String>,
    /// 超时覆盖（秒，可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds_override: Option<u64>,
    /// 最大重试覆盖（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries_override: Option<u32>,
    /// v0.20.1: SING 意图动词（如 "generate"、"enhance"、"inspect"）
    ///
    /// 由 IntentionGraphPlanner 合成意图后注入，使模型网关能按意图分类路由。
    /// None 时回退到 TaskType + agent_id 分类。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub intent_verb: Option<String>,
    /// v0.20.1: SING 意图宾语（如 "prose"、"style"、"character"）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub intent_object: Option<String>,
    /// Phase 2/3: 意图图发现的资产标签（如 ["genre_profile",
    /// "post_apocalyptic"]）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub asset_tags: Vec<String>,
    /// Phase 2/3: 意图图发现的具体资产 ID 列表
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub discovered_asset_ids: Vec<String>,
    /// v0.23: 结构化输出格式（OpenAI/Ollama JSON mode）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub response_format: Option<ResponseFormat>,
    /// v0.23.65: 请求级 system_prompt（来自 PromptRegistry writer_system
    /// 渲染）。
    ///
    /// 优先级：`LlmProfile.system_prompt_override` > 此字段 > 适配器默认。
    /// TimeSliced/TriShot 路径不经 `build_writer_prompt`，用此字段把
    /// `writer_system` 写作准则传递到适配器层。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub system_prompt: Option<String>,
}

impl GatewayRequest {
    /// v0.23 TriShot：构造「最快路由」请求——用于 Call 1 路由合成器。
    ///
    /// 强制 `speed_priority: High` + `budget_priority: Low`，使网关在
    /// `select_candidates` 中倾向 `TaskClass::LightTool` 的 60% 速度权重，
    /// 配合 `GatewayExecutor::select_fastest_profile` 按算力档案 TTFB
    /// 选最快模型。
    pub fn for_fast_routing(prompt: String, agent_id: &str) -> Self {
        Self {
            prompt,
            agent_id: agent_id.to_string(),
            task: TaskType::Analysis,
            complexity: Some(Complexity::Low),
            budget_priority: Priority::Low,
            speed_priority: Priority::High,
            estimated_input_tokens: 0,
            max_tokens: None,
            temperature: None,
            stream: false,
            request_id: uuid::Uuid::new_v4().to_string(),
            context_label: None,
            timeout_seconds_override: None,
            max_retries_override: None,
            intent_verb: None,
            intent_object: None,
            asset_tags: Vec::new(),
            discovered_asset_ids: Vec::new(),
            response_format: None,
            system_prompt: None,
        }
    }
}

/// 网关层对路由决策的扩展：RoutingDecision 已包含候选链
pub type GatewayRoutingDecision = RoutingDecision;

/// 网关整体状态（供前端展示）
#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatus {
    /// 最近一次全量探测时间
    pub last_probe_at: Option<String>,
    /// 当前被选中的主模型
    pub primary_model_id: Option<String>,
    /// 所有启用模型的健康快照
    pub models: Vec<ModelHealthSnapshot>,
    /// 是否正在探测中
    pub is_probing: bool,
}

// ============================================================================
// v0.15.0: 智能调度器新增类型
// ============================================================================

/// 任务复杂度分类（v0.15.0 核心决策依据）
///
/// 网关依据此分类决定"快模型 vs 推理大模型"：
/// - `LightTool`：短 prompt + 短输出（意图识别、输入提示、JSON 提取）→
///   优先快模型
/// - `BalancedWork`：中等任务（设定修改、伏笔提取、章节摘要）→ 平衡
/// - `HeavyCreation`：长 prompt + 长输出（Writer 续写、Inspector
///   质检、Rewrite）→ 优先推理大模型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    /// 轻量工具任务：短输入 + 短输出，延迟敏感，优先选最快模型
    LightTool,
    /// 平衡工作：中等长度，质量与速度兼顾
    BalancedWork,
    /// 重型创作：长输入 + 长输出，质量优先，优先推理大模型
    HeavyCreation,
}

impl Default for TaskClass {
    fn default() -> Self {
        Self::BalancedWork
    }
}

impl TaskClass {
    /// 短任务基准字段选择器：LightTool 看 short_ttfb
    pub fn prefers_speed(&self) -> bool {
        matches!(self, TaskClass::LightTool)
    }

    /// 重型任务：看 sustained_tps 和质量
    pub fn prefers_quality(&self) -> bool {
        matches!(self, TaskClass::HeavyCreation)
    }
}

/// 流式基准测试结果（v0.15.0）
///
/// 由 benchmark.rs 通过 LlmService::generate_stream 采集，
/// 记录真实的 first-chunk TTFB 和持续 token/s。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkResult {
    pub success: bool,
    /// 真实首字节延迟（毫秒）——流式第一个 chunk 的时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_ttfb_ms: Option<u64>,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 输出 token 数
    pub output_tokens: u32,
    /// 输入 token 数（估算）
    pub input_tokens: u32,
    /// 持续生成速度 tokens/s = output_tokens / ((duration - ttfb) / 1000)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sustained_tps: Option<f64>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 持久化算力档案（v0.15.0）
///
/// 存储于 SQLite `model_capability_profile` 表，跨应用启动保留。
/// 网关三维打分的核心数据源。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityProfile {
    pub model_id: String,
    /// 短任务 TTFB p50（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_ttfb_ms_p50: Option<u64>,
    /// 短任务 TTFB p95（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_ttfb_ms_p95: Option<u64>,
    /// 长任务 TTFB p50（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_ttfb_ms_p50: Option<u64>,
    /// 长任务 TTFB p95（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_ttfb_ms_p95: Option<u64>,
    /// 长输出持续 token/s（创作场景核心指标）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sustained_tps: Option<f64>,
    /// 短输出 token/s
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_output_tps: Option<f64>,
    /// 最近 24 小时成功率
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_rate_24h: Option<f64>,
    /// 上次完整基准时间戳（unix 秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_full_benchmark_at: Option<i64>,
    /// 上次健康探测时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_health_probe_at: Option<i64>,
    /// 基准样本数
    pub benchmark_sample_count: i64,
    /// 健康状态
    pub status: HealthStatus,
    /// 状态原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
    /// 综合能力得分（0-100）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_score: Option<f64>,
    /// 速度得分
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_score: Option<f64>,
    /// 质量得分
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<f64>,
}

impl CapabilityProfile {
    /// v0.23.14: 从实测数据合成三个分数（0-100），填入空置已久的字段。
    ///
    /// - speed_score: TTFB（首 token 延迟，越低越好）+
    ///   TPS（生成速度，越高越好）各 50%
    /// - quality_score: 成功率直接映射 0-100
    /// - capability_score: 速度 40% + 质量 60%
    pub fn compute_scores(&mut self) {
        // 速度分：TTFB（短任务 p50）
        let ttfb_score = self
            .short_ttfb_ms_p50
            .map(|ms| {
                // <500ms = 100, >5000ms = 0, 线性插值
                let clamped = ms.max(500).min(5000) as f64;
                100.0 - (clamped - 500.0) / 4500.0 * 100.0
            })
            .unwrap_or(50.0);

        // 速度分：TPS（长输出持续速度）
        let tps_score = self
            .sustained_tps
            .map(|tps| {
                // >50 tps = 100, <5 tps = 0, 线性插值
                let clamped = tps.max(5.0).min(50.0);
                (clamped - 5.0) / 45.0 * 100.0
            })
            .unwrap_or(50.0);

        let speed_score = (ttfb_score * 0.5 + tps_score * 0.5).round();
        self.speed_score = Some(speed_score);

        // 质量分：成功率
        let quality_score = self
            .success_rate_24h
            .map(|r| (r * 100.0).round())
            .unwrap_or(0.0);
        self.quality_score = Some(quality_score);

        // 综合分：速度 40% + 质量 60%
        let capability_score = (speed_score * 0.4 + quality_score * 0.6).round();
        self.capability_score = Some(capability_score);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_scores_fast_healthy_model() {
        let mut cap = CapabilityProfile {
            model_id: "test-fast".to_string(),
            short_ttfb_ms_p50: Some(300),
            sustained_tps: Some(60.0),
            success_rate_24h: Some(1.0),
            ..Default::default()
        };
        cap.compute_scores();
        // TTFB < 500 → 100, TPS > 50 → 100, speed = 100
        assert_eq!(cap.speed_score, Some(100.0));
        // success_rate 1.0 → 100
        assert_eq!(cap.quality_score, Some(100.0));
        // 100*0.4 + 100*0.6 = 100
        assert_eq!(cap.capability_score, Some(100.0));
    }

    #[test]
    fn test_compute_scores_slow_degraded_model() {
        let mut cap = CapabilityProfile {
            model_id: "test-slow".to_string(),
            short_ttfb_ms_p50: Some(4000),
            sustained_tps: Some(10.0),
            success_rate_24h: Some(0.7),
            ..Default::default()
        };
        cap.compute_scores();
        // TTFB 4000: 100 - (4000-500)/4500*100 = 100 - 77.78 = 22.22 → 22
        // TPS 10: (10-5)/45*100 = 11.11 → 11
        // speed = 22*0.5 + 11*0.5 = 16.5 → 17 (rounded)
        let speed = cap.speed_score.unwrap();
        assert!(speed >= 15.0 && speed <= 19.0, "speed_score was {}", speed);
        // quality = 70
        assert_eq!(cap.quality_score, Some(70.0));
        // capability = speed*0.4 + 70*0.6
        let cap_score = cap.capability_score.unwrap();
        assert!(
            cap_score >= 48.0 && cap_score <= 52.0,
            "capability was {}",
            cap_score
        );
    }

    #[test]
    fn test_compute_scores_no_data_defaults_to_50() {
        let mut cap = CapabilityProfile {
            model_id: "test-empty".to_string(),
            ..Default::default()
        };
        cap.compute_scores();
        // No TTFB → 50, No TPS → 50, speed = 50
        assert_eq!(cap.speed_score, Some(50.0));
        // No success_rate → 0
        assert_eq!(cap.quality_score, Some(0.0));
        // 50*0.4 + 0*0.6 = 20
        assert_eq!(cap.capability_score, Some(20.0));
    }

    #[test]
    fn test_for_fast_routing_defaults() {
        let req = GatewayRequest::for_fast_routing("测试 prompt".to_string(), "tri-shot-router");
        assert_eq!(req.prompt, "测试 prompt");
        assert_eq!(req.agent_id, "tri-shot-router");
        assert_eq!(req.speed_priority, Priority::High);
        assert_eq!(req.budget_priority, Priority::Low);
        assert_eq!(req.complexity, Some(Complexity::Low));
        assert!(!req.request_id.is_empty(), "request_id 应自动生成");
        assert!(req.asset_tags.is_empty());
        assert!(req.intent_verb.is_none());
    }
}
