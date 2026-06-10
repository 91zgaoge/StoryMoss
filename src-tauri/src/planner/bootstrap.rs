//! Novel Bootstrap Workflow - 向后兼容类型定义
//!
//! 旧版 NovelBootstrapWorkflow 实现已迁移到 narrative/genesis.rs。
//! 本文件仅保留公共类型定义，供向后兼容的进度事件发射使用。

use serde::{Deserialize, Serialize};

/// 小说初始化会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapSession {
    pub id: String,
    pub status: BootstrapStatus,
    pub current_step: String,
    pub steps_completed: usize,
    pub total_steps: usize,
    pub story_id: Option<String>,
    pub error_message: Option<String>,
    /// 生成的小说正文开头内容（直接返回给前端展示）
    pub first_chapter_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapStatus {
    InProgress,
    Completed,
    Failed,
}

/// Bootstrap 进度事件（推送到前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapProgressEvent {
    pub session_id: String,
    pub step_name: String,
    pub step_number: usize,
    pub total_steps: usize,
    pub message: String,
    pub status: String, // running | completed | failed
}

/// 故事概念（供测试和向后兼容使用）
#[derive(Debug, Clone, Deserialize)]
pub struct StoryConcept {
    pub title: String,
    pub description: String,
    pub genre: String,
    pub tone: String,
    pub pacing: String,
    #[serde(default)]
    pub themes: Vec<String>,
    #[serde(default)]
    pub target_length: String,
}

/// 从文本中提取 JSON 对象字符串
pub fn extract_json(content: &str) -> Result<&str, String> {
    if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
        Ok(&content[start..=end])
    } else {
        Err("No JSON object found in response".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"title": "测试", "genre": "科幻"}"#;
        let result = extract_json(input).unwrap();
        assert_eq!(result, r#"{"title": "测试", "genre": "科幻"}"#);
    }

    #[test]
    fn test_extract_json_with_markdown() {
        let input = "这里有一些解释\n```json\n{\"title\": \"测试\"}\n```\n更多解释";
        let result = extract_json(input).unwrap();
        assert_eq!(result, r#"{"title": "测试"}"#);
    }

    #[test]
    fn test_extract_json_with_prefix_text() {
        let input = "好的，这是你的JSON:\n{\"name\": \"value\"}\n希望这有帮助";
        let result = extract_json(input).unwrap();
        assert_eq!(result, r#"{"name": "value"}"#);
    }

    #[test]
    fn test_extract_json_no_json() {
        let input = "这里没有JSON对象";
        assert!(extract_json(input).is_err());
    }

    #[test]
    fn test_extract_json_nested() {
        let input = r#"{"outer": {"inner": "value"}}"#;
        let result = extract_json(input).unwrap();
        assert_eq!(result, r#"{"outer": {"inner": "value"}}"#);
    }

    #[test]
    fn test_story_concept_deserialization() {
        let json = r#"{
            "title": "都市仙尊",
            "description": "一个现代都市中的修仙故事",
            "genre": "都市玄幻",
            "tone": "热血",
            "pacing": "快节奏",
            "themes": ["复仇", "成长"],
            "target_length": "长篇100万字"
        }"#;
        let concept: StoryConcept = serde_json::from_str(json).unwrap();
        assert_eq!(concept.title, "都市仙尊");
        assert_eq!(concept.genre, "都市玄幻");
        assert_eq!(concept.themes.len(), 2);
    }

    #[test]
    fn test_story_concept_deserialization_defaults() {
        let json = r#"{"title": "极简", "description": "测试", "genre": "测试", "tone": "轻松", "pacing": "慢热"}"#;
        let concept: StoryConcept = serde_json::from_str(json).unwrap();
        assert!(concept.themes.is_empty());
        assert!(concept.target_length.is_empty());
    }
}
