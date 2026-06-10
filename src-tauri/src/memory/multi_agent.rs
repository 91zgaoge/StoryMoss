//! 多助手独立会话管理
//!
//! 世界观助手、人物助手、文风助手独立会话
//! 支持Wiki引用跟踪和保存到Wiki

use std::collections::HashMap;

use chrono::Local;
use serde::{Deserialize, Serialize};

use super::ingest::{IngestContent, IngestPipeline};
use crate::{db::models::AgentBotType, llm::LlmService};

/// 多助手会话管理器
pub struct MultiAgentSessionManager {
    sessions: HashMap<AgentBotType, AgentSession>,
    llm_service: LlmService,
    ingest_pipeline: IngestPipeline,
    story_id: String,
}

/// 助手会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub agent_type: AgentBotType,
    pub messages: Vec<Message>,
    pub used_wiki_pages: Vec<String>,
    pub created_at: chrono::DateTime<Local>,
    pub updated_at: chrono::DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<Local>,
    pub used_wiki_pages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "System"),
            MessageRole::User => write!(f, "User"),
            MessageRole::Assistant => write!(f, "Assistant"),
        }
    }
}

/// 助手响应
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub used_wiki_pages: Vec<String>,
    pub citations: Vec<String>,
}

impl MultiAgentSessionManager {
    pub fn new(story_id: String, llm_service: LlmService, ingest_pipeline: IngestPipeline) -> Self {
        let mut sessions = HashMap::new();

        // 初始化所有助手类型的会话
        for agent_type in [
            AgentBotType::WorldBuilding,
            AgentBotType::Character,
            AgentBotType::WritingStyle,
            AgentBotType::Scene,
            AgentBotType::Plot,
        ] {
            sessions.insert(
                agent_type,
                AgentSession {
                    agent_type,
                    messages: vec![],
                    used_wiki_pages: vec![],
                    created_at: Local::now(),
                    updated_at: Local::now(),
                },
            );
        }

        Self {
            sessions,
            llm_service,
            ingest_pipeline,
            story_id,
        }
    }

    /// 发送消息到特定助手
    pub async fn chat(
        &mut self,
        agent_type: AgentBotType,
        message: &str,
        query_context: Option<String>,
    ) -> Result<AgentResponse, Box<dyn std::error::Error>> {
        // 先获取所有需要的数据，避免借用冲突
        let system_prompt = self.get_system_prompt(&agent_type);

        let context = if let Some(ctx) = query_context {
            format!("相关背景知识：\n{}\n\n", ctx)
        } else {
            String::new()
        };

        // 获取对话历史
        let history = if let Some(session) = self.sessions.get(&agent_type) {
            session
                .messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        // 构建完整提示
        let full_prompt = format!(
            "{system_prompt}\n\n{history}\n\n{context}User: {message}\n\nAssistant:",
            system_prompt = system_prompt,
            history = history,
            context = context,
            message = message
        );

        // 调用LLM
        let response = self.llm_service.generate(full_prompt, None, None).await?;
        let response_content = response.content;

        // 提取Wiki引用
        let used_pages = self.extract_wiki_references(&response_content);

        // 现在获取可变引用来修改session
        let session = self
            .sessions
            .get_mut(&agent_type)
            .ok_or("Session not found")?;

        // 记录用户消息
        session.messages.push(Message {
            role: MessageRole::User,
            content: message.to_string(),
            timestamp: Local::now(),
            used_wiki_pages: vec![],
        });

        // 记录助手响应
        session.messages.push(Message {
            role: MessageRole::Assistant,
            content: response_content.clone(),
            timestamp: Local::now(),
            used_wiki_pages: used_pages.clone(),
        });

        // 更新使用的Wiki页面
        session.used_wiki_pages.extend(used_pages.clone());
        session.used_wiki_pages.sort();
        session.used_wiki_pages.dedup();
        session.updated_at = Local::now();

        // 生成引用标记
        let citations: Vec<String> = used_pages
            .iter()
            .enumerate()
            .map(|(i, page)| format!("[{}] {}", i + 1, page))
            .collect();

        Ok(AgentResponse {
            content: response_content,
            used_wiki_pages: used_pages,
            citations,
        })
    }

    /// 保存对话结果到Wiki
    pub async fn save_chat_to_wiki(
        &self,
        agent_type: AgentBotType,
        title: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.sessions.get(&agent_type).ok_or("Session not found")?;

        // 构建对话内容
        let conversation_text = session
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let content = format!(
            "# {}\n\n## 对话记录\n\n{}\n\n## 使用的参考资料\n\n{}",
            title,
            conversation_text,
            session
                .used_wiki_pages
                .iter()
                .map(|p| format!("- {}", p))
                .collect::<Vec<_>>()
                .join("\n")
        );

        // 使用Ingest流程处理对话内容
        let ingest_content = IngestContent {
            text: content,
            source: format!("chat:{:?}", agent_type),
            story_id: self.story_id.clone(),
            scene_id: None,
        };

        self.ingest_pipeline.ingest(&ingest_content).await?;

        Ok(())
    }

    /// 获取系统提示词
    fn get_system_prompt(&self, agent_type: &AgentBotType) -> String {
        match agent_type {
            AgentBotType::WorldBuilding => r#"你是世界观助手，专门帮助构建和完善小说的世界观设定。

你的职责：
1. 帮助设计和完善世界规则、历史背景、文化设定
2. 回答关于世界观的问题
3. 指出设定中的潜在冲突或不一致
4. 提供灵感建议

回答时请：
- 引用相关的Wiki页面
- 保持与已有设定的一致性
- 提供具体可行的建议"#
                .to_string(),
            AgentBotType::Character => r#"你是人物助手，专门帮助塑造角色形象和性格发展。

你的职责：
1. 帮助设计角色的性格、背景、动机
2. 分析角色间的关系和互动
3. 提供角色发展建议
4. 确保角色行为符合其性格设定

回答时请：
- 引用角色相关的Wiki页面
- 考虑角色的成长弧线
- 提供具体的对话或行为示例"#
                .to_string(),
            AgentBotType::WritingStyle => r#"你是文风助手，专门帮助优化写作风格和语言表达。

你的职责：
1. 提供文风改进建议
2. 帮助修改段落使其更符合设定风格
3. 分析文本的节奏、语气、用词
4. 提供具体的修改方案

回答时请：
- 引用文风相关的Wiki页面
- 给出修改前后的对比
- 解释修改的原因"#
                .to_string(),
            AgentBotType::Scene => r#"你是场景助手，专门帮助设计戏剧性的场景和情节发展。

你的职责：
1. 帮助设计场景的戏剧冲突
2. 提供场景布局、节奏控制建议
3. 分析场景的戏剧效果
4. 建议如何增强场景的紧张感或情感冲击力

回答时请：
- 引用场景相关的Wiki页面
- 关注戏剧目标、外部压迫、冲突类型
- 提供具体的场景设计建议"#
                .to_string(),
            AgentBotType::Plot => r#"你是情节助手，专门帮助规划和优化故事线。

你的职责：
1. 帮助设计情节转折和高潮
2. 分析故事结构的合理性
3. 提供伏笔和照应的设计建议
4. 确保情节推进符合逻辑

回答时请：
- 引用情节相关的Wiki页面
- 考虑前后文的连贯性
- 提供多种可能的发展方向"#
                .to_string(),
            _ => "你是一个专业的写作助手。".to_string(),
        }
    }

    /// 提取Wiki引用
    fn extract_wiki_references(&self, text: &str) -> Vec<String> {
        let mut references = vec![];

        // 简单的引用提取：查找 [[...]] 格式
        let mut chars = text.chars().peekable();
        let mut current = String::new();
        let mut in_brackets = false;

        while let Some(c) = chars.next() {
            if c == '[' && chars.peek() == Some(&'[') {
                chars.next(); // 跳过第二个[
                in_brackets = true;
                current.clear();
            } else if c == ']' && chars.peek() == Some(&']') {
                chars.next(); // 跳过第二个]
                in_brackets = false;
                if !current.is_empty() {
                    references.push(current.clone());
                }
            } else if in_brackets {
                current.push(c);
            }
        }

        references
    }

    /// 获取会话历史
    pub fn get_session_history(&self, agent_type: &AgentBotType) -> Option<Vec<Message>> {
        self.sessions.get(agent_type).map(|s| s.messages.clone())
    }

    /// 清空会话历史
    pub fn clear_session(&mut self, agent_type: AgentBotType) {
        if let Some(session) = self.sessions.get_mut(&agent_type) {
            session.messages.clear();
            session.used_wiki_pages.clear();
            session.updated_at = Local::now();
        }
    }

    /// 获取所有会话的统计信息
    pub fn get_stats(&self) -> HashMap<AgentBotType, SessionStats> {
        self.sessions
            .iter()
            .map(|(agent_type, session)| {
                let stats = SessionStats {
                    message_count: session.messages.len(),
                    wiki_references_count: session.used_wiki_pages.len(),
                    created_at: session.created_at,
                    updated_at: session.updated_at,
                };
                (*agent_type, stats)
            })
            .collect()
    }
}

/// 会话统计
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub message_count: usize,
    pub wiki_references_count: usize,
    pub created_at: chrono::DateTime<Local>,
    pub updated_at: chrono::DateTime<Local>,
}
