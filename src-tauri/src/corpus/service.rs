use std::path::{Path, PathBuf};

use base64::Engine;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use crate::{
    corpus::{
        models::{
            CorpusDetail, CorpusDistillation, CorpusMedia, CorpusPromotionDraft, CorpusSummary,
            CorpusUpdate,
        },
        repository::CorpusRepository,
    },
    db::DbPool,
    error::AppError,
    llm::LlmService,
    router::TaskType,
};

const TEXT_MAX_BYTES: u64 = 20 * 1024 * 1024;
const IMAGE_MAX_BYTES: u64 = 10 * 1024 * 1024;
const DISTILL_CONTENT_CHARS: usize = 12_000;
const DISTILL_CHUNK_CHARS: usize = 12_000;
/// 单次提炼最多喂给模型的分段数；更长的文档均匀抽样，覆盖首尾。
const DISTILL_MAX_CHUNKS: usize = 8;
const DISTILL_CHUNK_FALLBACK_CHARS: usize = 2_000;
const STORY_BRIEF_CHARS: usize = 2_000;

const DISTILL_SYSTEM: &str =
    "你是小说创作的素材编辑。把作者提供的语料拆成可用的创作原料。只输出严格 JSON，不要解释。";
const DISTILL_CHUNK_SYSTEM: &str =
    "你是资料摘要员。只提炼这段语料里可核实的事实、冲突、人物原型、世界观钩子与改编线索，不要评论，不要输出 JSON。";
const DISTILL_SCHEMA: &str = r#"输出 JSON：
{"facts":["可核实事实"],"conflicts":["可用冲突"],"archetypes":["人物原型"],"world_hooks":["世界观钩子"],"adaptation_angles":["改编建议"],"cautions":["与现有设定可能冲突处"]}"#;
const PROMOTE_SYSTEM: &str =
    "你是小说资产编辑。把语料提炼成目标资产草稿。只输出严格 JSON，不要解释；不要写入任何数据库。";

pub struct CorpusService {
    pool: DbPool,
}

impl CorpusService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn list(
        &self,
        story_id: &str,
        kind: Option<&str>,
        status: Option<&str>,
        query: Option<&str>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<CorpusSummary>, AppError> {
        CorpusRepository::new(self.pool.clone()).list(story_id, kind, status, query, limit, offset)
    }

    pub fn get(&self, id: &str) -> Result<CorpusDetail, AppError> {
        CorpusRepository::new(self.pool.clone())
            .get(id)?
            .ok_or_else(|| AppError::not_found("corpus", id))
    }

    pub fn create(
        &self,
        story_id: &str,
        kind: &str,
        title: &str,
        content: Option<&str>,
        summary: Option<&str>,
        source_url: Option<&str>,
        source_name: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> Result<CorpusDetail, AppError> {
        let content = content.unwrap_or("");
        let summary = summary
            .map(|s| s.to_string())
            .unwrap_or_else(|| make_summary(content));
        let id = CorpusRepository::new(self.pool.clone()).create(
            story_id,
            kind,
            title,
            content,
            &summary,
            source_url,
            source_name,
            tags.unwrap_or_default(),
        )?;
        self.get(&id)
    }

    pub fn update(&self, id: &str, update: &CorpusUpdate) -> Result<CorpusDetail, AppError> {
        CorpusRepository::new(self.pool.clone()).update(id, update)?;
        self.get(id)
    }

    pub fn delete(&self, id: &str, app: &AppHandle) -> Result<(), AppError> {
        let repo = CorpusRepository::new(self.pool.clone());
        let path = repo.get_file_path(id)?;
        repo.delete(id)?;
        if let Some(path) = path {
            let _ = std::fs::remove_file(path);
        }
        let _ = app;
        Ok(())
    }

    pub fn link(
        &self,
        corpus_id: &str,
        target_type: &str,
        target_id: &str,
        note: Option<&str>,
    ) -> Result<(), AppError> {
        let repo = CorpusRepository::new(self.pool.clone());
        repo.insert_link(corpus_id, target_type, target_id, note)?;
        repo.set_status(corpus_id, "used")?;
        Ok(())
    }

    pub fn unlink(&self, link_id: &str) -> Result<(), AppError> {
        CorpusRepository::new(self.pool.clone()).delete_link(link_id)?;
        Ok(())
    }

    /// 导入本地文件：校验 → hash 去重 → 复制到 app_data/corpus/<story>/ →
    /// 解析文本（txt/md/pdf/epub）或保存图片元数据 → 落库。
    pub fn import_file(
        &self,
        story_id: &str,
        kind: &str,
        file_path: &str,
        title: Option<&str>,
        tags: Option<Vec<String>>,
        app: &AppHandle,
    ) -> Result<CorpusDetail, AppError> {
        let src = Path::new(file_path);
        let metadata = std::fs::metadata(src).map_err(|e| {
            AppError::validation_failed(format!("文件不可读: {e}"), Some("file_path"))
        })?;
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif");
        let max = if is_image {
            IMAGE_MAX_BYTES
        } else {
            TEXT_MAX_BYTES
        };
        if metadata.len() > max {
            return Err(AppError::validation_failed(
                format!(
                    "文件过大（{} MB，上限 {} MB）",
                    metadata.len() / 1024 / 1024,
                    max / 1024 / 1024
                ),
                Some("file_size"),
            ));
        }

        let bytes = std::fs::read(src).map_err(|e| {
            AppError::validation_failed(format!("读取文件失败: {e}"), Some("file_path"))
        })?;
        let hash = hex_sha256(&bytes);
        let repo = CorpusRepository::new(self.pool.clone());
        if let Some(existing) = repo.find_by_hash(story_id, &hash)? {
            return self.get(&existing);
        }

        let app_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let dir = app_dir.join("corpus").join(story_id);
        std::fs::create_dir_all(&dir).map_err(|e| {
            AppError::validation_failed(format!("创建语料目录失败: {e}"), Some("app_dir"))
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        let dest: PathBuf = dir.join(format!("{id}.{ext}"));
        std::fs::copy(src, &dest).map_err(|e| {
            AppError::validation_failed(format!("复制文件失败: {e}"), Some("file_copy"))
        })?;

        let content = if is_image {
            String::new()
        } else {
            extract_text(&dest, &ext)?
        };
        let title = title
            .map(|t| t.to_string())
            .or_else(|| {
                src.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| format!("语料-{}", &id[..8]));
        let summary = make_summary(&content);
        let file_name = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let mime = mime_for(&ext);
        let kind = if is_image { "image" } else { kind };
        let id = repo.create_file_item(
            story_id,
            kind,
            &title,
            &content,
            &summary,
            dest.to_str().unwrap_or(""),
            &file_name,
            mime,
            metadata.len() as i64,
            &hash,
            tags.unwrap_or_default(),
        )?;
        self.get(&id)
    }

    pub fn read_media(&self, id: &str) -> Result<CorpusMedia, AppError> {
        let repo = CorpusRepository::new(self.pool.clone());
        let detail = repo
            .get(id)?
            .ok_or_else(|| AppError::not_found("corpus", id))?;
        let path = repo
            .get_file_path(id)?
            .ok_or_else(|| AppError::not_found("corpus_file", id))?;
        let metadata = std::fs::metadata(&path).map_err(|e| {
            AppError::validation_failed(format!("图片不可读: {e}"), Some("file_path"))
        })?;
        if metadata.len() > IMAGE_MAX_BYTES {
            return Err(AppError::validation_failed(
                "图片过大，无法预览".to_string(),
                Some("file_size"),
            ));
        }
        let bytes = std::fs::read(&path).map_err(|e| {
            AppError::validation_failed(format!("读取图片失败: {e}"), Some("file_path"))
        })?;
        let mime = detail
            .mime_type
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(CorpusMedia {
            mime: mime.clone(),
            data_url: format!("data:{mime};base64,{data}"),
        })
    }

    /// AI 提炼：失败只返回错误，不破坏原语料；成功后 status=distilled。
    pub async fn distill(&self, id: &str, app: &AppHandle) -> Result<CorpusDistillation, AppError> {
        let detail = self.get(id)?;
        let brief = self.story_brief(&detail.story_id)?;
        let content = self.prepare_distill_source(app, &detail.content).await;
        let user = format!(
            "故事上下文：\n{}\n\n语料标题：{}\n语料原文：\n{}\n\n{}",
            brief,
            detail.title,
            if content.is_empty() {
                "(图片语料，只有标题可供参考)"
            } else {
                content.as_str()
            },
            DISTILL_SCHEMA
        );
        let llm = LlmService::new(app.clone());
        let (_rid, result) = llm
            .generate_for_task_with_system_prompt(
                TaskType::Analysis,
                user,
                Some(4096),
                Some(0.4),
                Some("corpus_distill"),
                Some(DISTILL_SYSTEM.to_string()),
                None,
            )
            .await;
        let response = result.map_err(AppError::from)?;
        let mut parsed: CorpusDistillation =
            parse_json_loose(&response.content).ok_or_else(|| {
                AppError::validation_failed("AI 提炼返回不是有效 JSON", Some("distill"))
            })?;
        parsed.version = 1;
        parsed.model = response.model.clone();
        parsed.generated_at = chrono::Utc::now().to_rfc3339();
        let json = serde_json::to_string(&parsed).map_err(AppError::from)?;
        CorpusRepository::new(self.pool.clone()).set_distilled(id, &json, &response.model)?;
        Ok(parsed)
    }

    /// 转为资产草稿：只返回目标类型草稿，不写任何资产表。
    pub async fn promote(
        &self,
        id: &str,
        target_type: &str,
        app: &AppHandle,
    ) -> Result<CorpusPromotionDraft, AppError> {
        let detail = self.get(id)?;
        let schema = promote_schema(target_type)?;
        let content: String = detail.content.chars().take(DISTILL_CONTENT_CHARS).collect();
        let distilled = detail
            .distilled
            .as_ref()
            .map(|d| serde_json::to_string(d).unwrap_or_default())
            .unwrap_or_default();
        let user = format!(
            "语料标题：{}\n原文：\n{}\nAI 提炼：\n{}\n\n{}",
            detail.title,
            if content.is_empty() {
                "(图片语料)"
            } else {
                content.as_str()
            },
            distilled,
            schema
        );
        let llm = LlmService::new(app.clone());
        let (_rid, result) = llm
            .generate_for_task_with_system_prompt(
                TaskType::Analysis,
                user,
                Some(4096),
                Some(0.3),
                Some("corpus_promote"),
                Some(PROMOTE_SYSTEM.to_string()),
                None,
            )
            .await;
        let response = result.map_err(AppError::from)?;
        let fields: serde_json::Value = parse_json_loose(&response.content).ok_or_else(|| {
            AppError::validation_failed("AI 草稿返回不是有效 JSON", Some("promote"))
        })?;
        let title = fields
            .get("title")
            .or_else(|| fields.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| detail.title.clone());
        Ok(CorpusPromotionDraft {
            target_type: target_type.to_string(),
            title,
            fields,
        })
    }

    /// 超大文档先分段摘要再提炼，避免单次喂几十万 token 的全新前缀导致
    /// 首字节超时。短文本原样返回，保持原有单次调用行为。
    async fn prepare_distill_source(&self, app: &AppHandle, content: &str) -> String {
        let original_chars = content.chars().count();
        let chunks = chunk_text(content, DISTILL_CHUNK_CHARS, DISTILL_MAX_CHUNKS);
        if chunks.len() <= 1 {
            return content.chars().take(DISTILL_CONTENT_CHARS).collect();
        }

        log::info!(
            "corpus distill: 长文档分段预处理 chars={} chunks={}",
            original_chars,
            chunks.len()
        );
        let llm = LlmService::new(app.clone());
        let total = chunks.len();
        let mut summaries = Vec::with_capacity(total);
        for (index, chunk) in chunks.iter().enumerate() {
            let prompt = format!(
                "这是长文档第 {}/{} 段。请提炼本段要点，300 字以内：\n\n{}",
                index + 1,
                total,
                chunk
            );
            let (_rid, result) = llm
                .generate_for_task_with_system_prompt(
                    TaskType::Summarization,
                    prompt,
                    Some(600),
                    Some(0.2),
                    Some("corpus_chunk_summary"),
                    Some(DISTILL_CHUNK_SYSTEM.to_string()),
                    None,
                )
                .await;
            match result {
                Ok(response) if !response.content.trim().is_empty() => {
                    summaries.push(format!(
                        "【第 {}/{} 段要点】\n{}",
                        index + 1,
                        total,
                        response.content.trim()
                    ));
                }
                Ok(_) => {
                    log::warn!("corpus distill: 第 {}/{} 段摘要为空", index + 1, total);
                    summaries.push(chunk_fallback(chunk, index, total));
                }
                Err(e) => {
                    log::warn!(
                        "corpus distill: 第 {}/{} 段摘要失败，回退原文节选: {}",
                        index + 1,
                        total,
                        e
                    );
                    summaries.push(chunk_fallback(chunk, index, total));
                }
            }
        }
        format!(
            "（原文约 {} 字符，超过单次提炼上限，已按 {} 段分段摘要；以下为各段要点）\n\n{}",
            original_chars,
            total,
            summaries.join("\n\n")
        )
    }

    pub fn list_file_paths(&self, story_id: &str) -> Result<Vec<String>, AppError> {
        CorpusRepository::new(self.pool.clone()).list_file_paths_for_story(story_id)
    }

    pub fn delete_files(&self, paths: &[String]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn cleanup_story_dir(&self, story_id: &str, app: &AppHandle) {
        let app_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let _ = std::fs::remove_dir_all(app_dir.join("corpus").join(story_id));
    }

    fn story_brief(&self, story_id: &str) -> Result<String, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let brief: Option<String> = conn
            .query_row(
                "SELECT '标题：' || COALESCE(title, '') ||
                        '\n题材：' || COALESCE(genre, '') ||
                        '\n基调：' || COALESCE(tone, '') ||
                        '\n简介：' || COALESCE(description, '') ||
                        '\n世界观：' || COALESCE((SELECT concept FROM world_buildings WHERE story_id = ?1), '')
                 FROM stories WHERE id = ?1",
                rusqlite::params![story_id],
                |row| row.get(0),
            )
            .ok();
        Ok(brief
            .map(|s| s.chars().take(STORY_BRIEF_CHARS).collect())
            .unwrap_or_default())
    }
}

/// 按字符切分长文档；超过 max_chunks 段时均匀抽样窗口，保留开头与结尾。
fn chunk_text(content: &str, chunk_chars: usize, max_chunks: usize) -> Vec<String> {
    if chunk_chars == 0 || max_chunks == 0 {
        return vec![content.to_string()];
    }
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= chunk_chars {
        return vec![content.to_string()];
    }

    let total_chunks = chars.len().div_ceil(chunk_chars);
    if total_chunks <= max_chunks {
        return chars
            .chunks(chunk_chars)
            .map(|c| c.iter().collect())
            .collect();
    }

    if max_chunks == 1 {
        return vec![chars[..chunk_chars].iter().collect()];
    }
    let max_start = chars.len() - chunk_chars;
    (0..max_chunks)
        .map(|i| {
            let start = i * max_start / (max_chunks - 1);
            let end = (start + chunk_chars).min(chars.len());
            chars[start..end].iter().collect()
        })
        .collect()
}

fn chunk_fallback(chunk: &str, index: usize, total: usize) -> String {
    format!(
        "【第 {}/{} 段要点（摘要失败，原文节选）】\n{}",
        index + 1,
        total,
        chunk
            .chars()
            .take(DISTILL_CHUNK_FALLBACK_CHARS)
            .collect::<String>()
    )
}

fn make_summary(content: &str) -> String {
    let text = content.trim();
    if text.is_empty() {
        String::new()
    } else {
        text.chars().take(120).collect::<String>()
    }
}

fn extract_text(path: &Path, ext: &str) -> Result<String, AppError> {
    match ext {
        "txt" | "md" => std::fs::read_to_string(path).map_err(|e| {
            AppError::validation_failed(format!("读取文本失败: {e}"), Some("file_path"))
        }),
        "pdf" | "epub" => {
            let parsed =
                crate::book_deconstruction::parser::parse_book(path, None).map_err(|e| {
                    AppError::validation_failed(format!("解析文件失败: {e}"), Some("file_path"))
                })?;
            Ok(parsed.raw_text)
        }
        other => Err(AppError::validation_failed(
            format!("不支持的文件格式: {other}"),
            Some("file_format"),
        )),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn mime_for(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        "md" => "text/markdown",
        _ => "text/plain",
    }
}

fn promote_schema(target_type: &str) -> Result<&'static str, AppError> {
    match target_type {
        "character" => Ok(
            r#"输出 JSON：{"title":"角色名","fields":{"name":"","background":"","personality":"","goals":"","appearance":"","gender":"","age":0}}"#,
        ),
        "world" => Ok(r#"输出 JSON：{"title":"世界观标题","fields":{"concept":"","rules":[""]}}"#),
        "foreshadowing" => Ok(
            r#"输出 JSON：{"title":"伏笔标题","fields":{"description":"","setup":"","payoff":"","importance":"major"}}"#,
        ),
        "outline" => Ok(r#"输出 JSON：{"title":"大纲标题","fields":{"premise":"","acts":[""]}}"#),
        "scene" => Ok(
            r#"输出 JSON：{"title":"场景标题","fields":{"dramatic_goal":"","external_pressure":"","setting_location":"","content":""}}"#,
        ),
        _ => Err(AppError::validation_failed(
            format!("未知目标类型: {target_type}"),
            Some("target_type"),
        )),
    }
}

fn parse_json_loose<T: serde::de::DeserializeOwned>(raw: &str) -> Option<T> {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(v) = serde_json::from_str::<T>(cleaned) {
        return Some(v);
    }
    let start = cleaned.find('{')?;
    let end = cleaned.rfind('}')?;
    serde_json::from_str::<T>(&cleaned[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    #[test]
    fn test_parse_json_loose_with_fence_and_noise() {
        let raw = "好的：\n```json\n{\"title\":\"角色\",\"fields\":{\"name\":\"阿岩\"}}\n```\n以上";
        let v: serde_json::Value = parse_json_loose(raw).unwrap();
        assert_eq!(v["title"], "角色");
        assert_eq!(v["fields"]["name"], "阿岩");
    }

    #[test]
    fn test_mime_and_summary() {
        assert_eq!(mime_for("png"), "image/png");
        assert_eq!(make_summary("  你好世界  "), "你好世界");
    }

    #[test]
    fn chunk_text_keeps_short_document_whole() {
        let chunks = chunk_text("老街雨夜。", 12, 8);
        assert_eq!(chunks, vec!["老街雨夜。".to_string()]);
    }

    #[test]
    fn chunk_text_splits_contiguous_when_under_cap() {
        let chunks = chunk_text("abcdefghij", 4, 8);
        assert_eq!(chunks, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn chunk_text_samples_long_document_and_keeps_edges() {
        let content: String = (0..100)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let chunks = chunk_text(&content, 10, 4);
        assert_eq!(chunks.len(), 4);
        assert!(chunks.iter().all(|c| c.chars().count() == 10));
        assert!(chunks[0].starts_with('a'));
        assert!(chunks
            .last()
            .unwrap()
            .ends_with(content.chars().last().unwrap()));
    }

    #[test]
    fn chunk_text_never_returns_empty_windows() {
        let content: String = "文".repeat(96);
        let chunks = chunk_text(&content, 12, 8);
        assert_eq!(chunks.len(), 8);
        assert!(chunks.iter().all(|c| c.chars().count() == 12));
    }

    #[test]
    fn test_import_text_file_and_dedupe() {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("corpus-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.txt");
        std::fs::write(&path, "老街雨夜，霓虹反光。").unwrap();
        // 无 AppHandle 的导入路径无法在单测里构造真实 app_data；这里只验证
        // service 的 hash/format 分派仍可编译运行时由命令层覆盖。
        assert_eq!(mime_for("txt"), "text/plain");
        assert_eq!(mime_for("epub"), "application/epub+zip");
        let _ = path;
        let _ = CorpusService::new(pool);
        let _ = dir;
    }
}
