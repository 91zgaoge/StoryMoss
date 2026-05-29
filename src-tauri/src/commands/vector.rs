//! Vector commands

use tauri::State;

use crate::{db::DbPool, error::AppError, vector::SearchResult, VECTOR_STORE};

#[tauri::command(rename_all = "snake_case")]
pub async fn search_similar(
    story_id: String,
    query: String,
    top_k: Option<usize>,
    pool: State<'_, DbPool>,
) -> Result<Vec<SearchResult>, AppError> {
    use crate::embeddings::embed_text_async;

    let store = VECTOR_STORE
        .get()
        .ok_or(AppError::internal("Vector store not initialized"))?;
    let query_embedding = embed_text_async(query.clone())
        .await
        .map_err(AppError::from)?;

    store
        .search(&story_id, query_embedding, top_k.unwrap_or(5))
        .await
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn text_search_vectors(
    story_id: String,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<SearchResult>, AppError> {
    let store = VECTOR_STORE
        .get()
        .ok_or(AppError::internal("Vector store not initialized"))?;
    store
        .text_search(&story_id, &query, top_k.unwrap_or(5))
        .await
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hybrid_search_vectors(
    story_id: String,
    query: String,
    top_k: Option<usize>,
    pool: State<'_, DbPool>,
) -> Result<Vec<SearchResult>, AppError> {
    use crate::embeddings::embed_text_async;

    let store = VECTOR_STORE
        .get()
        .ok_or(AppError::internal("Vector store not initialized"))?;
    let query_embedding = embed_text_async(query.clone())
        .await
        .map_err(AppError::from)?;

    store
        .hybrid_search(&story_id, &query, query_embedding, top_k.unwrap_or(5))
        .await
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn embed_chapter(
    chapter_id: String,
    content: String,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    use crate::{embeddings::embed_text_async, vector::VectorRecord};

    let store = VECTOR_STORE
        .get()
        .ok_or(AppError::internal("Vector store not initialized"))?;
    let pool = pool.inner().clone();
    let chapter = crate::db::ChapterRepository::new(pool)
        .get_by_id(&chapter_id)
        .map_err(AppError::from)?;
    let (story_id, chapter_number) = match chapter {
        Some(c) => (c.story_id, c.chapter_number),
        None => (String::new(), 0),
    };
    let embedding = embed_text_async(content.clone())
        .await
        .map_err(AppError::from)?;

    let record = VectorRecord {
        id: format!("{}", uuid::Uuid::new_v4()),
        story_id,
        chapter_id,
        chapter_number,
        text: content.chars().take(500).collect(),
        record_type: "chapter".to_string(),
        embedding,
    };

    store.add_record(record).await.map_err(AppError::from)
}
