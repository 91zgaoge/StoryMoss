//! Memory Compressor Agent - 记忆压缩师
//!
//! 将长篇内容、实体档案、历史版本压缩为高层记忆摘要
//! 用于上下文窗口优化和长期记忆保留

use async_trait::async_trait;

use super::{Agent, AgentContext, AgentResult};
use crate::llm::service::LlmService;

pub struct MemoryCompressorAgent {
    llm_service: LlmService,
}

impl MemoryCompressorAgent {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }
}

#[async_trait]
impl Agent for MemoryCompressorAgent {
    fn name(&self) -> &str {
        "记忆压缩师"
    }

    fn description(&self) -> &str {
        "将详细内容压缩为高层摘要，保留关键信息的同时减少Token占用"
    }

    async fn execute(
        &self,
        context: &AgentContext,
        input: &str,
    ) -> Result<AgentResult, Box<dyn std::error::Error>> {
        let prompt = format!(
            r#"你是一位专业的文学记忆压缩师。请将以下小说相关内容压缩为简洁的高层摘要。

【作品信息】
标题: {}
题材: {}
文风: {}
节奏: {}

【待压缩内容】
{}

【压缩要求】
1. 保留核心情节、人物关系、关键伏笔
2. 删除细节描写、重复叙述、过渡段落
3. 输出长度控制在原文的 20%-30%
4. 使用第三人称客观叙述
5. 如果内容包含多个章节，按时间线组织

请直接输出压缩后的摘要，不要添加解释。"#,
            context.story.story_title,
            context.story.genre,
            context.story.tone,
            context.story.pacing,
            input
        );

        let response = self
            .llm_service
            .generate(prompt, Some(2048), Some(0.3))
            .await?;

        // 估算压缩率
        let original_len = input.chars().count();
        let compressed_len = response.content.chars().count();
        let compression_ratio = if original_len > 0 {
            compressed_len as f32 / original_len as f32
        } else {
            1.0
        };
        let score = (1.0 - compression_ratio).max(0.0).min(1.0);

        Ok(AgentResult::with_score(response.content, score))
    }
}

/// 批量压缩请求
#[derive(Debug, Clone)]
pub struct BatchCompressionRequest {
    pub items: Vec<CompressionItem>,
    pub target_ratio: f32, // 0.0 - 1.0
}

#[derive(Debug, Clone)]
pub struct CompressionItem {
    pub id: String,
    pub content_type: CompressionContentType,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum CompressionContentType {
    Chapter,
    EntityProfile,
    SceneHistory,
    Conversation,
}

impl MemoryCompressorAgent {
    /// 批量压缩内容
    pub async fn compress_batch(
        &self,
        _context: &AgentContext,
        request: &BatchCompressionRequest,
    ) -> Result<Vec<AgentResult>, Box<dyn std::error::Error>> {
        use futures::future::join_all;

        let futures = request.items.iter().map(|item| {
            let target_ratio = request.target_ratio;
            let input = format!(
                "[类型: {}]\n{}",
                match item.content_type {
                    CompressionContentType::Chapter => "章节内容",
                    CompressionContentType::EntityProfile => "实体档案",
                    CompressionContentType::SceneHistory => "场景历史",
                    CompressionContentType::Conversation => "对话记录",
                },
                item.content
            );

            let prompt = format!(
                r#"请将以下内容压缩至原长度的 {:.0}%，保留核心信息：

{}

直接输出压缩结果。"#,
                target_ratio * 100.0,
                input
            );

            async move {
                self.llm_service
                    .generate(prompt, Some(1024), Some(0.3))
                    .await
            }
        });

        let responses = join_all(futures).await;
        let results: Result<Vec<_>, _> = responses.into_iter().collect();
        let results = results?;

        Ok(results
            .into_iter()
            .map(|r| AgentResult::simple(r.content))
            .collect())
    }
}
