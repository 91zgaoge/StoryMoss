use super::{WorkflowInstance, WorkflowStatus, NodeExecutionStatus, WorkflowEngine, NodeType};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// Workflow scheduler - manages task execution with an in-memory queue
pub struct WorkflowScheduler {
    queue: Arc<Mutex<VecDeque<String>>>,
}

impl WorkflowScheduler {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Queue a workflow instance for execution
    pub async fn schedule_execution(
        &self,
        instance_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("[WorkflowScheduler] Queuing workflow instance {} for execution", instance_id);
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(instance_id);
        Ok(())
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

        match self.run_instance(engine, app_handle, &instance_id).await {
            Ok(_) => Some(Ok(instance_id)),
            Err(e) => Some(Err(format!("Instance {} failed: {}", instance_id, e))),
        }
    }

    /// Run a single workflow instance to completion (serial node execution)
    async fn run_instance(
        &self,
        engine: &WorkflowEngine,
        app_handle: &AppHandle,
        instance_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("[WorkflowScheduler] Starting workflow instance {}", instance_id);

        // Get instance and workflow
        let (workflow, mut instance) = {
            let instance = engine.get_instance(instance_id)
                .ok_or("Instance not found")?;
            let workflow = engine.get_workflow(&instance.workflow_id)
                .ok_or("Workflow not found")?;
            (workflow, instance)
        };

        // Emit start event
        let _ = app_handle.emit("workflow-started", serde_json::json!({
            "instance_id": instance_id,
            "workflow_id": workflow.id,
            "workflow_name": workflow.name,
        }));

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

            for node_id in &next_nodes {
                let node = workflow.nodes.iter()
                    .find(|n| n.id == *node_id)
                    .ok_or("Node not found")?;

                // Update node status to Running
                self.update_node_status(&mut instance, node_id, NodeExecutionStatus::Running, None, None);
                engine.update_instance(&instance);

                // Emit node start event
                let _ = app_handle.emit("workflow-node-started", serde_json::json!({
                    "instance_id": instance_id,
                    "node_id": node_id,
                    "node_name": node.name,
                    "node_type": format!("{:?}", node.node_type),
                }));

                // Execute the node
                let result = self.execute_node(node, &instance, app_handle).await;

                match result {
                    Ok(output) => {
                        self.update_node_status(&mut instance, node_id, NodeExecutionStatus::Completed, Some(output.clone()), None);
                        instance.context.variables.insert(node_id.clone(), output);
                        engine.update_instance(&instance);

                        let _ = app_handle.emit("workflow-node-completed", serde_json::json!({
                            "instance_id": instance_id,
                            "node_id": node_id,
                        }));
                    }
                    Err(e) => {
                        self.update_node_status(&mut instance, node_id, NodeExecutionStatus::Failed, None, Some(e.clone()));
                        instance.status = WorkflowStatus::Failed;
                        engine.update_instance(&instance);

                        let _ = app_handle.emit("workflow-node-failed", serde_json::json!({
                            "instance_id": instance_id,
                            "node_id": node_id,
                            "error": e,
                        }));
                        return Err(format!("Node {} failed: {}", node_id, e).into());
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

        let _ = app_handle.emit("workflow-completed", serde_json::json!({
            "instance_id": instance_id,
            "workflow_id": workflow.id,
        }));

        log::info!("[WorkflowScheduler] Workflow instance {} completed successfully", instance_id);
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
            NodeType::Start => {
                Ok(serde_json::json!({ "started": true }))
            }
            NodeType::WriteChapter => {
                let story_id = instance.story_id.clone();
                let instruction = node.config.parameters.get("instruction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Continue writing the story")
                    .to_string();
                
                // Try to get previous content from upstream nodes
                let previous_content = instance.context.variables.values()
                    .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
                    .last()
                    .unwrap_or("")
                    .to_string();
                
                let input = if previous_content.is_empty() {
                    instruction
                } else {
                    format!("{instruction}\n\nPrevious content:\n{previous_content}")
                };
                
                let agent_service = crate::agents::service::AgentService::new(app_handle.clone());
                let context = crate::agents::AgentContext::minimal(story_id, String::new());
                let task = crate::agents::service::AgentTask {
                    id: uuid::Uuid::new_v4().to_string(),
                    agent_type: crate::agents::service::AgentType::Writer,
                    context,
                    input,
                    parameters: HashMap::new(),
                    tier: None,
                };
                
                match agent_service.execute_task(task).await {
                    Ok(result) => Ok(serde_json::json!({
                        "content": result.content,
                        "score": result.score,
                    })),
                    Err(e) => Err(format!("Writer execution failed: {}", e)),
                }
            }
            NodeType::Inspect => {
                let content = instance.context.variables.values()
                    .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
                    .last()
                    .unwrap_or("")
                    .to_string();
                
                if content.is_empty() {
                    return Ok(serde_json::json!({ "content": "", "score": 0.0, "warning": "No content to inspect" }));
                }
                
                let agent_service = crate::agents::service::AgentService::new(app_handle.clone());
                let context = crate::agents::AgentContext::minimal(instance.story_id.clone(), String::new());
                let task = crate::agents::service::AgentTask {
                    id: uuid::Uuid::new_v4().to_string(),
                    agent_type: crate::agents::service::AgentType::Inspector,
                    context,
                    input: content,
                    parameters: HashMap::new(),
                    tier: None,
                };
                
                match agent_service.execute_task(task).await {
                    Ok(result) => Ok(serde_json::json!({
                        "content": result.content,
                        "score": result.score,
                    })),
                    Err(e) => Err(format!("Inspector execution failed: {}", e)),
                }
            }
            NodeType::Revise => {
                let variables = &instance.context.variables;
                let content = variables.values()
                    .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
                    .last()
                    .unwrap_or("")
                    .to_string();
                let inspect_result = variables.values()
                    .filter_map(|v| v.get("score").and_then(|s| s.as_f64()))
                    .last()
                    .unwrap_or(0.0);
                
                if content.is_empty() {
                    return Ok(serde_json::json!({ "content": "", "score": 0.0, "warning": "No content to revise" }));
                }
                
                let instruction = format!(
                    "Please revise the following content based on the inspection score {:.0}%:\n\n{}",
                    inspect_result * 100.0,
                    content
                );
                
                let agent_service = crate::agents::service::AgentService::new(app_handle.clone());
                let context = crate::agents::AgentContext::minimal(instance.story_id.clone(), String::new());
                let task = crate::agents::service::AgentTask {
                    id: uuid::Uuid::new_v4().to_string(),
                    agent_type: crate::agents::service::AgentType::Writer,
                    context,
                    input: instruction,
                    parameters: HashMap::new(),
                    tier: None,
                };
                
                match agent_service.execute_task(task).await {
                    Ok(result) => Ok(serde_json::json!({
                        "content": result.content,
                        "score": result.score,
                    })),
                    Err(e) => Err(format!("Revision failed: {}", e)),
                }
            }
            NodeType::VectorIndex => {
                let content = instance.context.variables.values()
                    .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
                    .last()
                    .unwrap_or("")
                    .to_string();
                
                if content.len() > 50 {
                    let llm_service = crate::llm::LlmService::new(app_handle.clone());
                    let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service);
                    let ingest_content = crate::memory::ingest::IngestContent {
                        text: content.clone(),
                        source: format!("workflow:{}", instance.id),
                        story_id: instance.story_id.clone(),
                        scene_id: None,
                    };
                    
                    match pipeline.ingest(&ingest_content).await {
                        Ok(result) => Ok(serde_json::json!({
                            "indexed": true,
                            "entities": result.entities.len(),
                            "relations": result.relations.len(),
                        })),
                        Err(e) => {
                            log::warn!("[Workflow] Ingest failed: {}", e);
                            Ok(serde_json::json!({ "indexed": false, "error": e.to_string() }))
                        }
                    }
                } else {
                    Ok(serde_json::json!({ "indexed": false, "reason": "content too short" }))
                }
            }
            NodeType::AnalyzePlot => {
                let content = instance.context.variables.values()
                    .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
                    .last()
                    .unwrap_or("")
                    .to_string();
                
                if content.is_empty() {
                    return Ok(serde_json::json!({ "content": "", "score": 0.0, "warning": "No content to analyze" }));
                }
                
                let agent_service = crate::agents::service::AgentService::new(app_handle.clone());
                let context = crate::agents::AgentContext::minimal(instance.story_id.clone(), String::new());
                let task = crate::agents::service::AgentTask {
                    id: uuid::Uuid::new_v4().to_string(),
                    agent_type: crate::agents::service::AgentType::PlotAnalyzer,
                    context,
                    input: content,
                    parameters: HashMap::new(),
                    tier: None,
                };
                
                match agent_service.execute_task(task).await {
                    Ok(result) => Ok(serde_json::json!({
                        "content": result.content,
                        "score": result.score,
                    })),
                    Err(e) => Err(format!("Plot analysis failed: {}", e)),
                }
            }
            NodeType::Condition => {
                let condition = node.config.parameters.get("condition")
                    .and_then(|v| v.as_str())
                    .unwrap_or("true");
                let result = condition == "true" || condition == "1";
                Ok(serde_json::json!({ "condition_met": result }))
            }
            NodeType::Parallel => {
                // Simplified: mark as completed, parallel branches are handled by DAG topology
                Ok(serde_json::json!({ "parallel": true }))
            }
            NodeType::End => {
                Ok(serde_json::json!({ "completed": true }))
            }
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
        let completed: std::collections::HashSet<String> = instance.context.completed_nodes.iter().cloned().collect();

        for node in workflow_nodes {
            // Skip already processed nodes
            if let Some(state) = instance.node_states.get(&node.id) {
                if state.status != NodeExecutionStatus::Pending {
                    continue;
                }
            }

            // Check if all dependencies are completed
            let dependencies: Vec<String> = workflow_edges
                .iter()
                .filter(|e| e.to_node == node.id)
                .map(|e| e.from_node.clone())
                .collect();

            let all_deps_completed = dependencies.iter().all(|dep| completed.contains(dep));

            if all_deps_completed || dependencies.is_empty() {
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
                    if !instance.context.completed_nodes.contains(&node_id.to_string()) {
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
            instance.node_states.get(end_id)
                .map(|s| s.status == NodeExecutionStatus::Completed)
                .unwrap_or(false)
        })
    }
}

impl Default for WorkflowScheduler {
    fn default() -> Self {
        Self::new()
    }
}
