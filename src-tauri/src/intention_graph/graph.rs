//! SING 意图图存储层
//!
//! 提供 SQLite 持久化 + 内存缓存的混合存储方案。
//! 图查询先走内存缓存（热数据），缓存未命中时回查 SQLite。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Local};
use rusqlite::{params, OptionalExtension};

use crate::db::connection::DbPool;
use crate::error::AppError;

use super::models::*;

// ==================== 内存缓存 ====================

/// 内存中的图缓存：热数据加速查询
pub struct InMemoryGraphCache {
    intentions: RwLock<HashMap<String, IntentionNode>>,
    assets: RwLock<HashMap<String, AssetNode>>,
    intention_asset_edges: RwLock<HashMap<String, Vec<IntentionAssetEdge>>>, // key = intention_id
    asset_asset_edges: RwLock<HashMap<String, Vec<AssetAssetEdge>>>,       // key = source_asset_id
    embedding_cache: RwLock<HashMap<String, Vec<f32>>>,                    // key = node_id
}

impl InMemoryGraphCache {
    pub fn new() -> Self {
        Self {
            intentions: RwLock::new(HashMap::new()),
            assets: RwLock::new(HashMap::new()),
            intention_asset_edges: RwLock::new(HashMap::new()),
            asset_asset_edges: RwLock::new(HashMap::new()),
            embedding_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_intention(&self, id: &str) -> Option<IntentionNode> {
        self.intentions.read().ok()?.get(id).cloned()
    }

    pub fn get_asset(&self, id: &str) -> Option<AssetNode> {
        self.assets.read().ok()?.get(id).cloned()
    }

    pub fn get_intention_edges(&self, intention_id: &str) -> Vec<IntentionAssetEdge> {
        self.intention_asset_edges
            .read()
            .ok()
            .and_then(|m| m.get(intention_id).cloned())
            .unwrap_or_default()
    }

    pub fn get_asset_edges(&self, asset_id: &str) -> Vec<AssetAssetEdge> {
        self.asset_asset_edges
            .read()
            .ok()
            .and_then(|m| m.get(asset_id).cloned())
            .unwrap_or_default()
    }

    pub fn get_embedding(&self, node_id: &str) -> Option<Vec<f32>> {
        self.embedding_cache.read().ok()?.get(node_id).cloned()
    }

    pub fn insert_intention(&self, node: IntentionNode) {
        if let Ok(mut guard) = self.intentions.write() {
            if let Some(ref emb) = node.embedding {
                if let Ok(mut emb_guard) = self.embedding_cache.write() {
                    emb_guard.insert(node.id.clone(), emb.clone());
                }
            }
            guard.insert(node.id.clone(), node);
        }
    }

    pub fn insert_asset(&self, node: AssetNode) {
        if let Ok(mut guard) = self.assets.write() {
            if let Some(ref emb) = node.embedding {
                if let Ok(mut emb_guard) = self.embedding_cache.write() {
                    emb_guard.insert(node.id.clone(), emb.clone());
                }
            }
            guard.insert(node.id.clone(), node);
        }
    }

    pub fn insert_intention_edge(&self, edge: IntentionAssetEdge) {
        if let Ok(mut guard) = self.intention_asset_edges.write() {
            guard
                .entry(edge.intention_id.clone())
                .or_default()
                .push(edge);
        }
    }

    pub fn insert_asset_edge(&self, edge: AssetAssetEdge) {
        if let Ok(mut guard) = self.asset_asset_edges.write() {
            guard
                .entry(edge.source_asset_id.clone())
                .or_default()
                .push(edge);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.intentions.write() {
            guard.clear();
        }
        if let Ok(mut guard) = self.assets.write() {
            guard.clear();
        }
        if let Ok(mut guard) = self.intention_asset_edges.write() {
            guard.clear();
        }
        if let Ok(mut guard) = self.asset_asset_edges.write() {
            guard.clear();
        }
        if let Ok(mut guard) = self.embedding_cache.write() {
            guard.clear();
        }
    }

    pub fn all_intentions(&self) -> Vec<IntentionNode> {
        self.intentions
            .read()
            .ok()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn all_assets(&self) -> Vec<AssetNode> {
        self.assets
            .read()
            .ok()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for InMemoryGraphCache {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== SQLite 存储层 ====================

/// 意图图 SQLite Repository
#[derive(Clone)]
pub struct IntentionGraphRepository {
    pool: DbPool,
    cache: Arc<InMemoryGraphCache>,
}

impl IntentionGraphRepository {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            cache: Arc::new(InMemoryGraphCache::new()),
        }
    }

    pub fn with_cache(pool: DbPool, cache: Arc<InMemoryGraphCache>) -> Self {
        Self { pool, cache }
    }

    pub fn cache(&self) -> &Arc<InMemoryGraphCache> {
        &self.cache
    }

    // ------------------------------------------------------------------
    // IntentionNode CRUD
    // ------------------------------------------------------------------

    pub fn create_intention(&self, node: &IntentionNode) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let embedding_json = node
            .embedding
            .as_ref()
            .map(|e| serialize_embedding(e))
            .unwrap_or_default();
        let created_at = node.created_at.timestamp();
        let updated_at = node.updated_at.timestamp();

        conn.execute(
            "INSERT INTO intention_nodes (id, intent_type, verb, object, description, embedding, frequency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
               intent_type = excluded.intent_type,
               verb = excluded.verb,
               object = excluded.object,
               description = excluded.description,
               embedding = excluded.embedding,
               frequency = excluded.frequency,
               updated_at = excluded.updated_at",
            params![
                node.id,
                node.intent_type.to_string(),
                node.verb,
                node.object,
                node.description,
                embedding_json,
                node.frequency,
                created_at,
                updated_at,
            ],
        )
        .map_err(|e| AppError::internal(format!("Failed to create intention: {}", e)))?;

        self.cache.insert_intention(node.clone());
        Ok(())
    }

    pub fn get_intention(&self, id: &str) -> Result<Option<IntentionNode>, AppError> {
        // 1. Try cache first
        if let Some(node) = self.cache.get_intention(id) {
            return Ok(Some(node));
        }

        // 2. Fallback to SQLite
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, intent_type, verb, object, description, embedding, frequency, created_at, updated_at
                 FROM intention_nodes WHERE id = ?1",
            )
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let node = stmt
            .query_row(params![id], |row| {
                let embedding_json: String = row.get(5).unwrap_or_default();
                let embedding = if embedding_json.is_empty() {
                    None
                } else {
                    deserialize_embedding(&embedding_json)
                };

                Ok(IntentionNode {
                    id: row.get(0)?,
                    intent_type: row.get::<_, String>(1)?.parse().unwrap_or(IntentType::Atomic),
                    verb: row.get(2)?,
                    object: row.get(3)?,
                    description: row.get(4)?,
                    embedding,
                    frequency: row.get(6)?,
                    created_at: DateTime::from_timestamp(row.get(7)?, 0)
                        .map(|dt| dt.with_timezone(&Local))
                        .unwrap_or_else(Local::now),
                    updated_at: DateTime::from_timestamp(row.get(8)?, 0)
                        .map(|dt| dt.with_timezone(&Local))
                        .unwrap_or_else(Local::now),
                })
            })
            .optional()
            .map_err(|e| AppError::internal(format!("Query failed: {}", e)))?;

        if let Some(ref node) = node {
            self.cache.insert_intention(node.clone());
        }

        Ok(node)
    }

    pub fn list_intentions(&self, intent_type: Option<IntentType>) -> Result<Vec<IntentionNode>, AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let sql = if intent_type.is_some() {
            "SELECT id, intent_type, verb, object, description, embedding, frequency, created_at, updated_at
             FROM intention_nodes WHERE intent_type = ?1 ORDER BY frequency DESC"
        } else {
            "SELECT id, intent_type, verb, object, description, embedding, frequency, created_at, updated_at
             FROM intention_nodes ORDER BY frequency DESC"
        };

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let rows = if let Some(it) = intent_type {
            stmt.query_map(params![it.to_string()], Self::map_intention_row)?
        } else {
            stmt.query_map([], Self::map_intention_row)?
        };

        let mut nodes = Vec::new();
        for row in rows {
            if let Ok(node) = row {
                self.cache.insert_intention(node.clone());
                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    fn map_intention_row(row: &rusqlite::Row) -> rusqlite::Result<IntentionNode> {
        let embedding_json: String = row.get(5).unwrap_or_default();
        let embedding = if embedding_json.is_empty() {
            None
        } else {
            deserialize_embedding(&embedding_json)
        };

        Ok(IntentionNode {
            id: row.get(0)?,
            intent_type: row.get::<_, String>(1)?.parse().unwrap_or(IntentType::Atomic),
            verb: row.get(2)?,
            object: row.get(3)?,
            description: row.get(4)?,
            embedding,
            frequency: row.get(6)?,
            created_at: DateTime::from_timestamp(row.get(7)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
            updated_at: DateTime::from_timestamp(row.get(8)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
        })
    }

    // ------------------------------------------------------------------
    // AssetNode CRUD
    // ------------------------------------------------------------------

    pub fn create_asset(&self, node: &AssetNode) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let embedding_json = node
            .embedding
            .as_ref()
            .map(|e| serialize_embedding(e))
            .unwrap_or_default();
        let metadata_json = node
            .metadata
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or_default();
        let created_at = node.created_at.timestamp();
        let updated_at = node.updated_at.timestamp();

        conn.execute(
            "INSERT INTO asset_nodes (id, asset_type, name, description, embedding, capability_id, metadata, frequency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
               asset_type = excluded.asset_type,
               name = excluded.name,
               description = excluded.description,
               embedding = excluded.embedding,
               capability_id = excluded.capability_id,
               metadata = excluded.metadata,
               frequency = excluded.frequency,
               updated_at = excluded.updated_at",
            params![
                node.id,
                node.asset_type.to_string(),
                node.name,
                node.description,
                embedding_json,
                node.capability_id.as_deref().unwrap_or(""),
                metadata_json,
                node.frequency,
                created_at,
                updated_at,
            ],
        )
        .map_err(|e| AppError::internal(format!("Failed to create asset: {}", e)))?;

        self.cache.insert_asset(node.clone());
        Ok(())
    }

    pub fn get_asset(&self, id: &str) -> Result<Option<AssetNode>, AppError> {
        if let Some(node) = self.cache.get_asset(id) {
            return Ok(Some(node));
        }

        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, asset_type, name, description, embedding, capability_id, metadata, frequency, created_at, updated_at
                 FROM asset_nodes WHERE id = ?1",
            )
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let node = stmt
            .query_row(params![id], Self::map_asset_row)
            .optional()
            .map_err(|e| AppError::internal(format!("Query failed: {}", e)))?;

        if let Some(ref node) = node {
            self.cache.insert_asset(node.clone());
        }

        Ok(node)
    }

    pub fn list_assets(&self, asset_type: Option<AssetType>) -> Result<Vec<AssetNode>, AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let sql = if asset_type.is_some() {
            "SELECT id, asset_type, name, description, embedding, capability_id, metadata, frequency, created_at, updated_at
             FROM asset_nodes WHERE asset_type = ?1 ORDER BY frequency DESC"
        } else {
            "SELECT id, asset_type, name, description, embedding, capability_id, metadata, frequency, created_at, updated_at
             FROM asset_nodes ORDER BY frequency DESC"
        };

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let rows = if let Some(at) = asset_type {
            stmt.query_map(params![at.to_string()], Self::map_asset_row)?
        } else {
            stmt.query_map([], Self::map_asset_row)?
        };

        let mut nodes = Vec::new();
        for row in rows {
            if let Ok(node) = row {
                self.cache.insert_asset(node.clone());
                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    fn map_asset_row(row: &rusqlite::Row) -> rusqlite::Result<AssetNode> {
        let embedding_json: String = row.get(4).unwrap_or_default();
        let embedding = if embedding_json.is_empty() {
            None
        } else {
            deserialize_embedding(&embedding_json)
        };
        let metadata_json: String = row.get(6).unwrap_or_default();
        let metadata = if metadata_json.is_empty() {
            None
        } else {
            serde_json::from_str(&metadata_json).ok()
        };
        let cap_id: String = row.get(5).unwrap_or_default();

        Ok(AssetNode {
            id: row.get(0)?,
            asset_type: row.get::<_, String>(1)?.parse().unwrap_or(AssetType::Skill),
            name: row.get(2)?,
            description: row.get(3)?,
            embedding,
            capability_id: if cap_id.is_empty() { None } else { Some(cap_id) },
            metadata,
            frequency: row.get(7)?,
            created_at: DateTime::from_timestamp(row.get(8)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
            updated_at: DateTime::from_timestamp(row.get(9)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
        })
    }

    // ------------------------------------------------------------------
    // Intention-Asset Edge CRUD
    // ------------------------------------------------------------------

    pub fn create_intention_asset_edge(
        &self,
        edge: &IntentionAssetEdge,
    ) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let created_at = edge.created_at.timestamp();
        let updated_at = edge.updated_at.timestamp();
        let reason = edge.reason.as_deref().unwrap_or("");

        conn.execute(
            "INSERT INTO intention_asset_edges (intention_id, asset_id, edge_type, weight, reason, cooccurrence_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(intention_id, asset_id, edge_type) DO UPDATE SET
               weight = excluded.weight,
               reason = excluded.reason,
               cooccurrence_count = excluded.cooccurrence_count,
               updated_at = excluded.updated_at",
            params![
                edge.intention_id,
                edge.asset_id,
                edge.edge_type.to_string(),
                edge.weight,
                reason,
                edge.cooccurrence_count,
                created_at,
                updated_at,
            ],
        )
        .map_err(|e| AppError::internal(format!("Failed to create intention-asset edge: {}", e)))?;

        self.cache.insert_intention_edge(edge.clone());
        Ok(())
    }

    pub fn get_intention_edges(
        &self,
        intention_id: &str,
        edge_type: Option<IntentionAssetEdgeType>,
    ) -> Result<Vec<IntentionAssetEdge>, AppError> {
        // Try cache first
        if edge_type.is_none() {
            let cached = self.cache.get_intention_edges(intention_id);
            if !cached.is_empty() {
                return Ok(cached);
            }
        }

        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let sql = if edge_type.is_some() {
            "SELECT id, intention_id, asset_id, edge_type, weight, reason, cooccurrence_count, created_at, updated_at
             FROM intention_asset_edges WHERE intention_id = ?1 AND edge_type = ?2"
        } else {
            "SELECT id, intention_id, asset_id, edge_type, weight, reason, cooccurrence_count, created_at, updated_at
             FROM intention_asset_edges WHERE intention_id = ?1"
        };

        let mut stmt = conn.prepare(sql).map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let rows = if let Some(et) = edge_type {
            stmt.query_map(params![intention_id, et.to_string()], Self::map_intention_asset_edge_row)?
        } else {
            stmt.query_map(params![intention_id], Self::map_intention_asset_edge_row)?
        };

        let mut edges = Vec::new();
        for row in rows {
            if let Ok(edge) = row {
                self.cache.insert_intention_edge(edge.clone());
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    fn map_intention_asset_edge_row(row: &rusqlite::Row) -> rusqlite::Result<IntentionAssetEdge> {
        let reason: String = row.get(5).unwrap_or_default();
        Ok(IntentionAssetEdge {
            id: row.get(0)?,
            intention_id: row.get(1)?,
            asset_id: row.get(2)?,
            edge_type: row.get::<_, String>(3)?.parse().unwrap_or(IntentionAssetEdgeType::TriggeredBy),
            weight: row.get(4)?,
            reason: if reason.is_empty() { None } else { Some(reason) },
            cooccurrence_count: row.get(6)?,
            created_at: DateTime::from_timestamp(row.get(7)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
            updated_at: DateTime::from_timestamp(row.get(8)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
        })
    }

    // ------------------------------------------------------------------
    // Asset-Asset Edge CRUD
    // ------------------------------------------------------------------

    pub fn create_asset_asset_edge(&self, edge: &AssetAssetEdge) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let created_at = edge.created_at.timestamp();
        let updated_at = edge.updated_at.timestamp();

        conn.execute(
            "INSERT INTO asset_asset_edges (source_asset_id, target_asset_id, edge_type, weight, cooccurrence_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_asset_id, target_asset_id, edge_type) DO UPDATE SET
               weight = excluded.weight,
               cooccurrence_count = excluded.cooccurrence_count,
               updated_at = excluded.updated_at",
            params![
                edge.source_asset_id,
                edge.target_asset_id,
                edge.edge_type.to_string(),
                edge.weight,
                edge.cooccurrence_count,
                created_at,
                updated_at,
            ],
        )
        .map_err(|e| AppError::internal(format!("Failed to create asset-asset edge: {}", e)))?;

        self.cache.insert_asset_edge(edge.clone());
        Ok(())
    }

    pub fn get_asset_edges(
        &self,
        asset_id: &str,
        edge_type: Option<AssetAssetEdgeType>,
    ) -> Result<Vec<AssetAssetEdge>, AppError> {
        if edge_type.is_none() {
            let cached = self.cache.get_asset_edges(asset_id);
            if !cached.is_empty() {
                return Ok(cached);
            }
        }

        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let sql = if edge_type.is_some() {
            "SELECT id, source_asset_id, target_asset_id, edge_type, weight, cooccurrence_count, created_at, updated_at
             FROM asset_asset_edges WHERE source_asset_id = ?1 AND edge_type = ?2"
        } else {
            "SELECT id, source_asset_id, target_asset_id, edge_type, weight, cooccurrence_count, created_at, updated_at
             FROM asset_asset_edges WHERE source_asset_id = ?1"
        };

        let mut stmt = conn.prepare(sql).map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let rows = if let Some(et) = edge_type {
            stmt.query_map(params![asset_id, et.to_string()], Self::map_asset_asset_edge_row)?
        } else {
            stmt.query_map(params![asset_id], Self::map_asset_asset_edge_row)?
        };

        let mut edges = Vec::new();
        for row in rows {
            if let Ok(edge) = row {
                self.cache.insert_asset_edge(edge.clone());
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    fn map_asset_asset_edge_row(row: &rusqlite::Row) -> rusqlite::Result<AssetAssetEdge> {
        Ok(AssetAssetEdge {
            id: row.get(0)?,
            source_asset_id: row.get(1)?,
            target_asset_id: row.get(2)?,
            edge_type: row.get::<_, String>(3)?.parse().unwrap_or(AssetAssetEdgeType::ToolCooccur),
            weight: row.get(4)?,
            cooccurrence_count: row.get(5)?,
            created_at: DateTime::from_timestamp(row.get(6)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
            updated_at: DateTime::from_timestamp(row.get(7)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
        })
    }

    // ------------------------------------------------------------------
    // Execution Graph CRUD
    // ------------------------------------------------------------------

    pub fn create_execution_graph(&self, graph: &ExecutionGraph) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let created_at = graph.created_at.timestamp();
        let completed_at = graph.completed_at.map(|dt| dt.timestamp());
        let root_intention_id = graph.root_intention_id.as_deref().unwrap_or("");
        let story_id = graph.story_id.as_deref().unwrap_or("");
        let plan_json = graph.plan_json.as_deref().unwrap_or("");
        let result_json = graph.result_json.as_deref().unwrap_or("");
        let execution_time_ms = graph.execution_time_ms;

        conn.execute(
            "INSERT INTO execution_graphs (id, request_id, story_id, user_input, root_intention_id, status, plan_json, result_json, created_at, completed_at, execution_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               status = excluded.status,
               plan_json = excluded.plan_json,
               result_json = excluded.result_json,
               completed_at = excluded.completed_at,
               execution_time_ms = excluded.execution_time_ms",
            params![
                graph.id,
                graph.request_id,
                story_id,
                graph.user_input,
                root_intention_id,
                graph.status.to_string(),
                plan_json,
                result_json,
                created_at,
                completed_at,
                execution_time_ms,
            ],
        )
        .map_err(|e| AppError::internal(format!("Failed to create execution graph: {}", e)))?;

        Ok(())
    }

    pub fn get_execution_graph(&self, id: &str) -> Result<Option<ExecutionGraph>, AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, request_id, story_id, user_input, root_intention_id, status, plan_json, result_json, created_at, completed_at, execution_time_ms
                 FROM execution_graphs WHERE id = ?1",
            )
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let graph = stmt
            .query_row(params![id], |row| {
                let story_id: String = row.get(2).unwrap_or_default();
                let root_id: String = row.get(4).unwrap_or_default();
                let plan_json: String = row.get(6).unwrap_or_default();
                let result_json: String = row.get(7).unwrap_or_default();
                let completed_at: Option<i64> = row.get(9)?;
                let exec_ms: Option<i64> = row.get(10)?;

                Ok(ExecutionGraph {
                    id: row.get(0)?,
                    request_id: row.get(1)?,
                    story_id: if story_id.is_empty() { None } else { Some(story_id) },
                    user_input: row.get(3)?,
                    root_intention_id: if root_id.is_empty() { None } else { Some(root_id) },
                    status: row.get::<_, String>(5)?.parse().unwrap_or(ExecutionGraphStatus::Building),
                    plan_json: if plan_json.is_empty() { None } else { Some(plan_json) },
                    result_json: if result_json.is_empty() { None } else { Some(result_json) },
                    created_at: DateTime::from_timestamp(row.get(8)?, 0)
                        .map(|dt| dt.with_timezone(&Local))
                        .unwrap_or_else(Local::now),
                    completed_at: completed_at.map(|ts| DateTime::from_timestamp(ts, 0).map(|dt| dt.with_timezone(&Local)).unwrap_or_else(Local::now)),
                    execution_time_ms: exec_ms,
                })
            })
            .optional()
            .map_err(|e| AppError::internal(format!("Query failed: {}", e)))?;

        Ok(graph)
    }

    // ------------------------------------------------------------------
    // Execution Node CRUD
    // ------------------------------------------------------------------

    pub fn create_execution_node(&self, node: &ExecutionNode) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let created_at = node.created_at.timestamp();
        let completed_at = node.completed_at.map(|dt| dt.timestamp());
        let intention_id = node.intention_id.as_deref().unwrap_or("");
        let asset_id = node.asset_id.as_deref().unwrap_or("");
        let parameters = node.parameters.as_ref().map(|p| p.to_string()).unwrap_or_default();
        let depends_on = node.depends_on.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()).unwrap_or_default();
        let outputs = node.outputs.as_ref().map(|o| o.to_string()).unwrap_or_default();
        let exec_ms = node.execution_time_ms;

        conn.execute(
            "INSERT INTO execution_nodes (id, graph_id, intention_id, asset_id, status, parameters, depends_on, outputs, discovered_from, execution_time_ms, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
               status = excluded.status,
               parameters = excluded.parameters,
               depends_on = excluded.depends_on,
               outputs = excluded.outputs,
               execution_time_ms = excluded.execution_time_ms,
               completed_at = excluded.completed_at",
            params![
                node.id,
                node.graph_id,
                intention_id,
                asset_id,
                node.status.to_string(),
                parameters,
                depends_on,
                outputs,
                node.discovered_from.to_string(),
                exec_ms,
                created_at,
                completed_at,
            ],
        )
        .map_err(|e| AppError::internal(format!("Failed to create execution node: {}", e)))?;

        Ok(())
    }

    pub fn get_execution_nodes_by_graph(&self, graph_id: &str) -> Result<Vec<ExecutionNode>, AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, graph_id, intention_id, asset_id, status, parameters, depends_on, outputs, discovered_from, execution_time_ms, created_at, completed_at
                 FROM execution_nodes WHERE graph_id = ?1 ORDER BY created_at",
            )
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let rows = stmt.query_map(params![graph_id], Self::map_execution_node_row)?;

        let mut nodes = Vec::new();
        for row in rows {
            if let Ok(node) = row {
                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    fn map_execution_node_row(row: &rusqlite::Row) -> rusqlite::Result<ExecutionNode> {
        let intention_id: String = row.get(2).unwrap_or_default();
        let asset_id: String = row.get(3).unwrap_or_default();
        let parameters_json: String = row.get(5).unwrap_or_default();
        let depends_on_json: String = row.get(6).unwrap_or_default();
        let outputs_json: String = row.get(7).unwrap_or_default();
        let completed_at: Option<i64> = row.get(11)?;
        let exec_ms: Option<i64> = row.get(9)?;

        Ok(ExecutionNode {
            id: row.get(0)?,
            graph_id: row.get(1)?,
            intention_id: if intention_id.is_empty() { None } else { Some(intention_id) },
            asset_id: if asset_id.is_empty() { None } else { Some(asset_id) },
            status: row.get::<_, String>(4)?.parse().unwrap_or(ExecutionNodeStatus::Pending),
            parameters: if parameters_json.is_empty() { None } else { serde_json::from_str(&parameters_json).ok() },
            depends_on: if depends_on_json.is_empty() { None } else { serde_json::from_str(&depends_on_json).ok() },
            outputs: if outputs_json.is_empty() { None } else { serde_json::from_str(&outputs_json).ok() },
            discovered_from: row.get::<_, String>(8)?.parse().unwrap_or(DiscoverySource::Synthesis),
            execution_time_ms: exec_ms,
            created_at: DateTime::from_timestamp(row.get(10)?, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now),
            completed_at: completed_at.map(|ts| DateTime::from_timestamp(ts, 0).map(|dt| dt.with_timezone(&Local)).unwrap_or_else(Local::now)),
        })
    }

    pub fn get_recent_executions(&self, limit: i64) -> Result<Vec<ExecutionGraph>, AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, request_id, story_id, user_input, root_intention_id, status, plan_json, result_json, created_at, completed_at, execution_time_ms
                 FROM execution_graphs ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;

        let rows = stmt.query_map(params![limit], |row| {
            let story_id: String = row.get(2).unwrap_or_default();
            let root_id: String = row.get(4).unwrap_or_default();
            let plan_json: String = row.get(6).unwrap_or_default();
            let result_json: String = row.get(7).unwrap_or_default();
            let completed_at: Option<i64> = row.get(9)?;
            let exec_ms: Option<i64> = row.get(10)?;

            Ok(ExecutionGraph {
                id: row.get(0)?,
                request_id: row.get(1)?,
                story_id: if story_id.is_empty() { None } else { Some(story_id) },
                user_input: row.get(3)?,
                root_intention_id: if root_id.is_empty() { None } else { Some(root_id) },
                status: row.get::<_, String>(5)?.parse().unwrap_or(ExecutionGraphStatus::Building),
                plan_json: if plan_json.is_empty() { None } else { Some(plan_json) },
                result_json: if result_json.is_empty() { None } else { Some(result_json) },
                created_at: DateTime::from_timestamp(row.get(8)?, 0)
                    .map(|dt| dt.with_timezone(&Local))
                    .unwrap_or_else(Local::now),
                completed_at: completed_at.map(|ts| DateTime::from_timestamp(ts, 0).map(|dt| dt.with_timezone(&Local)).unwrap_or_else(Local::now)),
                execution_time_ms: exec_ms,
            })
        }).map_err(|e| AppError::internal(format!("Query failed: {}", e)))?;

        let mut graphs = Vec::new();
        for row in rows {
            if let Ok(graph) = row {
                graphs.push(graph);
            }
        }

        Ok(graphs)
    }

    // ------------------------------------------------------------------
    // Statistics
    // ------------------------------------------------------------------

    pub fn get_statistics(&self) -> Result<GraphStatistics, AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let intention_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM intention_nodes", [], |row| row.get(0))
            .unwrap_or(0);
        let asset_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM asset_nodes", [], |row| row.get(0))
            .unwrap_or(0);
        let ia_edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM intention_asset_edges", [], |row| row.get(0))
            .unwrap_or(0);
        let aa_edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM asset_asset_edges", [], |row| row.get(0))
            .unwrap_or(0);
        let exec_graph_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM execution_graphs", [], |row| row.get(0))
            .unwrap_or(0);
        let exec_node_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM execution_nodes", [], |row| row.get(0))
            .unwrap_or(0);

        let mut top_intentions = Vec::new();
        let mut stmt = conn
            .prepare("SELECT id, frequency FROM intention_nodes ORDER BY frequency DESC LIMIT 10")
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let freq: i32 = row.get(1)?;
            Ok((id, freq))
        }).map_err(|e| AppError::internal(format!("Query failed: {}", e)))?;
        for row in rows {
            if let Ok((id, freq)) = row {
                top_intentions.push((id, freq));
            }
        }

        let mut top_assets = Vec::new();
        let mut stmt = conn
            .prepare("SELECT id, frequency FROM asset_nodes ORDER BY frequency DESC LIMIT 10")
            .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let freq: i32 = row.get(1)?;
            Ok((id, freq))
        }).map_err(|e| AppError::internal(format!("Query failed: {}", e)))?;
        for row in rows {
            if let Ok((id, freq)) = row {
                top_assets.push((id, freq));
            }
        }

        Ok(GraphStatistics {
            intention_count,
            asset_count,
            intention_asset_edge_count: ia_edge_count,
            asset_asset_edge_count: aa_edge_count,
            execution_graph_count: exec_graph_count,
            execution_node_count: exec_node_count,
            top_intentions,
            top_assets,
        })
    }

    // ------------------------------------------------------------------
    // Warm-up: Load all hot data into memory cache
    // ------------------------------------------------------------------

    pub fn warm_up_cache(&self) -> Result<(), AppError> {
        log::info!("[IntentionGraph] Warming up in-memory cache...");

        // Load all intentions
        let intentions = self.list_intentions(None)?;
        for node in intentions {
            self.cache.insert_intention(node);
        }

        // Load all assets
        let assets = self.list_assets(None)?;
        for node in assets {
            self.cache.insert_asset(node);
        }

        // Load top 1000 intention-asset edges (hot edges)
        {
            let conn = self.pool.get().map_err(|e| {
                AppError::internal(format!("Failed to get connection: {}", e))
            })?;
            let mut stmt = conn
                .prepare("SELECT id, intention_id, asset_id, edge_type, weight, reason, cooccurrence_count, created_at, updated_at FROM intention_asset_edges ORDER BY weight DESC LIMIT 1000")
                .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;
            let rows = stmt.query_map([], Self::map_intention_asset_edge_row)?;
            for row in rows {
                if let Ok(edge) = row {
                    self.cache.insert_intention_edge(edge);
                }
            }
        }

        // Load top 1000 asset-asset edges
        {
            let conn = self.pool.get().map_err(|e| {
                AppError::internal(format!("Failed to get connection: {}", e))
            })?;
            let mut stmt = conn
                .prepare("SELECT id, source_asset_id, target_asset_id, edge_type, weight, cooccurrence_count, created_at, updated_at FROM asset_asset_edges ORDER BY weight DESC LIMIT 1000")
                .map_err(|e| AppError::internal(format!("Prepare failed: {}", e)))?;
            let rows = stmt.query_map([], Self::map_asset_asset_edge_row)?;
            for row in rows {
                if let Ok(edge) = row {
                    self.cache.insert_asset_edge(edge);
                }
            }
        }

        let stats = self.get_statistics()?;
        log::info!(
            "[IntentionGraph] Cache warmed up: {} intentions, {} assets, {} ia_edges, {} aa_edges",
            stats.intention_count,
            stats.asset_count,
            stats.intention_asset_edge_count,
            stats.asset_asset_edge_count
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // Batch operations
    // ------------------------------------------------------------------

    pub fn batch_create_intentions(&self, nodes: &[IntentionNode]) -> Result<(), AppError> {
        let mut conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let tx = conn.transaction().map_err(|e| AppError::internal(format!("Transaction failed: {}", e)))?;

        for node in nodes {
            let embedding_json = node
                .embedding
                .as_ref()
                .map(|e| serialize_embedding(e))
                .unwrap_or_default();
            let created_at = node.created_at.timestamp();
            let updated_at = node.updated_at.timestamp();

            tx.execute(
                "INSERT INTO intention_nodes (id, intent_type, verb, object, description, embedding, frequency, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   intent_type = excluded.intent_type,
                   verb = excluded.verb,
                   object = excluded.object,
                   description = excluded.description,
                   embedding = excluded.embedding,
                   frequency = excluded.frequency,
                   updated_at = excluded.updated_at",
                params![
                    node.id,
                    node.intent_type.to_string(),
                    node.verb,
                    node.object,
                    node.description,
                    embedding_json,
                    node.frequency,
                    created_at,
                    updated_at,
                ],
            )
            .map_err(|e| AppError::internal(format!("Batch insert intention failed: {}", e)))?;

            self.cache.insert_intention(node.clone());
        }

        tx.commit().map_err(|e| AppError::internal(format!("Commit failed: {}", e)))?;
        Ok(())
    }

    pub fn batch_create_assets(&self, nodes: &[AssetNode]) -> Result<(), AppError> {
        let mut conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        let tx = conn.transaction().map_err(|e| AppError::internal(format!("Transaction failed: {}", e)))?;

        for node in nodes {
            let embedding_json = node
                .embedding
                .as_ref()
                .map(|e| serialize_embedding(e))
                .unwrap_or_default();
            let metadata_json = node
                .metadata
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_default();
            let created_at = node.created_at.timestamp();
            let updated_at = node.updated_at.timestamp();

            tx.execute(
                "INSERT INTO asset_nodes (id, asset_type, name, description, embedding, capability_id, metadata, frequency, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                   asset_type = excluded.asset_type,
                   name = excluded.name,
                   description = excluded.description,
                   embedding = excluded.embedding,
                   capability_id = excluded.capability_id,
                   metadata = excluded.metadata,
                   frequency = excluded.frequency,
                   updated_at = excluded.updated_at",
                params![
                    node.id,
                    node.asset_type.to_string(),
                    node.name,
                    node.description,
                    embedding_json,
                    node.capability_id.as_deref().unwrap_or(""),
                    metadata_json,
                    node.frequency,
                    created_at,
                    updated_at,
                ],
            )
            .map_err(|e| AppError::internal(format!("Batch insert asset failed: {}", e)))?;

            self.cache.insert_asset(node.clone());
        }

        tx.commit().map_err(|e| AppError::internal(format!("Commit failed: {}", e)))?;
        Ok(())
    }

    pub fn increment_intention_frequency(&self, id: &str) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        conn.execute(
            "UPDATE intention_nodes SET frequency = frequency + 1, updated_at = ?1 WHERE id = ?2",
            params![Local::now().timestamp(), id],
        )
        .map_err(|e| AppError::internal(format!("Failed to increment frequency: {}", e)))?;

        if let Ok(mut guard) = self.cache.intentions.write() {
            if let Some(node) = guard.get_mut(id) {
                node.increment_frequency();
            }
        }

        Ok(())
    }

    pub fn increment_asset_frequency(&self, id: &str) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(|e| {
            AppError::internal(format!("Failed to get connection: {}", e))
        })?;

        conn.execute(
            "UPDATE asset_nodes SET frequency = frequency + 1, updated_at = ?1 WHERE id = ?2",
            params![Local::now().timestamp(), id],
        )
        .map_err(|e| AppError::internal(format!("Failed to increment frequency: {}", e)))?;

        if let Ok(mut guard) = self.cache.assets.write() {
            if let Some(node) = guard.get_mut(id) {
                node.frequency += 1;
                node.updated_at = Local::now();
            }
        }

        Ok(())
    }
}
