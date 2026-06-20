//! 动态 ReAct 执行循环
//!
//! Actions ∈ {Discover, Invoke, Respond}
//! 工具集在执行过程中动态累积，支持从输出中启发式发现新意图。

use crate::error::AppError;

use super::models::*;

/// ReAct 动作类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReActAction {
    /// 发现新意图/资产
    Discover {
        source_node_id: String,
        reason: String,
    },
    /// 调用资产执行
    Invoke {
        node_id: String,
        parameters: serde_json::Value,
    },
    /// 返回最终结果
    Respond {
        message: String,
        final_outputs: serde_json::Value,
    },
}

/// 动态 ReAct 执行器
pub struct DynamicReactor {
    max_nodes: usize,
    max_iterations: usize,
}

impl DynamicReactor {
    pub fn new(max_nodes: usize, max_iterations: usize) -> Self {
        Self {
            max_nodes,
            max_iterations,
        }
    }

    /// 执行单步 ReAct 循环
    /// 返回 (动作, 是否继续)
    pub fn step(
        &self,
        _graph: &mut ExecutionGraph,
        nodes: &mut Vec<ExecutionNode>,
        iteration: usize,
    ) -> Result<(ReActAction, bool), AppError> {
        if iteration >= self.max_iterations {
            return Ok((
                ReActAction::Respond {
                    message: "Max iterations reached".to_string(),
                    final_outputs: serde_json::json!({}),
                },
                false,
            ));
        }

        if nodes.len() >= self.max_nodes {
            return Ok((
                ReActAction::Respond {
                    message: "Max nodes reached".to_string(),
                    final_outputs: serde_json::json!({}),
                },
                false,
            ));
        }

        // 1. 找到待执行的节点（已完成依赖的 pending 节点）
        let ready_node = self.find_ready_node(nodes);

        if let Some(node) = ready_node {
            // 2. 调用该节点
            return Ok((
                ReActAction::Invoke {
                    node_id: node.id.clone(),
                    parameters: node.parameters.clone().unwrap_or(serde_json::json!({})),
                },
                true,
            ));
        }

        // 3. 如果没有待执行节点，检查是否需要发现新节点
        let completed_node_ids: Vec<String> = nodes
            .iter()
            .filter(|n| n.status == ExecutionNodeStatus::Completed)
            .map(|n| n.id.clone())
            .collect();

        let has_discoveries = if !completed_node_ids.is_empty() {
            // 从已完成节点的输出中启发式发现
            let discoveries = self.discover_from_outputs(nodes);
            let has = !discoveries.is_empty();
            for discovery in discoveries {
                nodes.push(discovery);
            }
            has
        } else {
            false
        };

        if has_discoveries && !completed_node_ids.is_empty() {
            return Ok((
                ReActAction::Discover {
                    source_node_id: completed_node_ids[0].clone(),
                    reason: "Discovered from output heuristic".to_string(),
                },
                true,
            ));
        }

        // 4. 所有节点完成，返回结果
        Ok((
            ReActAction::Respond {
                message: "Execution completed".to_string(),
                final_outputs: self.aggregate_outputs(nodes),
            },
            false,
        ))
    }

    /// 从已完成节点的输出中启发式发现新意图
    fn discover_from_outputs(&self, nodes: &[ExecutionNode]) -> Vec<ExecutionNode> {
        let mut discoveries = Vec::new();

        for node in nodes {
            if node.status != ExecutionNodeStatus::Completed {
                continue;
            }
            if let Some(ref outputs) = node.outputs {
                // 启发式：如果输出包含质量警告，添加 inspector 节点
                let output_str = outputs.to_string().to_lowercase();
                if output_str.contains("quality")
                    || output_str.contains("issue")
                    || output_str.contains("problem")
                    || output_str.contains("错误")
                    || output_str.contains("问题")
                {
                    discoveries.push(ExecutionNode {
                        id: format!("discovered_inspector_{}", uuid::Uuid::new_v4()),
                        graph_id: node.graph_id.clone(),
                        intention_id: Some("inspect_quality".to_string()),
                        asset_id: Some("agent_inspector".to_string()),
                        status: ExecutionNodeStatus::Discovered,
                        parameters: Some(serde_json::json!({"source": node.id})),
                        depends_on: Some(vec![node.id.clone()]),
                        outputs: None,
                        discovered_from: DiscoverySource::OutputHeuristic,
                        execution_time_ms: None,
                        created_at: chrono::Local::now(),
                        completed_at: None,
                    });
                }

                // 启发式：如果输出包含风格相关关键词，添加 style_enhancer 节点
                if output_str.contains("style")
                    || output_str.contains("风格")
                    || output_str.contains("文风")
                    || output_str.contains("tone")
                {
                    discoveries.push(ExecutionNode {
                        id: format!("discovered_style_{}", uuid::Uuid::new_v4()),
                        graph_id: node.graph_id.clone(),
                        intention_id: Some("enhance_style".to_string()),
                        asset_id: Some("skill_style_enhancer".to_string()),
                        status: ExecutionNodeStatus::Discovered,
                        parameters: Some(serde_json::json!({"source": node.id})),
                        depends_on: Some(vec![node.id.clone()]),
                        outputs: None,
                        discovered_from: DiscoverySource::OutputHeuristic,
                        execution_time_ms: None,
                        created_at: chrono::Local::now(),
                        completed_at: None,
                    });
                }
            }
        }

        discoveries
    }

    /// 找到可以执行的节点（依赖全部完成）
    fn find_ready_node(&self, nodes: &[ExecutionNode]) -> Option<ExecutionNode> {
        let completed_ids: std::collections::HashSet<String> = nodes
            .iter()
            .filter(|n| {
                n.status == ExecutionNodeStatus::Completed
                    || n.status == ExecutionNodeStatus::Skipped
            })
            .map(|n| n.id.clone())
            .collect();

        for node in nodes {
            if node.status != ExecutionNodeStatus::Pending
                && node.status != ExecutionNodeStatus::Discovered
            {
                continue;
            }

            let deps_satisfied = match &node.depends_on {
                None => true,
                Some(deps) => deps.iter().all(|d| completed_ids.contains(d)),
            };

            if deps_satisfied {
                return Some(node.clone());
            }
        }

        None
    }

    /// 聚合所有已完成节点的输出
    pub fn aggregate_outputs(&self, nodes: &[ExecutionNode]) -> serde_json::Value {
        let mut outputs = serde_json::Map::new();
        for node in nodes {
            if node.status == ExecutionNodeStatus::Completed {
                if let Some(ref out) = node.outputs {
                    outputs.insert(node.id.clone(), out.clone());
                }
            }
        }
        serde_json::Value::Object(outputs)
    }
}

impl Default for DynamicReactor {
    fn default() -> Self {
        Self::new(20, 50)
    }
}
