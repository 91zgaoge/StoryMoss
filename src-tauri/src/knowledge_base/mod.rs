pub mod commands;

use serde::{Deserialize, Serialize};

use crate::vector::{LanceVectorStore, VectorRecord};

/// 知识库导入结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbImportResult {
    pub chunks_imported: usize,
    pub vectors_indexed: usize,
    pub duration_ms: u64,
}

/// 知识库搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbSearchResult {
    pub id: String,
    pub chapter_number: i32,
    pub text: String,
    pub score: f32,
    pub search_type: String,
}

/// 知识库统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbStats {
    pub total_vectors: usize,
    pub total_chapters: usize,
    pub last_imported_at: Option<String>,
}

/// 文本分块（Vela 同款算法）
/// - 按段落分割
/// - 长段落按句号/感叹号/问号分割
/// - 每块最大 max_chars 字符
/// - 块间重叠 overlap 字符
pub fn chunk_text(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();
    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in paragraphs {
        if para.len() > max_chars {
            if !current.is_empty() {
                chunks.push(current.trim().to_string());
                current = String::new();
            }
            let sentences: Vec<&str> = para.split(['。', '！', '？']).collect();
            let mut sentence_chunk = String::new();
            for sentence in sentences {
                if sentence_chunk.len() + sentence.len() > max_chars && !sentence_chunk.is_empty() {
                    chunks.push(sentence_chunk.trim().to_string());
                    let skip = sentence_chunk.len().saturating_sub(overlap);
                    sentence_chunk = sentence_chunk.chars().skip(skip).collect();
                }
                sentence_chunk.push_str(sentence);
                if !sentence.is_empty() {
                    sentence_chunk.push('。');
                }
            }
            if !sentence_chunk.is_empty() {
                current = sentence_chunk;
            }
        } else {
            if current.len() + para.len() > max_chars && !current.is_empty() {
                chunks.push(current.trim().to_string());
                let skip = current.len().saturating_sub(overlap);
                current = current.chars().skip(skip).collect();
                current.push_str("\n\n");
                current.push_str(para);
            } else {
                if !current.is_empty() {
                    current.push_str("\n\n");
                }
                current.push_str(para);
            }
        }
    }

    if !current.is_empty() {
        chunks.push(current.trim().to_string());
    }

    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(text.trim().to_string());
    }

    chunks
}

/// 生成 chunk ID
fn chunk_id(story_id: &str, chapter_number: i32, index: usize) -> String {
    format!("{}_{}_{}", story_id, chapter_number, index)
}

/// 导入文本到知识库
pub async fn import_text(
    store: &LanceVectorStore,
    story_id: &str,
    chapter_number: i32,
    content: &str,
    source_label: &str,
) -> Result<KbImportResult, Box<dyn std::error::Error + Send + Sync>> {
    let start = std::time::Instant::now();

    // 1. 分块
    let chunks = chunk_text(content, 500, 50);
    let chunks_imported = chunks.len();

    // 2. 生成 embedding 并导入
    let mut vectors_indexed = 0;
    for (i, chunk) in chunks.iter().enumerate() {
        match crate::embeddings::embedding::embed_text_async(chunk.clone()).await {
            Ok(embedding) => {
                let record = VectorRecord {
                    id: chunk_id(story_id, chapter_number, i),
                    story_id: story_id.to_string(),
                    chapter_id: format!("{}_{}", story_id, chapter_number),
                    chapter_number,
                    text: chunk.clone(),
                    record_type: source_label.to_string(),
                    metadata: None,
                    embedding,
                };
                if let Err(e) = store.upsert(record).await {
                    log::warn!("[kb_import] 导入 chunk {} 失败: {}", i, e);
                } else {
                    vectors_indexed += 1;
                }
            }
            Err(e) => {
                log::warn!("[kb_import] 生成 embedding 失败 (chunk {}): {}", i, e);
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    log::info!(
        "[kb_import] story_id={}, chapter={}, chunks={}, vectors={}, duration={}ms",
        story_id,
        chapter_number,
        chunks_imported,
        vectors_indexed,
        duration_ms
    );

    Ok(KbImportResult {
        chunks_imported,
        vectors_indexed,
        duration_ms,
    })
}

/// 知识库搜索
pub async fn kb_search(
    store: &LanceVectorStore,
    story_id: &str,
    query: &str,
    top_k: usize,
    chapter_range: Option<(i32, i32)>,
    search_mode: &str,
) -> Result<Vec<KbSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
    let results = match search_mode {
        "vector" => {
            let embedding =
                crate::embeddings::embedding::embed_text_async(query.to_string()).await?;
            store.search(story_id, embedding, top_k).await?
        }
        "fts" => store.text_search(story_id, query, top_k).await?,
        "hybrid" | _ => {
            let embedding =
                crate::embeddings::embedding::embed_text_async(query.to_string()).await?;
            store
                .hybrid_search(story_id, query, embedding, top_k)
                .await?
        }
    };

    let mut kb_results: Vec<KbSearchResult> = results
        .into_iter()
        .map(|r| KbSearchResult {
            id: r.id,
            chapter_number: r.chapter_number,
            text: r.text,
            score: r.score,
            search_type: search_mode.to_string(),
        })
        .collect();

    // 章节范围过滤
    if let Some((from, to)) = chapter_range {
        kb_results.retain(|r| r.chapter_number >= from && r.chapter_number <= to);
    }

    Ok(kb_results)
}

/// 删除某章的向量记录
pub async fn delete_chapter_vectors(
    store: &LanceVectorStore,
    story_id: &str,
    chapter_number: i32,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let chapter_id = format!("{}_{}", story_id, chapter_number);
    store.delete_chapter(&chapter_id).await?;
    Ok(1)
}
