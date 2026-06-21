#![allow(dead_code)]
//! 记忆系统模块
//!
//! 基于llm_wiki方法论的记忆系统实现：
//! - 两步思维链Ingest流程
//! - 知识图谱（带关系强度）
//! - 四阶段查询检索管线
//! - 多助手独立会话

use crate::{
    agents::{
        AgentContext, AgentMemoryContext, ChapterSummary, CharacterInfo, NarrativeContext,
        StoryContext, StyleContext, WorldContext,
    },
    db::{Chapter, Character, Story},
};

pub mod health_daemon;
pub mod hybrid_search;
pub mod ingest;
pub mod multi_agent;
pub mod orchestrator;
pub mod query;
pub mod retention;
pub mod tokenizer;
pub mod writer;

pub use orchestrator::MemoryOrchestrator;
pub use tokenizer::CJKTokenizer;

pub use crate::domain::memory_pack::{
    MemoryEntry, MemoryItemDto, MemoryPack, MemoryStats, MemoryWarning,
};

/// 短期记忆管理器 - 维护 Agent 执行所需的上下文
pub struct ShortTermMemory {
    max_chapters: usize,
    max_characters: usize,
    max_events: usize,
}

impl ShortTermMemory {
    pub fn new() -> Self {
        Self {
            max_chapters: 5,
            max_characters: 10,
            max_events: 20,
        }
    }

    /// 从数据库记录构建 AgentContext
    pub fn build_context(
        &self,
        pool: &crate::db::DbPool,
        story: &Story,
        chapters: &[Chapter],
        characters: &[Character],
        target_chapter_number: u32,
        _outline: &str,
    ) -> AgentContext {
        // 构建章节摘要（最近 N 章）
        let mut previous_chapters: Vec<ChapterSummary> = chapters
            .iter()
            .filter(|c| c.chapter_number < target_chapter_number as i32)
            .map(|c| ChapterSummary {
                number: c.chapter_number as u32,
                title: c.title.clone().unwrap_or_default(),
                summary: self.summarize_chapter(c),
            })
            .collect();

        // 只保留最近的 N 章
        if previous_chapters.len() > self.max_chapters {
            previous_chapters = previous_chapters
                .into_iter()
                .rev()
                .take(self.max_chapters)
                .rev()
                .collect();
        }

        // 构建角色信息
        let character_infos: Vec<CharacterInfo> = characters
            .iter()
            .take(self.max_characters)
            .map(|c| CharacterInfo {
                name: c.name.clone(),
                personality: c.personality.clone().unwrap_or_default(),
                role: c.goals.clone().unwrap_or_else(|| "角色".to_string()),
                appearance: c.appearance.clone(),
                gender: c.gender.clone(),
                age: c.age,
            })
            .collect();

        // 组装 MemoryPack
        let memory_pack = {
            let orchestrator = crate::memory::orchestrator::MemoryOrchestrator::new(pool.clone());
            match orchestrator.build_memory_pack(
                &story.id,
                target_chapter_number as i32,
                "write",
                None,
            ) {
                Ok(mut pack) => {
                    for chapter in &previous_chapters {
                        pack.working_memory.push(MemoryEntry {
                            layer: "working".to_string(),
                            source: "previous_chapter".to_string(),
                            chapter: chapter.number as i32,
                            content: serde_json::json!({
                                "title": chapter.title,
                                "summary": chapter.summary
                            }),
                        });
                    }
                    Some(pack)
                }
                Err(e) => {
                    log::warn!(
                        "[ShortTermMemory] MemoryPack build failed: {}, continuing without",
                        e
                    );
                    None
                }
            }
        };

        AgentContext {
            story: StoryContext {
                story_id: story.id.clone(),
                story_title: story.title.clone(),
                genre: story.genre.clone().unwrap_or_else(|| "general".to_string()),
                tone: story.tone.clone().unwrap_or_else(|| "neutral".to_string()),
                pacing: story.pacing.clone().unwrap_or_else(|| "medium".to_string()),
                ..Default::default()
            },
            narrative: NarrativeContext {
                chapter_number: target_chapter_number,
                previous_chapters,
                characters: character_infos,
                current_content: None,
                selected_text: None,
                narrative_structure: None,
                active_threads: vec![],
                narrative_event_history: None,
                outline_context: None,
            },
            style: StyleContext {
                style_dna_id: None,
                style_blend: None,
                style_fingerprint: None,
                ..Default::default()
            },
            world: WorldContext {
                world_rules: None,
                scene_structure: None,
                methodology_id: None,
                methodology_step: None,
            },
            memory: AgentMemoryContext {
                memory_pack,
                memory: None,
            },
            runtime_contract: None,
        }
    }

    /// 生成章节摘要（首尾提取 + 关键词密度启发式摘要）
    fn summarize_chapter(&self, chapter: &Chapter) -> String {
        let content = chapter.content.as_ref().map(|s| s.as_str()).unwrap_or("");
        if content.is_empty() {
            return "No content".to_string();
        }
        if content.len() <= 300 {
            return content.to_string();
        }

        let paragraphs: Vec<&str> = content
            .split('\n')
            .filter(|s| !s.trim().is_empty())
            .collect();
        let first_para = paragraphs.first().unwrap_or(&"").trim();
        let last_para = paragraphs.last().unwrap_or(&"").trim();

        // 关键词密度提取：找出出现频率最高的 2-3 个词
        let mut word_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for word in content.split_whitespace() {
            let w = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if w.len() > 1 && !w.starts_with("的") && !w.starts_with("了") && !w.starts_with("是")
            {
                *word_counts.entry(w).or_insert(0) += 1;
            }
        }
        let mut top_words: Vec<(String, usize)> = word_counts.into_iter().collect();
        top_words.sort_by(|a, b| b.1.cmp(&a.1));
        let keywords: Vec<String> = top_words.into_iter().take(3).map(|(w, _)| w).collect();

        format!(
            "{}\n...（中间省略 {} 字，关键词: {}）...\n{}",
            first_para.chars().take(80).collect::<String>(),
            content.len(),
            keywords.join(", "),
            last_para.chars().take(80).collect::<String>()
        )
    }

    /// 提取关键事件（简化版，基于关键词）
    fn extract_key_events(&self, chapter: &Chapter) -> Vec<String> {
        let mut events = Vec::new();

        // 如果有大纲，使用大纲
        if let Some(outline) = &chapter.outline {
            events.push(format!("Chapter {}: {}", chapter.chapter_number, outline));
        }

        events
    }

    /// 推断角色当前状态
    fn infer_character_state(&self, character: &Character, _chapters: &[Chapter]) -> String {
        // 简化版：返回目标或最新动态特征
        character.goals.clone().unwrap_or_else(|| {
            if let Some(first_trait) = character.dynamic_traits.first() {
                format!(
                    "{} (confidence: {:.0}%)",
                    first_trait.trait_name,
                    first_trait.confidence * 100.0
                )
            } else {
                "Active".to_string()
            }
        })
    }
}

impl Default for ShortTermMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// 记忆系统主结构
pub struct MemorySystem {
    pub tokenizer: CJKTokenizer,
    pub short_term: ShortTermMemory,
}

impl MemorySystem {
    pub fn new() -> Self {
        Self {
            tokenizer: CJKTokenizer::new(),
            short_term: ShortTermMemory::new(),
        }
    }
}

impl Default for MemorySystem {
    fn default() -> Self {
        Self::new()
    }
}
