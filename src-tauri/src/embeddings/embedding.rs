//! Text Embedding Module
//!
//! 增强版嵌入服务，支持：
//! - 批量文本嵌入
//! - 实体嵌入（名称+描述+属性拼接）
//! - Embedding 缓存
//! - 多模型支持（预留接口）
#![allow(dead_code)]

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, Once};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

static EMBEDDING_INITIALIZED: OnceCell<bool> = OnceCell::new();
static mut VOCAB: Option<HashMap<String, usize>> = None;
static EMBEDDING_INIT: Once = Once::new();

/// Embedding 缓存
static EMBEDDING_CACHE: OnceCell<Mutex<EmbeddingCache>> = OnceCell::new();

/// Embedding representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
    pub dimensions: usize,
    pub model: String,
}

/// 实体嵌入请求
#[derive(Debug, Clone)]
pub struct EntityEmbeddingRequest {
    pub entity_id: String,
    pub name: String,
    pub description: Option<String>,
    pub entity_type: String,
    pub attributes: HashMap<String, serde_json::Value>,
}

/// 嵌入缓存
pub struct EmbeddingCache {
    cache: HashMap<u64, Vec<f32>>,
    max_size: usize,
    hits: u64,
    misses: u64,
}

impl EmbeddingCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    /// 计算文本哈希
    fn hash_text(text: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    /// 获取缓存的嵌入
    pub fn get(&mut self, text: &str) -> Option<Vec<f32>> {
        let hash = Self::hash_text(text);
        if let Some(embedding) = self.cache.get(&hash) {
            self.hits += 1;
            Some(embedding.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    /// 设置缓存
    pub fn set(&mut self, text: &str, embedding: Vec<f32>) {
        // 如果缓存已满，清除最旧的 10%
        if self.cache.len() >= self.max_size {
            let to_remove: Vec<u64> = self.cache.keys().take(self.max_size / 10).cloned().collect();
            for key in to_remove {
                self.cache.remove(&key);
            }
        }
        
        let hash = Self::hash_text(text);
        self.cache.insert(hash, embedding);
    }

    /// 获取缓存统计
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
            hits: self.hits,
            misses: self.misses,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f32 / (self.hits + self.misses) as f32
            } else {
                0.0
            },
        }
    }

    /// 清除缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

/// 缓存统计
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f32,
}

/// 初始化嵌入模型
pub fn init_embedding_model() -> Result<(), Box<dyn std::error::Error>> {
    EMBEDDING_INIT.call_once(|| {
        let _ = EMBEDDING_INITIALIZED.set(true);
        unsafe {
            VOCAB = Some(HashMap::new());
        }
        let _ = EMBEDDING_CACHE.set(Mutex::new(EmbeddingCache::new(10000)));
        log::info!("Embedding module initialized (384-dim feature vectors with cache)");
    });
    Ok(())
}

/// 分词 - 支持中文和英文
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.to_lowercase().chars().collect();

    // 提取单字/单字符
    for ch in &chars {
        if ch.is_alphanumeric() || ch.is_ascii_punctuation() {
            tokens.push(ch.to_string());
        }
    }

    // 提取双字词/bigrams
    for window in chars.windows(2) {
        let bigram: String = window.iter().collect();
        if bigram.chars().any(|c| c.is_alphabetic() || c.is_numeric()) {
            tokens.push(bigram);
        }
    }

    // 提取单词 (英文)
    let word_chars: String = text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' })
        .collect();

    for word in word_chars.split_whitespace() {
        if word.len() > 2 {
            tokens.push(word.to_string());
        }
    }

    tokens
}

/// 基于词频的嵌入 (改进版TF特征)
pub fn embed_text(text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // 先检查缓存
    if let Ok(mut cache) = EMBEDDING_CACHE.get().unwrap().lock() {
        if let Some(cached) = cache.get(text) {
            return Ok(cached);
        }
    }

    const DIM: usize = 384;
    let mut features = vec![0.0f32; DIM];

    if text.is_empty() {
        return Ok(features);
    }

    let tokens = tokenize(text);

    if tokens.is_empty() {
        return Ok(features);
    }

    // 统计词频
    let mut token_counts: HashMap<String, usize> = HashMap::new();
    for token in &tokens {
        *token_counts.entry(token.clone()).or_insert(0) += 1;
    }

    // 使用哈希将词映射到固定维度
    for (token, count) in token_counts {
        // 使用FNV-1a哈希
        let hash = fnv1a_hash(&token);
        let idx = (hash % DIM as u64) as usize;
        let tf = 1.0 + (count as f32).ln().max(0.0); // log normalization
        features[idx] += tf;
    }

    // 添加位置编码信息
    let text_len = text.len().min(DIM);
    for i in 0..text_len.min(64) {
        features[DIM - 64 + i] = (text.chars().nth(i).unwrap_or(' ') as u32 as f32) / 65535.0;
    }

    // L2 归一化
    let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-6 {
        for x in &mut features {
            *x /= norm;
        }
    }

    // 存入缓存
    if let Ok(mut cache) = EMBEDDING_CACHE.get().unwrap().lock() {
        cache.set(text, features.clone());
    }

    Ok(features)
}

/// FNV-1a 哈希函数
fn fnv1a_hash(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 批量生成文本嵌入
pub fn embed_texts(texts: Vec<String>) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    texts.iter().map(|t| embed_text(t)).collect()
}

/// 异步版本：在 spawn_blocking 中执行 embed_text，避免阻塞异步运行时
pub async fn embed_text_async(text: String) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    tokio::task::spawn_blocking(move || {
        embed_text(&text).map_err(|e| {
            let msg = e.to_string();
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, msg)) as Box<dyn std::error::Error + Send + Sync>
        })
    })
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
}

/// 异步版本：在 spawn_blocking 中执行 embed_entity
pub async fn embed_entity_async(request: EntityEmbeddingRequest) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    tokio::task::spawn_blocking(move || {
        embed_entity(&request).map_err(|e| {
            let msg = e.to_string();
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, msg)) as Box<dyn std::error::Error + Send + Sync>
        })
    })
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
}

/// 生成实体嵌入
/// 将实体名称、描述、类型、属性拼接成文本后生成嵌入
pub fn embed_entity(request: &EntityEmbeddingRequest) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // 构建实体文本表示
    let mut text_parts = vec![format!("Name: {}", request.name)];
    
    // 添加类型信息
    text_parts.push(format!("Type: {}", request.entity_type));
    
    // 添加描述
    if let Some(desc) = &request.description {
        if !desc.is_empty() {
            text_parts.push(format!("Description: {}", desc));
        }
    }
    
    // 添加属性
    if !request.attributes.is_empty() {
        let attrs_text: Vec<String> = request.attributes
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect();
        text_parts.push(format!("Attributes: {}", attrs_text.join(", ")));
    }
    
    let full_text = text_parts.join(". ");
    embed_text(&full_text)
}

/// 批量生成实体嵌入
pub fn embed_entities(requests: Vec<EntityEmbeddingRequest>) -> Result<HashMap<String, Vec<f32>>, Box<dyn std::error::Error>> {
    let mut results = HashMap::new();
    for request in requests {
        let embedding = embed_entity(&request)?;
        results.insert(request.entity_id.clone(), embedding);
    }
    Ok(results)
}



/// 计算余弦相似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let min_len = a.len().min(b.len());
    let dot_product: f32 = a[..min_len].iter().zip(&b[..min_len]).map(|(x, y)| x * y).sum();
    dot_product // 向量已归一化
}

/// 获取嵌入维度
pub fn embedding_dim() -> usize {
    384
}

/// 获取缓存统计
pub fn get_cache_stats() -> Option<CacheStats> {
    EMBEDDING_CACHE.get()
        .and_then(|cache| cache.lock().ok())
        .map(|c| c.stats())
}

/// 清除嵌入缓存
pub fn clear_cache() {
    if let Some(cache) = EMBEDDING_CACHE.get() {
        if let Ok(mut c) = cache.lock() {
            c.clear();
            log::info!("Embedding cache cleared");
        }
    }
}

/// 场景嵌入请求
#[derive(Debug, Clone)]
pub struct SceneEmbeddingRequest {
    pub scene_id: String,
    pub title: Option<String>,
    pub content: String,
    pub dramatic_goal: Option<String>,
    pub characters_present: Vec<String>,
    pub setting: Option<String>,
}

/// 生成场景嵌入
pub fn embed_scene(request: &SceneEmbeddingRequest) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut text_parts = Vec::new();
    
    // 标题
    if let Some(title) = &request.title {
        text_parts.push(format!("Scene: {}", title));
    }
    
    // 戏剧目标
    if let Some(goal) = &request.dramatic_goal {
        text_parts.push(format!("Goal: {}", goal));
    }
    
    // 在场角色
    if !request.characters_present.is_empty() {
        text_parts.push(format!("Characters: {}", request.characters_present.join(", ")));
    }
    
    // 场景设置
    if let Some(setting) = &request.setting {
        text_parts.push(format!("Setting: {}", setting));
    }
    
    // 内容摘要（取前500字符）
    let content_preview: String = request.content.chars().take(500).collect();
    text_parts.push(format!("Content: {}", content_preview));
    
    let full_text = text_parts.join(". ");
    embed_text(&full_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_text() {
        init_embedding_model().ok();
        
        let vec1 = embed_text("Hello world").unwrap();
        let vec2 = embed_text("Hello world").unwrap();
        let vec3 = embed_text("Goodbye world").unwrap();

        assert_eq!(vec1.len(), 384);

        // Same text should produce same embedding (from cache)
        assert!(cosine_similarity(&vec1, &vec2) > 0.99);

        // Different text should have lower similarity
        let sim = cosine_similarity(&vec1, &vec3);
        assert!(sim < 0.9);
    }

    #[test]
    fn test_embed_entity() {
        init_embedding_model().ok();
        
        let mut attrs = HashMap::new();
        attrs.insert("age".to_string(), serde_json::json!(25));
        attrs.insert("profession".to_string(), serde_json::json!("wizard"));
        
        let request = EntityEmbeddingRequest {
            entity_id: "char_001".to_string(),
            name: "Gandalf".to_string(),
            description: Some("A powerful wizard".to_string()),
            entity_type: "Character".to_string(),
            attributes: attrs,
        };
        
        let embedding = embed_entity(&request).unwrap();
        assert_eq!(embedding.len(), 384);
    }

    #[test]
    fn test_cache() {
        init_embedding_model().ok();
        
        // 第一次嵌入（缓存未命中）
        let _ = embed_text("Cache test text");
        
        // 第二次嵌入（缓存命中）
        let _ = embed_text("Cache test text");
        
        let stats = get_cache_stats().unwrap();
        assert!(stats.hits >= 1);
        assert!(stats.misses >= 1);
    }
}
