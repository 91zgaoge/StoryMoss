#![allow(dead_code)]
//! LanceDB Vector Store
//!
//! Real LanceDB-backed vector storage using ANN vector search.
//! Replaces the previous SQLite-compatible layer with true vector indexing.

use std::sync::Arc;

use arrow_array::{
    types::Float32Type, Array, FixedSizeListArray, Float32Array, Int32Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use datafusion_expr::{col, lit};
use futures::TryStreamExt;
use lancedb::{
    connect,
    index::Index,
    query::{ExecutableQuery, QueryBase},
    Connection, DistanceType, Table,
};
use serde::{Deserialize, Serialize};

const EMBEDDING_DIM: i32 = 384;
const TABLE_NAME: &str = "vector_records";
const VECTOR_COL: &str = "vector";

/// 向量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub story_id: String,
    pub chapter_id: String,
    pub chapter_number: i32,
    pub text: String,
    pub record_type: String,
    /// LitSeg: 叙事元数据（JSON，含 act_number 等）
    pub metadata: Option<String>,
    pub embedding: Vec<f32>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub story_id: String,
    pub chapter_id: String,
    pub chapter_number: i32,
    pub text: String,
    pub score: f32,
    /// LitSeg: 叙事元数据
    pub metadata: Option<String>,
}

/// LanceDB 向量存储
pub struct LanceVectorStore {
    db_path: String,
    db: Option<Connection>,
    table: Option<Table>,
}

/// Escape special LIKE pattern characters so user input is treated as literal
/// text.
fn escape_like_pattern(s: &str) -> String {
    // Backslash must be replaced first so we don't double-escape the escapes.
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl LanceVectorStore {
    pub fn new(db_path: String) -> Self {
        Self {
            db_path,
            db: None,
            table: None,
        }
    }

    fn schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("story_id", DataType::Utf8, false),
            Field::new("chapter_id", DataType::Utf8, false),
            Field::new("chapter_number", DataType::Int32, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("record_type", DataType::Utf8, false),
            Field::new("metadata", DataType::Utf8, true),
            Field::new(
                VECTOR_COL,
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM,
                ),
                false,
            ),
        ]))
    }

    fn empty_batch() -> Result<RecordBatch, Box<dyn std::error::Error + Send + Sync>> {
        let schema = Self::schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(Vec::<&str>::new())),
                Arc::new(StringArray::from(Vec::<&str>::new())),
                Arc::new(StringArray::from(Vec::<&str>::new())),
                Arc::new(Int32Array::from(Vec::<i32>::new())),
                Arc::new(StringArray::from(Vec::<&str>::new())),
                Arc::new(StringArray::from(Vec::<&str>::new())),
                Arc::new(StringArray::from(Vec::<Option<&str>>::new())),
                Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        std::iter::empty::<Option<Vec<Option<f32>>>>(),
                        EMBEDDING_DIM,
                    ),
                ),
            ],
        )?;
        Ok(batch)
    }

    fn records_to_batch(
        records: &[VectorRecord],
    ) -> Result<RecordBatch, Box<dyn std::error::Error + Send + Sync>> {
        let schema = Self::schema();
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        let story_ids: Vec<&str> = records.iter().map(|r| r.story_id.as_str()).collect();
        let chapter_ids: Vec<&str> = records.iter().map(|r| r.chapter_id.as_str()).collect();
        let chapter_numbers: Vec<i32> = records.iter().map(|r| r.chapter_number).collect();
        let texts: Vec<&str> = records.iter().map(|r| r.text.as_str()).collect();
        let record_types: Vec<&str> = records.iter().map(|r| r.record_type.as_str()).collect();
        let metadata: Vec<Option<&str>> = records.iter().map(|r| r.metadata.as_deref()).collect();
        let vectors: Vec<Option<Vec<Option<f32>>>> = records
            .iter()
            .map(|r| Some(r.embedding.iter().map(|&v| Some(v)).collect()))
            .collect();

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(story_ids)),
                Arc::new(StringArray::from(chapter_ids)),
                Arc::new(Int32Array::from(chapter_numbers)),
                Arc::new(StringArray::from(texts)),
                Arc::new(StringArray::from(record_types)),
                Arc::new(StringArray::from(metadata)),
                Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        vectors.into_iter(),
                        EMBEDDING_DIM,
                    ),
                ),
            ],
        )?;
        Ok(batch)
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = connect(&self.db_path).execute().await?;

        let table = match db.open_table(TABLE_NAME).execute().await {
            Ok(t) => {
                // LitSeg: 检查 schema 是否包含 metadata 列，如果不包含则重建表
                let schema = t
                    .schema()
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
                let has_metadata = schema.fields.iter().any(|f| f.name() == "metadata");
                if !has_metadata {
                    log::info!("[LanceVectorStore] Schema outdated, dropping and recreating table");
                    let _ = db.drop_table(TABLE_NAME, &[]).await;
                    let empty_batch = Self::empty_batch()?;
                    db.create_table(TABLE_NAME, empty_batch).execute().await?
                } else {
                    t
                }
            }
            Err(_) => {
                let empty_batch = Self::empty_batch()?;
                db.create_table(TABLE_NAME, empty_batch).execute().await?
            }
        };

        // 仅在数据量足够时尝试创建向量索引，失败时记录但不阻塞初始化
        if let Ok(count) = table.count_rows(None).await {
            if count >= 256 {
                if let Err(e) = table
                    .create_index(&[VECTOR_COL], Index::Auto)
                    .execute()
                    .await
                {
                    log::warn!("[LanceVectorStore] Failed to create vector index: {}", e);
                }
            }
        }

        self.db = Some(db);
        self.table = Some(table);
        log::info!("LanceDB vector store initialized at {}", self.db_path);
        Ok(())
    }

    fn table(&self) -> Result<&Table, Box<dyn std::error::Error + Send + Sync>> {
        self.table.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Table not initialized",
            )) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    /// Upsert a record (update if exists, insert if not)
    pub async fn upsert(
        &self,
        record: VectorRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let table = self.table()?;
        let batch = Self::records_to_batch(&[record])?;
        let reader = Box::new(RecordBatchIterator::new(vec![Ok(batch)], Self::schema()));

        let mut builder = table.merge_insert(&["id"]);
        builder.when_matched_update_all(None);
        builder.when_not_matched_insert_all();
        builder.execute(reader).await?;

        Ok(())
    }

    pub async fn add_record(
        &self,
        record: VectorRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.upsert(record).await
    }

    pub async fn search(
        &self,
        story_id: &str,
        query_embedding: Vec<f32>,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let table = self.table()?;

        let filter = col("story_id").eq(lit(story_id));
        let batches: Vec<RecordBatch> = table
            .query()
            .nearest_to(query_embedding.as_slice())?
            .column(VECTOR_COL)
            .distance_type(DistanceType::Cosine)
            .only_if_expr(filter)
            .limit(top_k)
            .execute()
            .await?
            .try_collect()
            .await?;

        Ok(Self::batches_to_results(batches))
    }

    fn batches_to_results(batches: Vec<RecordBatch>) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for batch in batches {
            let num_rows = batch.num_rows();
            let ids = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let story_ids = batch
                .column_by_name("story_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let chapter_ids = batch
                .column_by_name("chapter_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let chapter_numbers = batch
                .column_by_name("chapter_number")
                .and_then(|c| c.as_any().downcast_ref::<Int32Array>());
            let texts = batch
                .column_by_name("text")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let metadata_arr = batch
                .column_by_name("metadata")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            for i in 0..num_rows {
                let score = distances.map(|d| 1.0 - d.value(i)).unwrap_or(0.0);
                let metadata = metadata_arr.and_then(|a| {
                    if a.is_null(i) {
                        None
                    } else {
                        Some(a.value(i).to_string())
                    }
                });
                results.push(SearchResult {
                    id: ids.map(|a| a.value(i).to_string()).unwrap_or_default(),
                    story_id: story_ids
                        .map(|a| a.value(i).to_string())
                        .unwrap_or_default(),
                    chapter_id: chapter_ids
                        .map(|a| a.value(i).to_string())
                        .unwrap_or_default(),
                    chapter_number: chapter_numbers.map(|a| a.value(i)).unwrap_or(0),
                    text: texts.map(|a| a.value(i).to_string()).unwrap_or_default(),
                    score,
                    metadata,
                });
            }
        }
        results
    }

    /// 基于关键词的文本搜索（LanceDB filter fallback）
    pub async fn text_search(
        &self,
        story_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let table = self.table()?;
        // Use a parameterized story_id filter and a prefix LIKE on text to avoid
        // full table scans and SQL injection risks. Wildcard characters in the
        // user query are escaped so they are treated as literal text.
        let prefix = format!("{}%", escape_like_pattern(query));
        let filter = col("story_id")
            .eq(lit(story_id))
            .and(col("text").like(lit(prefix)));

        let batches: Vec<RecordBatch> = table
            .query()
            .only_if_expr(filter)
            .limit(top_k)
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut results = Vec::new();
        for batch in batches {
            let num_rows = batch.num_rows();
            let ids = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let story_ids = batch
                .column_by_name("story_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let chapter_ids = batch
                .column_by_name("chapter_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let chapter_numbers = batch
                .column_by_name("chapter_number")
                .and_then(|c| c.as_any().downcast_ref::<Int32Array>());
            let texts = batch
                .column_by_name("text")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            for i in 0..num_rows {
                results.push(SearchResult {
                    id: ids.map(|a| a.value(i).to_string()).unwrap_or_default(),
                    story_id: story_ids
                        .map(|a| a.value(i).to_string())
                        .unwrap_or_default(),
                    chapter_id: chapter_ids
                        .map(|a| a.value(i).to_string())
                        .unwrap_or_default(),
                    chapter_number: chapter_numbers.map(|a| a.value(i)).unwrap_or(0),
                    text: texts.map(|a| a.value(i).to_string()).unwrap_or_default(),
                    score: 0.8, // 基础文本匹配分数
                    metadata: None,
                });
            }
        }
        Ok(results)
    }

    /// 混合搜索：向量相似度 + 文本搜索，使用 RRF 融合
    pub async fn hybrid_search(
        &self,
        story_id: &str,
        query_text: &str,
        query_embedding: Vec<f32>,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // Run vector and text searches concurrently to reduce latency.
        let (vector_results, text_results) = tokio::try_join!(
            self.search(story_id, query_embedding, top_k * 2),
            self.text_search(story_id, query_text, top_k * 2),
        )?;

        // RRF fusion is CPU-bound ranking work; move it off the async runtime.
        let fused = tokio::task::spawn_blocking(
            move || -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
                const RRF_K: f32 = 60.0;

                let mut scores: std::collections::HashMap<String, f32> =
                    std::collections::HashMap::new();

                for (rank, r) in vector_results.iter().enumerate() {
                    let score = 1.0 / (RRF_K + rank as f32 + 1.0);
                    *scores.entry(r.id.clone()).or_insert(0.0) += score;
                }

                for (rank, r) in text_results.iter().enumerate() {
                    let score = 1.0 / (RRF_K + rank as f32 + 1.0);
                    *scores.entry(r.id.clone()).or_insert(0.0) += score;
                }

                let mut all_results: std::collections::HashMap<String, SearchResult> =
                    std::collections::HashMap::new();
                for r in vector_results.into_iter().chain(text_results.into_iter()) {
                    all_results.entry(r.id.clone()).or_insert(r);
                }

                let mut fused: Vec<SearchResult> = all_results
                    .into_iter()
                    .map(|(id, mut r)| {
                        r.score = scores.get(&id).copied().unwrap_or(0.0);
                        r
                    })
                    .collect();

                fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
                fused.truncate(top_k);

                Ok(fused)
            },
        )
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)??;

        Ok(fused)
    }

    pub async fn delete(&self, id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let table = self.table()?;
        let safe_id = id.replace("'", "''");
        table.delete(&format!("id = '{}'", safe_id)).await?;
        Ok(())
    }

    pub async fn delete_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let table = self.table()?;
        let safe_chapter_id = chapter_id.replace("'", "''");
        table
            .delete(&format!("chapter_id = '{}'", safe_chapter_id))
            .await?;
        Ok(())
    }

    pub async fn count(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let table = self.table()?;
        Ok(table.count_rows(None).await?)
    }

    fn to_memory_result(r: SearchResult) -> crate::memory::query::SearchResult {
        let mut metadata = serde_json::json!({
            "story_id": r.story_id,
            "chapter_id": r.chapter_id,
            "chapter_number": r.chapter_number,
        });
        // LitSeg: 合并 narrative 元数据
        if let Some(ref meta_str) = r.metadata {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(meta_str) {
                if let Some(obj) = metadata.as_object_mut() {
                    if let Some(parsed_obj) = parsed.as_object() {
                        for (k, v) in parsed_obj {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }
        crate::memory::query::SearchResult {
            id: r.id,
            content: r.text,
            score: r.score,
            source_type: crate::memory::query::SourceType::Scene,
            metadata,
        }
    }
}

#[async_trait::async_trait]
impl crate::memory::query::VectorStore for LanceVectorStore {
    async fn search_with_token(
        &self,
        story_id: &str,
        token: &str,
        limit: usize,
    ) -> Result<Vec<crate::memory::query::SearchResult>, Box<dyn std::error::Error + Send + Sync>>
    {
        let results = self.text_search(story_id, token, limit).await?;
        Ok(results
            .into_iter()
            .map(LanceVectorStore::to_memory_result)
            .collect())
    }

    async fn search_with_embedding(
        &self,
        story_id: &str,
        embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<crate::memory::query::SearchResult>, Box<dyn std::error::Error + Send + Sync>>
    {
        let results = self.search(story_id, embedding, limit).await?;
        Ok(results
            .into_iter()
            .map(LanceVectorStore::to_memory_result)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_record(id: &str, story_id: &str, chapter_id: &str) -> VectorRecord {
        let mut embedding = vec![0.0f32; EMBEDDING_DIM as usize];
        if id == "r1" {
            embedding[0] = 0.1;
            embedding[1] = 0.2;
            embedding[2] = 0.3;
            embedding[3] = 0.4;
        } else {
            embedding[0] = 0.9;
            embedding[1] = 0.8;
            embedding[2] = 0.7;
            embedding[3] = 0.6;
        }
        VectorRecord {
            id: id.to_string(),
            story_id: story_id.to_string(),
            chapter_id: chapter_id.to_string(),
            chapter_number: 1,
            text: "测试文本".to_string(),
            record_type: "chapter".to_string(),
            metadata: None,
            embedding,
        }
    }

    #[tokio::test]
    async fn test_persistence() {
        let db_uri = format!("memory://test_{}", uuid::Uuid::new_v4());

        // Phase 1: Create store, add records
        {
            let mut store = LanceVectorStore::new(db_uri.clone());
            store.init().await.unwrap();

            let record = create_test_record("r1", "story_1", "chap_1");
            store.add_record(record).await.unwrap();

            let record2 = create_test_record("r2", "story_1", "chap_2");
            store.add_record(record2).await.unwrap();

            assert_eq!(store.count().await.unwrap(), 2);

            let query = {
                let mut v = vec![0.0f32; EMBEDDING_DIM as usize];
                v[0] = 0.1;
                v[1] = 0.2;
                v[2] = 0.3;
                v[3] = 0.4;
                v
            };
            let results = store.search("story_1", query, 5).await.unwrap();
            assert_eq!(results.len(), 2);
        }

        // Phase 2: Re-open with same URI (memory DB is fresh each time, so this just
        // tests struct)
        {
            let mut store = LanceVectorStore::new(db_uri.clone());
            store.init().await.unwrap();
            // Memory DB is not actually persisted across instances, so count is 0
            // This test mainly verifies init() doesn't panic
            assert_eq!(store.count().await.unwrap(), 0);
        }
    }

    #[tokio::test]
    async fn test_search_and_delete() {
        let db_uri = format!("memory://test_{}", uuid::Uuid::new_v4());
        let mut store = LanceVectorStore::new(db_uri);
        store.init().await.unwrap();

        let r1 = create_test_record("r1", "s1", "c1");
        let r2 = create_test_record("r2", "s1", "c2");
        store.add_record(r1).await.unwrap();
        store.add_record(r2).await.unwrap();

        let query_r2 = {
            let mut v = vec![0.0f32; EMBEDDING_DIM as usize];
            v[0] = 0.9;
            v[1] = 0.8;
            v[2] = 0.7;
            v[3] = 0.6;
            v
        };
        let results = store.search("s1", query_r2.clone(), 5).await.unwrap();
        assert_eq!(results.len(), 2);
        // Highest similarity should be r2
        assert_eq!(results[0].id, "r2");

        store.delete("r1").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        let query_r1 = {
            let mut v = vec![0.0f32; EMBEDDING_DIM as usize];
            v[0] = 0.1;
            v[1] = 0.2;
            v[2] = 0.3;
            v[3] = 0.4;
            v
        };
        let results = store.search("s1", query_r1, 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "r2");
    }
}
