//! Commentator Agent - 古典小说评论员
//!
//! 以金圣叹风格的古典评点家视角，对小说段落进行实时点评
//! 评点内容简洁犀利，富有文学洞见

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Agent, AgentContext, AgentResult};
use crate::llm::service::LlmService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParagraphCommentary {
    pub paragraph_index: usize,
    pub commentary: String,
    pub tone: CommentaryTone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommentaryTone {
    Insightful, // 洞见型：揭示写作技巧和深层含义
    Witty,      // 机智型：幽默讽刺，类似金圣叹
    Emotional,  // 情感型：共鸣人物情绪
    Critical,   // 批判型：指出不足和改进空间
}

pub struct CommentatorAgent {
    llm_service: LlmService,
}

impl CommentatorAgent {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }

    /// 对单个段落生成评点
    pub async fn comment_on_paragraph(
        &self,
        context: &AgentContext,
        paragraph_index: usize,
        paragraph_text: &str,
        previous_paragraphs: &[String],
    ) -> Result<ParagraphCommentary, Box<dyn std::error::Error>> {
        if paragraph_text.trim().len() < 20 {
            return Ok(ParagraphCommentary {
                paragraph_index,
                commentary: String::new(),
                tone: CommentaryTone::Insightful,
            });
        }

        let prev_text = if previous_paragraphs.len() >= 2 {
            previous_paragraphs[previous_paragraphs.len() - 2..].join("\n")
        } else if !previous_paragraphs.is_empty() {
            previous_paragraphs.join("\n")
        } else {
            "（首段）".to_string()
        };

        let prompt = format!(
            r#"你是一位中国古典小说评点家，风格类似金圣叹。请对以下小说段落进行简短点评。

【作品信息】
标题: {}
题材: {}

【前文】
{}

【当前段落】
{}

【点评要求】
1. 用古典文人评点的口吻，简洁有力，不超过60字
2. 可点评：文笔、结构、人物、伏笔、情感、节奏
3. 语气可带几分 witty（机锋），但不可刻薄伤人
4. 如果没有值得点评之处，回复空字符串
5. 直接输出点评内容，不要加引号或解释

点评："#,
            context.story.story_title, context.story.genre, prev_text, paragraph_text
        );

        let response = self
            .llm_service
            .generate(prompt, Some(128), Some(0.85))
            .await?;
        let commentary = response.content.trim().to_string();

        let tone = if commentary.contains("妙") || commentary.contains("绝") {
            CommentaryTone::Witty
        } else if commentary.contains("惜")
            || commentary.contains("可惜")
            || commentary.contains("欠")
        {
            CommentaryTone::Critical
        } else if commentary.contains("情")
            || commentary.contains("悲")
            || commentary.contains("叹")
        {
            CommentaryTone::Emotional
        } else {
            CommentaryTone::Insightful
        };

        Ok(ParagraphCommentary {
            paragraph_index,
            commentary,
            tone,
        })
    }

    /// 批量评点多个段落
    pub async fn comment_on_text(
        &self,
        context: &AgentContext,
        text: &str,
    ) -> Result<Vec<ParagraphCommentary>, Box<dyn std::error::Error>> {
        let paragraphs: Vec<String> = text
            .split('\n')
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .collect();

        let mut results = Vec::new();
        let mut prev = Vec::new();

        for (idx, para) in paragraphs.iter().enumerate() {
            match self.comment_on_paragraph(context, idx, para, &prev).await {
                Ok(c) if !c.commentary.is_empty() => {
                    results.push(c);
                }
                _ => {}
            }
            prev.push(para.clone());
        }

        Ok(results)
    }
}

#[async_trait]
impl Agent for CommentatorAgent {
    fn name(&self) -> &str {
        "古典评点家"
    }

    fn description(&self) -> &str {
        "以金圣叹风格对小说段落进行实时文学点评"
    }

    async fn execute(
        &self,
        context: &AgentContext,
        input: &str,
    ) -> Result<AgentResult, Box<dyn std::error::Error>> {
        let commentaries = self.comment_on_text(context, input).await?;
        let content = serde_json::to_string(&commentaries)?;
        Ok(AgentResult::simple(content))
    }
}
