#![allow(dead_code)]
use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
};

use tauri::{AppHandle, Emitter, Manager};

use super::{NodeExecutionStatus, NodeType, WorkflowEngine, WorkflowInstance, WorkflowStatus};
use crate::task_system::{
    models::{CreateTaskRequest, TaskStatus, TaskType},
    service::TaskService,
};

/// Workflow scheduler - manages task execution with an in-memory queue
pub struct WorkflowScheduler {
    queue: Arc<Mutex<VecDeque<String>>>,
    /// P2-17 修复: 正在执行的实例集合，防止并发执行
    running_instances: Arc<Mutex<HashSet<String>>>,
}

impl WorkflowScheduler {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            running_instances: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Queue a workflow instance for execution
    /// 入队后会自动触发后台执行（若当前没有正在执行的任务）
    pub async fn schedule_execution(
        &self,
        instance_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 幂等检查: 已在队列或正在执行则跳过
        {
            let queue = self.queue.lock().unwrap();
            if queue.contains(&instance_id) {
                log::warn!(
                    "[WorkflowScheduler] Instance {} already in queue, skipping",
                    instance_id
                );
                return Ok(());
            }
        }
        {
            let running = self.running_instances.lock().unwrap();
            if running.contains(&instance_id) {
                log::warn!(
                    "[WorkflowScheduler] Instance {} already running, skipping",
                    instance_id
                );
                return Ok(());
            }
        }

        log::info!(
            "[WorkflowScheduler] Queuing workflow instance {} for execution",
            instance_id
        );
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(instance_id);
        Ok(())
    }

    /// 应在应用初始化时调用一次，启动一个 tokio::spawn 循环
    pub fn start_auto_drain(&self, engine: Arc<WorkflowEngine>, app_handle: AppHandle) {
        let queue = self.queue.clone();
        let running = self.running_instances.clone();
        tauri::async_runtime::spawn(async move {
            log::info!("[WorkflowScheduler] Auto-drain worker started");
            loop {
                // 每 2 秒检查一次队列
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                let instance_id = {
                    let mut q = queue.lock().unwrap();
                    q.pop_front()
                };

                if let Some(id) = instance_id {
                    // P2-17 修复: 检查实例是否正在执行
                    {
                        let mut running_set = running.lock().unwrap();
                        if running_set.contains(&id) {
                            log::warn!(
                                "[WorkflowScheduler] Instance {} is already running, skipping \
                                 auto-drain",
                                id
                            );
                            continue;
                        }
                        running_set.insert(id.clone());
                    }

                    log::info!("[WorkflowScheduler] Auto-draining instance {}", id);
                    let scheduler = WorkflowScheduler {
                        queue: queue.clone(),
                        running_instances: running.clone(),
                    };
                    match scheduler.run_instance(&engine, &app_handle, &id).await {
                        Ok(_) => {
                            log::info!("[WorkflowScheduler] Instance {} completed", id);
                        }
                        Err(e) => {
                            log::error!("[WorkflowScheduler] Instance {} failed: {}", id, e);
                            let _ = app_handle.emit(
                                "workflow-instance-failed",
                                serde_json::json!({
                                    "instance_id": id,
                                    "error": e.to_string(),
                                }),
                            );
                        }
                    }

                    // 释放执行标记
                    let mut running_set = running.lock().unwrap();
                    running_set.remove(&id);
                }
            }
        });
    }

    /// Get the number of queued instances
    pub fn queue_len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Process the next instance in the queue (serial execution)
    ///
    /// This is a simple executor that runs one node at a time.
    /// In production, this could be replaced with a worker pool.
    pub async fn execute_next(
        &self,
        engine: &WorkflowEngine,
        app_handle: &AppHandle,
    ) -> Option<Result<String, String>> {
        let instance_id = {
            let mut queue = self.queue.lock().unwrap();
            queue.pop_front()?
        };

        // P2-17 修复: 检查实例是否正在执行
        {
            let mut running = self.running_instances.lock().unwrap();
            if running.contains(&instance_id) {
                log::warn!(
                    "[WorkflowScheduler] Instance {} is already running, skipping execute_next",
                    instance_id
                );
                return Some(Err(format!("Instance {} is already running", instance_id)));
            }
            running.insert(instance_id.clone());
        }

        let result = match self.run_instance(engine, app_handle, &instance_id).await {
            Ok(_) => Some(Ok(instance_id.clone())),
            Err(e) => Some(Err(format!("Instance {} failed: {}", instance_id, e))),
        };

        // 释放执行标记
        let mut running = self.running_instances.lock().unwrap();
        running.remove(&instance_id);

        result
    }

    /// Run a single workflow instance to completion (serial node execution)
    async fn run_instance(
        &self,
        engine: &WorkflowEngine,
        app_handle: &AppHandle,
        instance_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!(
            "[WorkflowScheduler] Starting workflow instance {}",
            instance_id
        );

        // Get instance and workflow
        let (workflow, mut instance) = {
            let instance = engine
                .get_instance(instance_id)
                .ok_or("Instance not found")?;
            let workflow = engine
                .get_workflow(&instance.workflow_id)
                .ok_or("Workflow not found")?;
            (workflow, instance)
        };

        // Emit start event
        let _ = app_handle.emit(
            "workflow-started",
            serde_json::json!({
                "instance_id": instance_id,
                "workflow_id": workflow.id,
                "workflow_name": workflow.name,
            }),
        );

        // Mark instance as Running
        instance.status = WorkflowStatus::Running;
        engine.update_instance(&instance);

        // Execute nodes in topological order
        let mut iteration_count = 0;
        let max_iterations = workflow.nodes.len() * 2; // Safety limit

        loop {
            if iteration_count >= max_iterations {
                instance.status = WorkflowStatus::Failed;
                engine.update_instance(&instance);
                return Err("Workflow exceeded maximum iteration count".into());
            }
            iteration_count += 1;

            let next_nodes = self.get_next_nodes(&instance, &workflow.nodes, &workflow.edges);
            if next_nodes.is_empty() {
                break;
            }
            // Phase 1: Mark all nodes as Running (mutable borrow)
            let mut node_clones = Vec::new();
            for node_id in &next_nodes {
                let node = workflow
                    .nodes
                    .iter()
                    .find(|n| n.id == *node_id)
                    .ok_or("Node not found")?;

                self.update_node_status(
                    &mut instance,
                    node_id,
                    NodeExecutionStatus::Running,
                    None,
                    None,
                );
                engine.update_instance(&instance);

                let _ = app_handle.emit(
                    "workflow-node-started",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "node_id": node_id,
                        "node_name": node.name,
                        "node_type": node.node_type.to_string(),
                    }),
                );

                node_clones.push(node.clone());
            }

            // Phase 2: Execute nodes in parallel (each closure gets its own
            // clone)
            let mut node_futures = Vec::new();
            for node in &node_clones {
                let app_handle_clone = app_handle.clone();
                let scheduler_ref = self;
                let instance_clone = instance.clone();
                let timeout_secs = node.config.timeout_seconds.unwrap_or(300);
                node_futures.push(async move {
                    // P1-10 修复: 节点执行包装 timeout
                    let result = match tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        scheduler_ref.execute_node(node, &instance_clone, &app_handle_clone),
                    )
                    .await
                    {
                        Ok(res) => res,
                        Err(_) => Err(format!("节点执行超时 ({} 秒)", timeout_secs)),
                    };
                    (node.id.clone(), node.name.clone(), result)
                });
            }

            // Execute all nodes in parallel
            let node_results = futures::future::join_all(node_futures).await;

            // Process results serially to avoid concurrent mutable access to
            // instance
            for (node_id, _node_name, result) in node_results {
                match result {
                    Ok(output) => {
                        self.update_node_status(
                            &mut instance,
                            &node_id,
                            NodeExecutionStatus::Completed,
                            Some(output.clone()),
                            None,
                        );
                        instance.context.variables.insert(node_id.clone(), output);
                        engine.update_instance(&instance);

                        let _ = app_handle.emit(
                            "workflow-node-completed",
                            serde_json::json!({
                                "instance_id": instance_id,
                                "node_id": node_id,
                            }),
                        );
                    }
                    Err(e) => {
                        self.update_node_status(
                            &mut instance,
                            &node_id,
                            NodeExecutionStatus::Failed,
                            None,
                            Some(e.clone()),
                        );
                        // P1-11 修复: 失败时自动重试（最多 3 次）
                        // P2-18 修复: 重置失败节点状态为
                        // Pending，以便重试时重新执行
                        const MAX_RETRIES: u32 = 3;
                        let current_retries = instance.retry_count.unwrap_or(0);
                        if current_retries < MAX_RETRIES {
                            instance.retry_count = Some(current_retries + 1);
                            instance.status = WorkflowStatus::Pending;
                            // 重置失败节点为 Pending，保留已完成节点
                            if let Some(state) = instance.node_states.get_mut(&node_id) {
                                state.status = NodeExecutionStatus::Pending;
                                state.error = None;
                            }
                            engine.update_instance(&instance);
                            // 重新入队
                            {
                                let mut q = self.queue.lock().unwrap();
                                q.push_back(instance_id.to_string());
                            }
                            let _ = app_handle.emit(
                                "workflow-instance-retried",
                                serde_json::json!({
                                    "instance_id": instance_id,
                                    "node_id": node_id,
                                    "retry_count": current_retries + 1,
                                    "max_retries": MAX_RETRIES,
                                    "error": e,
                                }),
                            );
                            log::info!(
                                "[WorkflowScheduler] Instance {} queued for retry {}/{}",
                                instance_id,
                                current_retries + 1,
                                MAX_RETRIES
                            );
                            return Err(format!(
                                "Node {} failed, retry {}/{} queued",
                                node_id,
                                current_retries + 1,
                                MAX_RETRIES
                            )
                            .into());
                        } else {
                            instance.status = WorkflowStatus::Failed;
                            engine.update_instance(&instance);

                            let _ = app_handle.emit(
                                "workflow-node-failed",
                                serde_json::json!({
                                    "instance_id": instance_id,
                                    "node_id": node_id,
                                    "error": e,
                                }),
                            );
                            return Err(format!("Node {} failed: {}", node_id, e).into());
                        }
                    }
                }
            }

            if self.is_workflow_complete(&instance, &workflow.nodes) {
                break;
            }
        }

        instance.status = WorkflowStatus::Completed;
        instance.completed_at = Some(chrono::Utc::now());
        engine.update_instance(&instance);

        let _ = app_handle.emit(
            "workflow-completed",
            serde_json::json!({
                "instance_id": instance_id,
                "workflow_id": workflow.id,
            }),
        );

        log::info!(
            "[WorkflowScheduler] Workflow instance {} completed successfully",
            instance_id
        );
        Ok(())
    }

    /// Execute a single workflow node
    async fn execute_node(
        &self,
        node: &super::WorkflowNode,
        instance: &WorkflowInstance,
        app_handle: &AppHandle,
    ) -> Result<serde_json::Value, String> {
        match node.node_type {
            NodeType::Start => Ok(serde_json::json!({ "started": true })),
            NodeType::Condition => {
                let condition = node
                    .config
                    .parameters
                    .get("condition")
                    .and_then(|v| v.as_str())
                    .unwrap_or("true");
                // P1-10 修复: Condition 节点支持上下文变量和基本比较
                let result = evaluate_condition(condition, &instance.context.variables);
                Ok(serde_json::json!({ "condition_met": result }))
            }
            NodeType::Parallel => {
                // Simplified: mark as completed, parallel branches are handled
                // by DAG topology
                Ok(serde_json::json!({ "parallel": true }))
            }
            NodeType::End => Ok(serde_json::json!({ "completed": true })),
            _ => {
                self.execute_node_via_task_system(node, instance, app_handle)
                    .await
            }
        }
    }

    /// Build payload input/parameters for a workflow node
    fn build_node_payload(
        &self,
        node: &super::WorkflowNode,
        instance: &WorkflowInstance,
    ) -> (String, serde_json::Value) {
        match node.node_type {
            NodeType::WriteChapter => {
                let instruction = node
                    .config
                    .parameters
                    .get("instruction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Continue writing the story")
                    .to_string();
                let previous_content = get_last_content(&instance.context.variables);
                let input = if previous_content.is_empty() {
                    instruction
                } else {
                    format!("{instruction}\n\nPrevious content:\n{previous_content}")
                };
                (input, serde_json::json!({}))
            }
            NodeType::Revise => {
                let content = get_last_content(&instance.context.variables);
                let inspect_score = get_last_score(&instance.context.variables);
                let input = format!(
                    "Please revise the following content based on the inspection score \
                     {:.0}%:\n\n{}",
                    inspect_score * 100.0,
                    content
                );
                let parameters = serde_json::json!({
                    "inspect_score": inspect_score,
                });
                (input, parameters)
            }
            _ => {
                let content = get_last_content(&instance.context.variables);
                (content, serde_json::json!({}))
            }
        }
    }

    /// Delegate workflow node execution to task_system
    async fn execute_node_via_task_system(
        &self,
        node: &super::WorkflowNode,
        instance: &WorkflowInstance,
        app_handle: &AppHandle,
    ) -> Result<serde_json::Value, String> {
        let task_service = app_handle.state::<TaskService>().inner().clone();

        let (input, parameters) = self.build_node_payload(node, instance);
        let timeout_secs = node.config.timeout_seconds.unwrap_or(300);

        let payload = serde_json::json!({
            "instance_id": instance.id,
            "node_id": node.id,
            "story_id": instance.story_id,
            "node_type": node.node_type.to_string(),
            "input": input,
            "parameters": parameters,
        });

        let req = CreateTaskRequest {
            name: format!("Workflow Node {} ({})", node.id, node.node_type),
            description: Some(format!(
                "instance: {}, story: {}",
                instance.id, instance.story_id
            )),
            task_type: TaskType::WorkflowNode.to_string(),
            schedule_type: "once".to_string(),
            cron_pattern: None,
            payload: Some(payload.to_string()),
            enabled: Some(false),
            max_retries: Some(0),
            heartbeat_timeout_seconds: Some(timeout_secs as i32),
        };

        let task = task_service
            .create_task(req)
            .map_err(|e| format!("Failed to create workflow node task: {}", e))?;
        let task_id = task.id;

        task_service
            .trigger_task(&task_id)
            .map_err(|e| format!("Failed to trigger workflow node task: {}", e))?;

        let poll_interval = std::time::Duration::from_millis(200);
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() >= timeout {
                let _ = task_service.cancel_task(&task_id);
                return Err(format!(
                    "Workflow node task {} timed out ({} seconds)",
                    task_id, timeout_secs
                ));
            }

            match task_service.get_task(&task_id) {
                Ok(Some(task)) => match task.status {
                    TaskStatus::Completed => {
                        let result_json = task.result.unwrap_or_else(|| "{}".to_string());
                        return serde_json::from_str(&result_json)
                            .map_err(|e| format!("Failed to parse workflow node result: {}", e));
                    }
                    TaskStatus::Failed => {
                        let err = task
                            .error_message
                            .unwrap_or_else(|| "Workflow node task failed".to_string());
                        return Err(err);
                    }
                    TaskStatus::Cancelled => {
                        return Err("Workflow node task was cancelled".to_string());
                    }
                    _ => {}
                },
                Ok(None) => {
                    return Err(format!("Workflow node task {} not found", task_id));
                }
                Err(e) => {
                    return Err(format!("Failed to get workflow node task status: {}", e));
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Get next executable nodes based on current state
    pub fn get_next_nodes(
        &self,
        instance: &WorkflowInstance,
        workflow_nodes: &[super::WorkflowNode],
        workflow_edges: &[super::WorkflowEdge],
    ) -> Vec<String> {
        let mut next_nodes = Vec::new();
        let completed: std::collections::HashSet<String> =
            instance.context.completed_nodes.iter().cloned().collect();

        for node in workflow_nodes {
            // Skip already processed nodes
            if let Some(state) = instance.node_states.get(&node.id) {
                if state.status != NodeExecutionStatus::Pending {
                    continue;
                }
            }

            // P0-5 修复: 检查所有入边的前驱节点已完成，且边条件满足
            let incoming_edges: Vec<&super::WorkflowEdge> = workflow_edges
                .iter()
                .filter(|e| e.to_node == node.id)
                .collect();

            let all_deps_satisfied = incoming_edges.iter().all(|edge| {
                // 前驱节点必须已完成
                if !completed.contains(&edge.from_node) {
                    return false;
                }
                // 如果有 condition，必须满足
                if let Some(condition) = &edge.condition {
                    condition.evaluate(&instance.context.variables)
                } else {
                    true
                }
            });

            if all_deps_satisfied || incoming_edges.is_empty() {
                next_nodes.push(node.id.clone());
            }
        }

        next_nodes
    }

    /// Update node execution status
    pub fn update_node_status(
        &self,
        instance: &mut WorkflowInstance,
        node_id: &str,
        status: NodeExecutionStatus,
        output: Option<serde_json::Value>,
        error: Option<String>,
    ) {
        if let Some(state) = instance.node_states.get_mut(node_id) {
            state.status = status.clone();
            state.output = output;
            state.error = error;

            match status {
                NodeExecutionStatus::Running => {
                    state.started_at = Some(chrono::Utc::now());
                }
                NodeExecutionStatus::Completed => {
                    state.completed_at = Some(chrono::Utc::now());
                    if !instance
                        .context
                        .completed_nodes
                        .contains(&node_id.to_string())
                    {
                        instance.context.completed_nodes.push(node_id.to_string());
                    }
                }
                NodeExecutionStatus::Failed => {
                    instance.context.failed_nodes.push(node_id.to_string());
                }
                _ => {}
            }
        }
    }

    /// Check if workflow is complete
    pub fn is_workflow_complete(
        &self,
        instance: &WorkflowInstance,
        workflow_nodes: &[super::WorkflowNode],
    ) -> bool {
        let end_nodes: Vec<String> = workflow_nodes
            .iter()
            .filter(|n| matches!(n.node_type, super::NodeType::End))
            .map(|n| n.id.clone())
            .collect();

        end_nodes.iter().all(|end_id| {
            instance
                .node_states
                .get(end_id)
                .map(|s| s.status == NodeExecutionStatus::Completed)
                .unwrap_or(false)
        })
    }
}

fn get_last_content(variables: &std::collections::HashMap<String, serde_json::Value>) -> String {
    variables
        .values()
        .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
        .last()
        .unwrap_or("")
        .to_string()
}

fn get_last_score(variables: &std::collections::HashMap<String, serde_json::Value>) -> f64 {
    variables
        .values()
        .filter_map(|v| v.get("score").and_then(|s| s.as_f64()))
        .last()
        .unwrap_or(0.0)
}

/// 轻量级条件表达式求值（P1-10 修复）
/// 支持:
/// - 硬编码 "true" / "1"
/// - 数值比较: "{{score}} > 0.7", "{{count}} >= 5"
/// - 字符串相等: "{{status}} == \"approved\""
/// - 变量存在性: "{{var}}"（非空即为 true）
fn evaluate_condition(
    condition: &str,
    variables: &std::collections::HashMap<String, serde_json::Value>,
) -> bool {
    let trimmed = condition.trim();
    if trimmed == "true" || trimmed == "1" {
        return true;
    }
    if trimmed == "false" || trimmed == "0" {
        return false;
    }

    // 替换 {{key}} 变量引用
    let mut expanded = trimmed.to_string();
    for (key, value) in variables {
        let placeholder = format!("{{{{{}}}}}", key);
        let replacement = match value {
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => value.to_string(),
        };
        expanded = expanded.replace(&placeholder, &replacement);
    }

    // 数值比较解析
    let ops = [
        (">=", 2usize),
        ("<=", 2),
        ("==", 2),
        ("!=", 2),
        (">", 1),
        ("<", 1),
    ];
    for (op, op_len) in ops {
        if let Some(pos) = expanded.find(op) {
            let left = expanded[..pos].trim();
            let right = expanded[pos + op_len..]
                .trim()
                .trim_matches(|c| c == '\"' || c == '\'');
            // 尝试数值比较
            if let (Ok(l), Ok(r)) = (left.parse::<f64>(), right.parse::<f64>()) {
                return match op {
                    ">=" => l >= r,
                    "<=" => l <= r,
                    "==" => (l - r).abs() < f64::EPSILON,
                    "!=" => (l - r).abs() >= f64::EPSILON,
                    ">" => l > r,
                    "<" => l < r,
                    _ => false,
                };
            }
            // 字符串比较
            return match op {
                "==" => left == right,
                "!=" => left != right,
                _ => false,
            };
        }
    }

    // 无运算符时，检查展开后的字符串是否为 truthy
    !expanded.is_empty() && expanded != "false" && expanded != "0" && expanded != "null"
}

impl Default for WorkflowScheduler {
    fn default() -> Self {
        Self::new()
    }
}
