//! Model Gateway — 任务分类与分配
//!
//! v0.15.0: 升级为智能任务分类器，按 prompt 长度 + agent 类型将任务分入
//! LightTool / BalancedWork / HeavyCreation 三类，驱动后续模型路由决策。
//! v0.19.0: 集成 SING 意图图分类，支持基于意图的任务路由。

use super::types::{GatewayRequest, TaskClass};
use crate::router::{Complexity, RoutingRequest, TaskType};

/// 智能任务分类器（v0.15.0 替代旧版 TaskClassifier）
///
/// 决策依据：
/// 1. Agent 类型（Writer/Inspector 等）
/// 2. TaskType + 估计 input/output token 数
/// 3. 升级惩罚：输入 >4000 token 或输出 >1500 token → 升为 HeavyCreation
/// 4. SING 意图类型（v0.19.0）：基于意图图的动词-宾语分类
#[derive(Debug, Clone, Default)]
pub struct TaskClassifier;

impl TaskClassifier {
    pub fn new() -> Self {
        Self
    }

    /// 根据 GatewayRequest 判定任务复杂度类别（v0.15.0 核心）
    ///
    /// v0.20.1: 若请求携带 SING 意图（intent_verb + intent_object），
    /// 优先使用 `classify_by_intention` 进行意图感知分类，实现从用户意图
    /// 到模型复杂度的精确映射。否则回退到 TaskType + agent_id 分类。
    ///
    /// Phase 2/3: 同时参考意图图发现的 asset_tags 做二次校准：
    /// - 外部工具 / 系统命令类标签 → 轻量工具
    /// - 明确创作类标签 → 重型创作
    pub fn classify_task(req: &GatewayRequest) -> TaskClass {
        // v0.20.1: SING 意图感知优先
        let intent_verb = req.intent_verb.as_ref();
        let intent_object = req.intent_object.as_ref();
        let base = if let (Some(verb), Some(object)) = (intent_verb, intent_object) {
            Self::classify_by_intention(verb.as_str(), object.as_str())
        } else {
            let base = Self::classify_by_type(&req.task, &req.agent_id);
            Self::upgrade_by_size(base, req.estimated_input_tokens, req.max_tokens)
        };

        // Phase 2/3: 资产标签校准（仅在标签强烈暗示不同类别时覆盖）
        Self::adjust_by_asset_tags(base, &req.asset_tags)
    }

    fn adjust_by_asset_tags(base: TaskClass, tags: &[String]) -> TaskClass {
        let tag_set: std::collections::HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();
        if tag_set.contains("mcp_tool") || tag_set.contains("system_command") {
            return TaskClass::LightTool;
        }
        // v0.23.9: 把 TriShot 透传过来的创作资产标签识别为重型创作，
        // 让 Call 3 最终生成优先使用质量/创作能力强的模型。
        let creative_tags = [
            "genre_profile",
            "creative_writing",
            "methodology",
            "beat_card",
            "story_engine",
            "pressure_relationship",
            "style_dna",
            "skill",
        ];
        if creative_tags.iter().any(|t| tag_set.contains(t)) {
            return TaskClass::HeavyCreation;
        }
        base
    }

    /// v0.19.0: 基于 SING 意图图的任务分类
    ///
    /// 将意图图的动词-宾语分类映射到任务类别，实现意图感知的模型路由。
    pub fn classify_by_intention(verb: &str, object: &str) -> TaskClass {
        match verb.to_lowercase().as_str() {
            // 轻量分析类
            "analyze" | "detect" | "probe" | "check" | "verify" | "inspect" => TaskClass::LightTool,
            // 生成/创作类（重型）
            "generate" | "write" | "create" | "compose" | "draft" => TaskClass::HeavyCreation,
            // 增强/修改类（中型）
            "enhance" | "polish" | "revise" | "edit" | "improve" | "refine" => {
                TaskClass::BalancedWork
            }
            // 管理/查询类（轻量）
            "manage" | "update" | "query" | "search" | "find" => TaskClass::LightTool,
            // 规划/结构类（中型）
            "plan" | "outline" | "structure" | "organize" => TaskClass::BalancedWork,
            // 默认根据宾语判断
            _ => Self::classify_by_object(object),
        }
    }

    /// 根据宾语判断任务类别
    fn classify_by_object(object: &str) -> TaskClass {
        match object.to_lowercase().as_str() {
            "prose" | "chapter" | "scene" | "story" | "content" => TaskClass::HeavyCreation,
            "style" | "tone" | "voice" | "character" => TaskClass::BalancedWork,
            "world" | "setting" | "outline" | "structure" => TaskClass::BalancedWork,
            "quality" | "issue" | "problem" | "error" => TaskClass::LightTool,
            _ => TaskClass::BalancedWork,
        }
    }

    fn classify_by_type(task: &TaskType, agent_id: &str) -> TaskClass {
        match (task, agent_id) {
            // 轻量工具
            (TaskType::Analysis, id)
                if id == "input_hint"
                    || id == "intent_detection"
                    || id == "model_gateway_probe" =>
            {
                TaskClass::LightTool
            }
            (TaskType::Summarization, _) => TaskClass::LightTool,
            (TaskType::WorldBuilding, _) => TaskClass::LightTool,
            // 重型创作
            (TaskType::CreativeWriting, _) => TaskClass::HeavyCreation,
            (TaskType::Editing, _) => TaskClass::HeavyCreation,
            (_, id) if id == "writer" || id == "inspector" || id == "style_mimic" => {
                TaskClass::HeavyCreation
            }
            _ => TaskClass::BalancedWork,
        }
    }

    fn upgrade_by_size(base: TaskClass, input_tokens: u32, max_output: Option<i32>) -> TaskClass {
        let out = max_output.unwrap_or(512) as u32;
        match base {
            TaskClass::LightTool if input_tokens > 4000 || out > 1500 => TaskClass::BalancedWork,
            TaskClass::BalancedWork if input_tokens > 4000 || out > 1500 => {
                TaskClass::HeavyCreation
            }
            other => other,
        }
    }

    /// 兼容旧版：根据请求动态评估复杂度（转为 RoutingRequest）
    pub fn classify(&self, request: &GatewayRequest) -> RoutingRequest {
        let complexity = request
            .complexity
            .unwrap_or_else(|| match Self::classify_task(request) {
                TaskClass::LightTool => Complexity::Low,
                TaskClass::BalancedWork => Complexity::Medium,
                TaskClass::HeavyCreation => Complexity::High,
            });

        RoutingRequest {
            task: request.task.clone(),
            complexity,
            budget_priority: request.budget_priority.clone(),
            speed_priority: request.speed_priority.clone(),
            estimated_input_tokens: request.estimated_input_tokens,
            constraints: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::Priority;

    #[test]
    fn test_classify_by_intention_verbs() {
        // 轻量分析类
        assert_eq!(
            TaskClassifier::classify_by_intention("analyze", "quality"),
            TaskClass::LightTool
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("detect", "issue"),
            TaskClass::LightTool
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("inspect", "prose"),
            TaskClass::LightTool
        );

        // 重型创作类
        assert_eq!(
            TaskClassifier::classify_by_intention("generate", "prose"),
            TaskClass::HeavyCreation
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("write", "chapter"),
            TaskClass::HeavyCreation
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("create", "story"),
            TaskClass::HeavyCreation
        );

        // 中型增强类
        assert_eq!(
            TaskClassifier::classify_by_intention("enhance", "style"),
            TaskClass::BalancedWork
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("polish", "prose"),
            TaskClass::BalancedWork
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("revise", "content"),
            TaskClass::BalancedWork
        );

        // 管理/查询类
        assert_eq!(
            TaskClassifier::classify_by_intention("manage", "character"),
            TaskClass::LightTool
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("query", "world"),
            TaskClass::LightTool
        );

        // 规划类
        assert_eq!(
            TaskClassifier::classify_by_intention("plan", "outline"),
            TaskClass::BalancedWork
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("structure", "scene"),
            TaskClass::BalancedWork
        );
    }

    #[test]
    fn test_classify_by_intention_fallback_to_object() {
        // 未知动词，根据宾语判断
        assert_eq!(
            TaskClassifier::classify_by_intention("unknown", "prose"),
            TaskClass::HeavyCreation
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("unknown", "style"),
            TaskClass::BalancedWork
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("unknown", "quality"),
            TaskClass::LightTool
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("unknown", "unknown"),
            TaskClass::BalancedWork
        );
    }

    #[test]
    fn test_classify_by_intention_case_insensitive() {
        assert_eq!(
            TaskClassifier::classify_by_intention("GENERATE", "PROSE"),
            TaskClass::HeavyCreation
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("Analyze", "Quality"),
            TaskClass::LightTool
        );
        assert_eq!(
            TaskClassifier::classify_by_intention("Enhance", "Style"),
            TaskClass::BalancedWork
        );
    }

    #[test]
    fn test_adjust_by_asset_tags() {
        assert_eq!(
            TaskClassifier::adjust_by_asset_tags(
                TaskClass::BalancedWork,
                &["mcp_tool".to_string()]
            ),
            TaskClass::LightTool
        );
        assert_eq!(
            TaskClassifier::adjust_by_asset_tags(
                TaskClass::LightTool,
                &["genre_profile".to_string()]
            ),
            TaskClass::HeavyCreation
        );
        assert_eq!(
            TaskClassifier::adjust_by_asset_tags(TaskClass::HeavyCreation, &["other".to_string()]),
            TaskClass::HeavyCreation
        );
    }

    #[test]
    fn test_classify_task_considers_asset_tags() {
        let req = GatewayRequest {
            prompt: "test".to_string(),
            agent_id: "writer".to_string(),
            task: TaskType::CreativeWriting,
            complexity: None,
            budget_priority: Priority::Low,
            speed_priority: Priority::Low,
            estimated_input_tokens: 0,
            max_tokens: None,
            temperature: None,
            stream: false,
            request_id: "r1".to_string(),
            context_label: None,
            timeout_seconds_override: None,
            max_retries_override: None,
            intent_verb: Some("generate".to_string()),
            intent_object: Some("prose".to_string()),
            asset_tags: vec!["mcp_tool".to_string()],
            discovered_asset_ids: vec![],
            response_format: None,
            system_prompt: None,
        };
        // 即使意图是 generate prose，mcp_tool 标签也应将其降级为 LightTool
        assert_eq!(TaskClassifier::classify_task(&req), TaskClass::LightTool);
    }
}
