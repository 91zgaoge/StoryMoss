//! 自动化触发器
//!
//! 定义各种事件触发条件和规则

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 触发事件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerEvent {
    /// 故事创建
    StoryCreated { story_id: String },
    /// 章节创建
    ChapterCreated { story_id: String, chapter_id: String },
    /// 角色创建
    CharacterCreated { story_id: String, character_id: String },
    /// 章节内容更新
    ChapterContentUpdated { story_id: String, chapter_id: String, word_count: usize },
    /// 角色关系更新
    CharacterRelationshipUpdated { story_id: String, character_id: String },
    /// 故事设定更新
    StorySettingUpdated { story_id: String },
    /// 工作流完成
    WorkflowCompleted { workflow_id: String, instance_id: String },
    /// 任务完成
    TaskCompleted { task_id: String, task_type: String },
}

/// 触发条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerCondition {
    /// 总是触发
    Always,
    /// 字数达到阈值
    WordCountThreshold { min_words: usize },
    /// 角色数量达到阈值
    CharacterCountThreshold { min_count: usize },
    /// 章节数量达到阈值
    ChapterCountThreshold { min_count: usize },
    /// 时间间隔（秒）
    TimeInterval { seconds: u64 },
    /// 自定义条件表达式
    CustomExpression { expression: String },
}

/// 自动化触发器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationTrigger {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    /// 触发事件类型
    pub event_type: TriggerEvent,
    /// 触发条件
    pub conditions: Vec<TriggerCondition>,
    /// 处理器ID
    pub handler_id: String,
    /// 额外参数
    pub parameters: HashMap<String, serde_json::Value>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl AutomationTrigger {
    pub fn new(
        name: String,
        description: String,
        event_type: TriggerEvent,
        conditions: Vec<TriggerCondition>,
        handler_id: String,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            enabled: true,
            event_type,
            conditions,
            handler_id,
            parameters: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 检查事件是否匹配触发器
    pub fn matches_event(&self, event: &TriggerEvent) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查事件类型匹配
        std::mem::discriminant(&self.event_type) == std::mem::discriminant(event)
    }

    /// 评估触发条件
    pub async fn evaluate_conditions<T: TriggerContext>(
        &self,
        event: &TriggerEvent,
        context: &T,
    ) -> Result<bool, String> {
        for condition in &self.conditions {
            if !self.evaluate_single_condition(condition, event, context).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn evaluate_single_condition<T: TriggerContext>(
        &self,
        condition: &TriggerCondition,
        event: &TriggerEvent,
        context: &T,
    ) -> Result<bool, String> {
        match condition {
            TriggerCondition::Always => Ok(true),
            TriggerCondition::WordCountThreshold { min_words } => {
                match event {
                    TriggerEvent::ChapterContentUpdated { word_count, .. } => {
                        Ok(*word_count >= *min_words)
                    }
                    _ => {
                        // 从上下文获取字数
                        if let Some(story_id) = self.extract_story_id(event) {
                            let word_count = context.get_story_word_count(&story_id).await?;
                            Ok(word_count >= *min_words)
                        } else {
                            Ok(false)
                        }
                    }
                }
            }
            TriggerCondition::CharacterCountThreshold { min_count } => {
                if let Some(story_id) = self.extract_story_id(event) {
                    let char_count = context.get_character_count(&story_id).await?;
                    Ok(char_count >= *min_count)
                } else {
                    Ok(false)
                }
            }
            TriggerCondition::ChapterCountThreshold { min_count } => {
                if let Some(story_id) = self.extract_story_id(event) {
                    let chapter_count = context.get_chapter_count(&story_id).await?;
                    Ok(chapter_count >= *min_count)
                } else {
                    Ok(false)
                }
            }
            TriggerCondition::TimeInterval { seconds } => {
                // 检查上次触发时间
                let last_trigger = context.get_last_trigger_time(&self.id).await?;
                let now = chrono::Utc::now();
                if let Some(last) = last_trigger {
                    let elapsed = now.signed_duration_since(last).num_seconds() as u64;
                    Ok(elapsed >= *seconds)
                } else {
                    Ok(true) // 首次触发
                }
            }
            TriggerCondition::CustomExpression { expression } => {
                // 简单的表达式求值（可扩展）
                self.evaluate_expression(expression, event, context).await
            }
        }
    }

    fn extract_story_id(&self, event: &TriggerEvent) -> Option<String> {
        match event {
            TriggerEvent::StoryCreated { story_id } => Some(story_id.clone()),
            TriggerEvent::ChapterCreated { story_id, .. } => Some(story_id.clone()),
            TriggerEvent::CharacterCreated { story_id, .. } => Some(story_id.clone()),
            TriggerEvent::ChapterContentUpdated { story_id, .. } => Some(story_id.clone()),
            TriggerEvent::CharacterRelationshipUpdated { story_id, .. } => Some(story_id.clone()),
            TriggerEvent::StorySettingUpdated { story_id } => Some(story_id.clone()),
            _ => None,
        }
    }

    async fn evaluate_expression<T: TriggerContext>(
        &self,
        _expression: &str,
        _event: &TriggerEvent,
        _context: &T,
    ) -> Result<bool, String> {
        // TODO: 实现表达式求值器
        // 暂时返回 true
        Ok(true)
    }
}

/// 触发器上下文 - 提供数据查询接口
pub trait TriggerContext {
    async fn get_story_word_count(&self, story_id: &str) -> Result<usize, String>;
    async fn get_character_count(&self, story_id: &str) -> Result<usize, String>;
    async fn get_chapter_count(&self, story_id: &str) -> Result<usize, String>;
    async fn get_last_trigger_time(&self, trigger_id: &str) -> Result<Option<chrono::DateTime<chrono::Utc>>, String>;
}

/// 预定义触发器模板
pub struct TriggerTemplates;

impl TriggerTemplates {
    /// 故事创建后自动生成角色
    pub fn auto_generate_characters_on_story_creation() -> AutomationTrigger {
        AutomationTrigger::new(
            "自动生成角色".to_string(),
            "故事创建后自动生成初始角色".to_string(),
            TriggerEvent::StoryCreated { story_id: String::new() },
            vec![TriggerCondition::Always],
            "generate_characters".to_string(),
        )
    }

    /// 章节创建后自动分析情节
    pub fn auto_analyze_plot_on_chapter_creation() -> AutomationTrigger {
        AutomationTrigger::new(
            "自动分析情节".to_string(),
            "章节创建后自动分析情节发展".to_string(),
            TriggerEvent::ChapterCreated { story_id: String::new(), chapter_id: String::new() },
            vec![TriggerCondition::Always],
            "analyze_plot".to_string(),
        )
    }

    /// 内容更新后自动索引
    pub fn auto_index_on_content_update() -> AutomationTrigger {
        AutomationTrigger::new(
            "自动内容索引".to_string(),
            "内容更新后自动建立向量索引".to_string(),
            TriggerEvent::ChapterContentUpdated {
                story_id: String::new(),
                chapter_id: String::new(),
                word_count: 0
            },
            vec![TriggerCondition::WordCountThreshold { min_words: 100 }],
            "vector_index".to_string(),
        )
    }

    /// 定期备份
    pub fn periodic_backup() -> AutomationTrigger {
        AutomationTrigger::new(
            "定期备份".to_string(),
            "每小时自动备份故事数据".to_string(),
            TriggerEvent::TaskCompleted {
                task_id: String::new(),
                task_type: "backup_trigger".to_string()
            },
            vec![TriggerCondition::TimeInterval { seconds: 3600 }],
            "backup_story".to_string(),
        )
    }
}