//! Knowledge Distiller Agent - 知识蒸馏师
//!
//! 将知识图谱中的实体与关系蒸馏为高层故事摘要和世界设定
//! 用于快速回顾故事世界观、人物关系与核心情节

use async_trait::async_trait;

use super::{Agent, AgentContext, AgentResult};
use crate::llm::service::LlmService;

pub struct KnowledgeDistillerAgent {
    llm_service: LlmService,
}

impl KnowledgeDistillerAgent {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }
}

#[async_trait]
impl Agent for KnowledgeDistillerAgent {
    fn name(&self) -> &str {
        "知识蒸馏师"
    }

    fn description(&self) -> &str {
        "将知识图谱蒸馏为高层故事摘要，提炼世界观、人物关系与核心情节"
    }

    async fn execute(
        &self,
        context: &AgentContext,
        input: &str,
    ) -> Result<AgentResult, Box<dyn std::error::Error>> {
        let prompt = format!(
            r#"你是一位专业的文学知识蒸馏师。请根据以下小说知识图谱，提炼出高层摘要。

【作品信息】
标题: {}
题材: {}
文风: {}
节奏: {}

【知识图谱】
{}

【蒸馏要求】
1. 世界观概述：提炼故事的宏观设定、核心规则、时代背景
2. 主要势力：总结故事中的重要组织、阵营、群体及其关系
3. 人物关系网：梳理核心角色之间的关系、立场、冲突
4. 核心情节线：提炼当前已展开的主要悬念、伏笔、目标
5. 输出条理清晰，使用Markdown格式，总长度控制在800字以内

请直接输出蒸馏后的摘要。"#,
            context.story.story_title,
            context.story.genre,
            context.story.tone,
            context.story.pacing,
            input
        );

        let response = self
            .llm_service
            .generate(prompt, Some(2048), Some(0.4))
            .await?;

        Ok(AgentResult::with_score(response.content, 0.9))
    }
}
