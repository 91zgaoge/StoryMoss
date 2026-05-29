#![allow(dead_code)]
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod loader;
pub mod scheduler;
pub use loader::{LoadedWorkflow, WorkflowLoader};
pub use scheduler::WorkflowScheduler;

/// Workflow definition - DAG structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub created_at: DateTime<Utc>,
}

/// Workflow node types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub config: NodeConfig,
    pub position: Option<NodePosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Start,
    WriteChapter,
    Inspect,
    Revise,
    VectorIndex,
    AnalyzePlot,
    Parallel,
    Condition,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub parameters: HashMap<String, serde_json::Value>,
    pub timeout_seconds: Option<u64>,
    pub retry_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

/// Workflow edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub id: String,
    pub from_node: String,
    pub to_node: String,
    pub condition: Option<EdgeCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionOperator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
}

impl EdgeCondition {
    /// Evaluate this condition against workflow context variables.
    pub fn evaluate(&self, variables: &HashMap<String, serde_json::Value>) -> bool {
        let field_value = variables.get(&self.field);
        match self.operator {
            ConditionOperator::Eq => field_value.map(|v| v == &self.value).unwrap_or(false),
            ConditionOperator::Neq => field_value.map(|v| v != &self.value).unwrap_or(true),
            ConditionOperator::Gt => match (field_value, &self.value) {
                (Some(serde_json::Value::Number(a)), serde_json::Value::Number(b)) => a
                    .as_f64()
                    .zip(b.as_f64())
                    .map(|(a, b)| a > b)
                    .unwrap_or(false),
                _ => false,
            },
            ConditionOperator::Gte => match (field_value, &self.value) {
                (Some(serde_json::Value::Number(a)), serde_json::Value::Number(b)) => a
                    .as_f64()
                    .zip(b.as_f64())
                    .map(|(a, b)| a >= b)
                    .unwrap_or(false),
                _ => false,
            },
            ConditionOperator::Lt => match (field_value, &self.value) {
                (Some(serde_json::Value::Number(a)), serde_json::Value::Number(b)) => a
                    .as_f64()
                    .zip(b.as_f64())
                    .map(|(a, b)| a < b)
                    .unwrap_or(false),
                _ => false,
            },
            ConditionOperator::Lte => match (field_value, &self.value) {
                (Some(serde_json::Value::Number(a)), serde_json::Value::Number(b)) => a
                    .as_f64()
                    .zip(b.as_f64())
                    .map(|(a, b)| a <= b)
                    .unwrap_or(false),
                _ => false,
            },
            ConditionOperator::Contains => match (field_value, &self.value) {
                (Some(serde_json::Value::String(a)), serde_json::Value::String(b)) => a.contains(b),
                (Some(serde_json::Value::Array(a)), serde_json::Value::String(b)) => {
                    a.iter().any(|v| v.as_str() == Some(b))
                }
                _ => false,
            },
            ConditionOperator::NotContains => match (field_value, &self.value) {
                (Some(serde_json::Value::String(a)), serde_json::Value::String(b)) => {
                    !a.contains(b)
                }
                (Some(serde_json::Value::Array(a)), serde_json::Value::String(b)) => {
                    !a.iter().any(|v| v.as_str() == Some(b))
                }
                _ => true,
            },
        }
    }
}

/// Workflow execution instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    pub id: String,
    pub workflow_id: String,
    pub story_id: String,
    pub status: WorkflowStatus,
    pub context: ExecutionContext,
    pub node_states: HashMap<String, NodeState>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub retry_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub variables: HashMap<String, serde_json::Value>,
    pub current_node_id: Option<String>,
    pub completed_nodes: Vec<String>,
    pub failed_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: String,
    pub status: NodeExecutionStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Workflow engine
pub struct WorkflowEngine {
    workflows: Arc<Mutex<HashMap<String, Workflow>>>,
    instances: Arc<Mutex<HashMap<String, WorkflowInstance>>>,
    pool: Option<crate::db::DbPool>,
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(Mutex::new(HashMap::new())),
            instances: Arc::new(Mutex::new(HashMap::new())),
            pool: None,
        }
    }

    pub fn with_pool(pool: crate::db::DbPool) -> (Self, Vec<String>) {
        let mut engine = Self {
            workflows: Arc::new(Mutex::new(HashMap::new())),
            instances: Arc::new(Mutex::new(HashMap::new())),
            pool: Some(pool.clone()),
        };
        let mut restored_ids = Vec::new();
        // 从数据库恢复实例
        if let Err(e) = engine.load_instances_from_db(&pool) {
            log::warn!("[WorkflowEngine] Failed to load instances from db: {}", e);
        } else {
            let instances = engine.instances.lock().unwrap();
            for (id, instance) in instances.iter() {
                if matches!(
                    instance.status,
                    WorkflowStatus::Pending | WorkflowStatus::Running
                ) {
                    restored_ids.push(id.clone());
                }
            }
        }
        (engine, restored_ids)
    }

    fn load_instances_from_db(&mut self, pool: &crate::db::DbPool) -> Result<(), rusqlite::Error> {
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, instance_json FROM workflow_instances WHERE status IN ('Pending', \
             'Running', 'Paused')",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let json: String = row.get(1)?;
            Ok((id, json))
        })?;

        let mut instances = self.instances.lock().unwrap();
        for row in rows {
            let (id, json) = row?;
            match serde_json::from_str::<WorkflowInstance>(&json) {
                Ok(instance) => {
                    instances.insert(id, instance);
                }
                Err(e) => {
                    log::warn!(
                        "[WorkflowEngine] Failed to deserialize instance {}: {}",
                        id,
                        e
                    );
                }
            }
        }
        log::info!(
            "[WorkflowEngine] Loaded {} instances from database",
            instances.len()
        );
        Ok(())
    }

    fn save_instance_to_db(&self, instance: &WorkflowInstance) -> Result<(), rusqlite::Error> {
        let pool = match &self.pool {
            Some(p) => p,
            None => return Ok(()),
        };
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let json = serde_json::to_string(instance).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO workflow_instances (id, workflow_id, story_id, status, instance_json, \
             updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 status = excluded.status,
                 instance_json = excluded.instance_json,
                 updated_at = excluded.updated_at",
            rusqlite::params![
                instance.id,
                instance.workflow_id,
                instance.story_id,
                format!("{:?}", instance.status),
                json,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn register_workflow(&self, workflow: Workflow) -> Result<(), Box<dyn std::error::Error>> {
        let mut workflows = self.workflows.lock().unwrap();
        if self.has_cycle(&workflow.nodes, &workflow.edges) {
            return Err("Workflow contains cycle".into());
        }
        workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    pub fn create_instance(
        &self,
        workflow_id: &str,
        story_id: &str,
        initial_context: HashMap<String, serde_json::Value>,
    ) -> Result<WorkflowInstance, Box<dyn std::error::Error>> {
        let workflows = self.workflows.lock().unwrap();
        let workflow = workflows.get(workflow_id).ok_or("Workflow not found")?;

        let instance_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let mut node_states = HashMap::new();
        for node in &workflow.nodes {
            node_states.insert(
                node.id.clone(),
                NodeState {
                    node_id: node.id.clone(),
                    status: NodeExecutionStatus::Pending,
                    started_at: None,
                    completed_at: None,
                    output: None,
                    error: None,
                    attempts: 0,
                },
            );
        }

        let instance = WorkflowInstance {
            id: instance_id,
            workflow_id: workflow_id.to_string(),
            story_id: story_id.to_string(),
            status: WorkflowStatus::Pending,
            context: ExecutionContext {
                variables: initial_context,
                current_node_id: None,
                completed_nodes: vec![],
                failed_nodes: vec![],
            },
            node_states,
            started_at: now,
            completed_at: None,
            retry_count: None,
        };

        let mut instances = self.instances.lock().unwrap();
        instances.insert(instance.id.clone(), instance.clone());
        if let Err(e) = self.save_instance_to_db(&instance) {
            log::warn!(
                "[WorkflowEngine] Failed to persist new instance {}: {}",
                instance.id,
                e
            );
        }
        Ok(instance)
    }

    pub fn start_instance(&self, instance_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut instances = self.instances.lock().unwrap();
        let instance = instances.get_mut(instance_id).ok_or("Instance not found")?;

        if instance.status != WorkflowStatus::Pending {
            return Err("Instance already started".into());
        }

        instance.status = WorkflowStatus::Running;
        let workflows = self.workflows.lock().unwrap();
        let workflow = workflows
            .get(&instance.workflow_id)
            .ok_or("Workflow not found")?;
        let start_node = workflow
            .nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Start))
            .ok_or("No start node found")?;

        instance.context.current_node_id = Some(start_node.id.clone());
        Ok(())
    }

    pub fn get_instance(&self, instance_id: &str) -> Option<WorkflowInstance> {
        let instances = self.instances.lock().unwrap();
        instances.get(instance_id).cloned()
    }

    pub fn get_workflow(&self, workflow_id: &str) -> Option<Workflow> {
        let workflows = self.workflows.lock().unwrap();
        workflows.get(workflow_id).cloned()
    }

    pub fn update_instance(&self, instance: &WorkflowInstance) {
        let mut instances = self.instances.lock().unwrap();
        instances.insert(instance.id.clone(), instance.clone());
        if let Err(e) = self.save_instance_to_db(instance) {
            log::warn!(
                "[WorkflowEngine] Failed to persist instance {}: {}",
                instance.id,
                e
            );
        }
    }

    fn has_cycle(&self, nodes: &[WorkflowNode], edges: &[WorkflowEdge]) -> bool {
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        for edge in edges {
            adjacency
                .entry(edge.from_node.clone())
                .or_default()
                .push(edge.to_node.clone());
        }

        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        fn dfs(
            node: &str,
            adjacency: &HashMap<String, Vec<String>>,
            visited: &mut HashSet<String>,
            rec_stack: &mut HashSet<String>,
        ) -> bool {
            visited.insert(node.to_string());
            rec_stack.insert(node.to_string());

            if let Some(neighbors) = adjacency.get(node) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        if dfs(neighbor, adjacency, visited, rec_stack) {
                            return true;
                        }
                    } else if rec_stack.contains(neighbor) {
                        return true;
                    }
                }
            }
            rec_stack.remove(node);
            false
        }

        for node in nodes {
            if !visited.contains(&node.id) {
                if dfs(&node.id, &adjacency, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub mod templates {
    use super::*;

    pub fn standard_writing_workflow() -> Workflow {
        let now = Utc::now();
        Workflow {
            id: "standard-writing".to_string(),
            name: "Standard Writing Workflow".to_string(),
            description: "Write -> Inspect -> Index".to_string(),
            nodes: vec![
                WorkflowNode {
                    id: "start".to_string(),
                    name: "Start".to_string(),
                    node_type: NodeType::Start,
                    config: NodeConfig {
                        parameters: HashMap::new(),
                        timeout_seconds: None,
                        retry_count: None,
                    },
                    position: Some(NodePosition { x: 100.0, y: 100.0 }),
                },
                WorkflowNode {
                    id: "write".to_string(),
                    name: "Write Chapter".to_string(),
                    node_type: NodeType::WriteChapter,
                    config: NodeConfig {
                        parameters: HashMap::new(),
                        timeout_seconds: Some(300),
                        retry_count: Some(2),
                    },
                    position: Some(NodePosition { x: 300.0, y: 100.0 }),
                },
                WorkflowNode {
                    id: "inspect".to_string(),
                    name: "Inspect".to_string(),
                    node_type: NodeType::Inspect,
                    config: NodeConfig {
                        parameters: HashMap::new(),
                        timeout_seconds: Some(120),
                        retry_count: Some(1),
                    },
                    position: Some(NodePosition { x: 500.0, y: 100.0 }),
                },
                WorkflowNode {
                    id: "end".to_string(),
                    name: "End".to_string(),
                    node_type: NodeType::End,
                    config: NodeConfig {
                        parameters: HashMap::new(),
                        timeout_seconds: None,
                        retry_count: None,
                    },
                    position: Some(NodePosition { x: 700.0, y: 100.0 }),
                },
            ],
            edges: vec![
                WorkflowEdge {
                    id: "e1".to_string(),
                    from_node: "start".to_string(),
                    to_node: "write".to_string(),
                    condition: None,
                },
                WorkflowEdge {
                    id: "e2".to_string(),
                    from_node: "write".to_string(),
                    to_node: "inspect".to_string(),
                    condition: None,
                },
                WorkflowEdge {
                    id: "e3".to_string(),
                    from_node: "inspect".to_string(),
                    to_node: "end".to_string(),
                    condition: None,
                },
            ],
            created_at: now,
        }
    }
}
