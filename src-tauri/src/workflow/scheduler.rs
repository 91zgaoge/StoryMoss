use super::{WorkflowInstance, WorkflowStatus, NodeExecutionStatus, WorkflowEngine};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

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
    pub fn execute_next(
        &self,
        engine: &WorkflowEngine,
    ) -> Option<Result<String, String>> {
        let instance_id = {
            let mut queue = self.queue.lock().unwrap();
            queue.pop_front()?
        };

        match self.run_instance(engine, &instance_id) {
            Ok(_) => Some(Ok(instance_id)),
            Err(e) => Some(Err(format!("Instance {} failed: {}", instance_id, e))),
        }
    }

    /// Run a single workflow instance to completion (serial node execution)
    fn run_instance(
        &self,
        _engine: &WorkflowEngine,
        instance_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("[WorkflowScheduler] Instance {} queued but real node execution not yet implemented", instance_id);
        Err("Workflow node execution is not yet implemented. Use CreationWorkflowEngine for full workflow execution.".into())
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
