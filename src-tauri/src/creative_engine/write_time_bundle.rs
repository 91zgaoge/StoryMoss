//! WriteTimeBundle - 时间线 1（写作时刻）的最小可行约束包
//!
//! 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md 模块 8
//!
//! Phase 0 实证结论（2026-06-14，qwen3.6-35b）：
//! - 最小约束 vs 全量资产平均质量差距仅 7.9%（< 30% 阈值），架构成立。
//! - S1 玄幻：最小约束反而反超全量（A=110 vs B=99），因为全量 prompt 太长导致
//!   模型忽略了世界观红线。教训：红线必须最前最突出。
//! - S3 都市：全量大胜最小约束（B=125 vs A=99，差 26
//!   分），因为都市题材吃风格细节。 教训：风格片段需按题材自适应纳入。
//!
//! 因此本模块实现两条改进：
//! 1. 红线突出注入：to_prompt() 输出时红线在最前、加粗强调。
//! 2. 题材自适应：按 stories.genre 决定是否纳入风格片段。

use crate::db::{
    Character, CharacterRepository, DbPool, GenreProfileRepository, SceneRepository,
    StoryContractRepository,
};

// ==================== 数据结构 ====================

/// 写作时刻的最小约束包。
///
/// 只含"一次写对基本盘"必需的资产。审计用资产（Inspector/伏笔/记忆比对）
/// 不在此处——它们在时间线 2（AuditExecutor）异步加载。
pub struct WriteTimeBundle {
    /// 合同红线：MASTER_SETTING 核心世界观约束
    pub contract_redlines: Option<String>,
    /// 当前章节出场角色核心（姓名 + 当前状态）
    pub core_characters: Vec<CoreCharacter>,
    /// 当前 scene 大纲（dramatic_goal + conflict_type + setting）
    pub scene_outline: Option<SceneOutline>,
    /// GenreProfile 反模式清单
    pub genre_antipatterns: Vec<String>,
    /// 风格 DNA 片段（题材自适应，部分题材为 None）
    pub style_slice: Option<String>,
    /// 故事基础元信息
    pub story_meta: StoryMeta,
    /// 题材分类（决定 style_slice 是否纳入）
    pub genre_category: GenreCategory,
}

pub struct CoreCharacter {
    pub name: String,
    pub identity: Option<String>,
    pub physical_state: Option<String>,
    pub mental_state: Option<String>,
    pub location: Option<String>,
    pub personality: Option<String>,
}

pub struct SceneOutline {
    pub dramatic_goal: Option<String>,
    pub conflict_type: Option<String>,
    pub external_pressure: Option<String>,
    pub setting_location: Option<String>,
}

pub struct StoryMeta {
    pub title: String,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub description: Option<String>,
}

/// 题材分类——决定风格片段是否纳入（Phase 0 实证）。
#[derive(Debug, Clone, PartialEq)]
pub enum GenreCategory {
    /// 都市/情感/现实主义：风格细节是质量关键，纳入轻量风格片段
    RealismEmotional,
    /// 玄幻/仙侠/科幻：红线守严 > 风格约束，不纳入风格片段
    Speculative,
    /// 悬疑/推理：逻辑链是关键
    Mystery,
    /// 未知/默认：保守策略，不纳入
    Unknown,
}

impl GenreCategory {
    /// 是否应纳入轻量风格片段（Phase 0 实证）。
    pub fn include_style_slice(&self) -> bool {
        matches!(
            self,
            GenreCategory::RealismEmotional | GenreCategory::Mystery
        )
    }

    /// 根据 genre 字符串推断题材分类。
    /// 匹配逻辑宽松（包含关键词即命中），未命中归 Unknown。
    pub fn from_genre(genre: Option<&str>) -> Self {
        let g = match genre {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => return GenreCategory::Unknown,
        };
        let g_lower = g.to_lowercase();
        // 现实/情感类
        let realism_keywords = [
            "都市", "现实", "情感", "言情", "青春", "校园", "职场", "家庭", "年代", "生活", "治愈",
            "日常", "urban", "realism", "romance",
        ];
        if realism_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::RealismEmotional;
        }
        // 悬疑/推理类
        let mystery_keywords = [
            "悬疑",
            "推理",
            "侦探",
            "犯罪",
            "惊悚",
            "mystery",
            "thriller",
            "detective",
        ];
        if mystery_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::Mystery;
        }
        // 架空/幻想类
        let speculative_keywords = [
            "玄幻", "仙侠", "科幻", "奇幻", "修真", "末世", "网游", "灵异", "fantasy", "scifi",
            "sci-fi", "xianxia",
        ];
        if speculative_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::Speculative;
        }
        GenreCategory::Unknown
    }
}

// ==================== 加载 ====================

impl WriteTimeBundle {
    /// 从 DB 加载最小约束包。全部走 spawn_blocking（由调用方包裹）。
    ///
    /// `style_slice_override` 允许调用方传入预生成的风格片段（来自 StyleDna）。
    /// 若为 None，则按 genre_category.include_style_slice() 决定是否留空。
    pub fn load_sync(
        pool: &DbPool,
        story_id: &str,
        chapter_number: i32,
        style_slice_override: Option<String>,
    ) -> Result<Self, String> {
        // 1. 故事元信息
        let story_repo = crate::db::StoryRepository::new(pool.clone());
        let story = story_repo
            .get_by_id(story_id)
            .map_err(|e| format!("查询故事失败: {}", e))?
            .ok_or_else(|| format!("故事 {} 不存在", story_id))?;

        let genre_category = GenreCategory::from_genre(story.genre.as_deref());

        let story_meta = StoryMeta {
            title: story.title.clone(),
            genre: story.genre.clone(),
            tone: story.tone.clone(),
            pacing: story.pacing.clone(),
            description: story.description.clone(),
        };

        // 2. 合同红线（MASTER_SETTING）
        let contract_repo = StoryContractRepository::new(pool.clone());
        let contract_redlines = match contract_repo.get_by_story(story_id) {
            Ok(contracts) => {
                let master = contracts
                    .iter()
                    .find(|c| c.contract_type == "MASTER_SETTING");
                master.map(|c| c.contract_json.clone())
            }
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询合同失败: {}", e);
                None
            }
        };

        // 3. 角色核心
        let char_repo = CharacterRepository::new(pool.clone());
        let core_characters: Vec<CoreCharacter> = match char_repo.get_by_story(story_id) {
            Ok(chars) => chars
                .iter()
                .map(|c: &Character| CoreCharacter {
                    name: c.name.clone(),
                    identity: c.background.clone(),
                    physical_state: c.cs_physical_state.clone(),
                    mental_state: c.cs_mental_state.clone(),
                    location: c.cs_location.clone(),
                    personality: c.personality.clone(),
                })
                .collect(),
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询角色失败: {}", e);
                vec![]
            }
        };

        // 4. 场景大纲
        let scene_repo = SceneRepository::new(pool.clone());
        let scene_outline = match scene_repo.get_by_story(story_id) {
            Ok(scenes) => {
                let scene = scenes.iter().find(|s| s.sequence_number == chapter_number);
                scene.map(|s| SceneOutline {
                    dramatic_goal: s.dramatic_goal.clone(),
                    conflict_type: s.conflict_type.as_ref().map(|c| format!("{:?}", c)),
                    external_pressure: s.external_pressure.clone(),
                    setting_location: s.setting_location.clone(),
                })
            }
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询场景失败: {}", e);
                None
            }
        };

        // 5. GenreProfile 反模式清单
        let genre_repo = GenreProfileRepository::new(pool.clone());
        let genre_antipatterns = match &story.genre {
            Some(genre_name) => match genre_repo.get_by_name(genre_name) {
                Ok(Some(profile)) => parse_antipatterns(&profile.anti_patterns_json),
                _ => vec![],
            },
            None => vec![],
        };

        // 6. 风格片段（题材自适应）
        let style_slice = if genre_category.include_style_slice() {
            style_slice_override
        } else {
            None
        };

        Ok(WriteTimeBundle {
            contract_redlines,
            core_characters,
            scene_outline,
            genre_antipatterns,
            style_slice,
            story_meta,
            genre_category,
        })
    }

    /// 将 bundle 序列化为 prompt 注入字符串。
    ///
    /// 注入顺序（Phase 0 实证：红线最前最突出）：
    /// 1. 世界观红线（加粗强调「绝不可违背」）
    /// 2. 角色当前状态（直接影响行为合理性）
    /// 3. 场景大纲
    /// 4. GenreProfile 反模式
    /// 5. 风格片段（若有）
    pub fn to_prompt(&self) -> String {
        let mut sections: Vec<String> = vec![];

        // ① 世界观红线——最前、最突出（Phase 0 S1 实证：资产多 ≠ 幻觉少，红线必须醒目）
        if let Some(ref redlines) = self.contract_redlines {
            // 尝试从 contract_json 提取核心约束文本；若解析失败，原文兜底
            let redline_text = extract_redline_text(redlines);
            sections.push(format!(
                "【⚠️ 世界观红线（绝不可违背，违反即判定为严重错误）】\n{}",
                redline_text
            ));
        }

        // ② 角色核心 + 当前状态
        if !self.core_characters.is_empty() {
            let char_lines: Vec<String> = self
                .core_characters
                .iter()
                .map(|c| {
                    let mut parts = vec![format!("姓名：{}", c.name)];
                    if let Some(ref id) = c.identity {
                        parts.push(format!("身份：{}", id));
                    }
                    // 当前状态优先（Phase 0 memory 维度是最大波动源）
                    let mut state_parts = vec![];
                    if let Some(ref s) = c.physical_state {
                        state_parts.push(format!("身体：{}", s));
                    }
                    if let Some(ref s) = c.mental_state {
                        state_parts.push(format!("精神：{}", s));
                    }
                    if let Some(ref s) = c.location {
                        state_parts.push(format!("位置：{}", s));
                    }
                    if !state_parts.is_empty() {
                        parts.push(format!("当前状态：{}", state_parts.join("，")));
                    }
                    if let Some(ref p) = c.personality {
                        parts.push(format!("性格：{}", p));
                    }
                    format!("- {}", parts.join(" | "))
                })
                .collect();
            sections.push(format!(
                "【登场角色（必须严格遵循其当前状态）】\n{}",
                char_lines.join("\n")
            ));
        }

        // ③ 场景大纲
        if let Some(ref outline) = self.scene_outline {
            let mut outline_parts = vec![];
            if let Some(ref g) = outline.dramatic_goal {
                outline_parts.push(format!("戏剧目标：{}", g));
            }
            if let Some(ref c) = outline.conflict_type {
                outline_parts.push(format!("冲突类型：{}", c));
            }
            if let Some(ref p) = outline.external_pressure {
                outline_parts.push(format!("外部压迫：{}", p));
            }
            if let Some(ref s) = outline.setting_location {
                outline_parts.push(format!("场景地点：{}", s));
            }
            if !outline_parts.is_empty() {
                sections.push(format!("【本场景任务】\n{}", outline_parts.join("\n")));
            }
        }

        // ④ 反模式清单
        if !self.genre_antipatterns.is_empty() {
            let anti_lines: Vec<String> = self
                .genre_antipatterns
                .iter()
                .map(|a| format!("  - {}", a))
                .collect();
            sections.push(format!("【必须避免的反模式】\n{}", anti_lines.join("\n")));
        }

        // ⑤ 风格片段（题材自适应，仅 RealismEmotional/Mystery 纳入）
        if let Some(ref style) = self.style_slice {
            sections.push(format!("【风格指引】\n{}", style));
        }

        sections.join("\n\n")
    }
}

// ==================== 辅助函数 ====================

/// 解析 anti_patterns_json（可能是 JSON 数组或换行分隔文本）。
fn parse_antipatterns(json_str: &Option<String>) -> Vec<String> {
    let s = match json_str {
        Some(s) if !s.trim().is_empty() => s,
        _ => return vec![],
    };
    // 尝试 JSON 数组
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(s) {
        return arr;
    }
    // 尝试 JSON 数组（元素为对象，取 text 字段）
    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
        return arr
            .iter()
            .filter_map(|v| {
                v.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| v.as_str().map(|s| s.to_string()))
            })
            .collect();
    }
    // 兜底：按换行分割
    s.lines()
        .map(|l| l.trim().trim_start_matches('-').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// 从 contract_json 提取核心红线文本。
/// 若能解析为 JSON 且含 world_rules/redlines
/// 字段，提取之；否则原文兜底（截断）。
fn extract_redline_text(contract_json: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(contract_json) {
        // 尝试常见字段名
        for key in &[
            "redlines",
            "world_rules",
            "core_rules",
            "world_setting",
            "description",
        ] {
            if let Some(val) = v.get(key) {
                if let Some(s) = val.as_str() {
                    return truncate(s, 800);
                }
                if let Ok(s) = serde_json::to_string(val) {
                    return truncate(&s, 800);
                }
            }
        }
        // 兜底：整个 JSON 的文本内容
        if let Some(s) = v.as_str() {
            return truncate(s, 800);
        }
    }
    // 非 JSON：原文截断
    truncate(contract_json, 800)
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!(
            "{}...（已截断）",
            chars.iter().take(max_chars).collect::<String>()
        )
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genre_category_realism_detection() {
        assert_eq!(
            GenreCategory::from_genre(Some("都市言情")),
            GenreCategory::RealismEmotional
        );
        assert_eq!(
            GenreCategory::from_genre(Some("青春校园")),
            GenreCategory::RealismEmotional
        );
        assert_eq!(
            GenreCategory::from_genre(Some("Urban Romance")),
            GenreCategory::RealismEmotional
        );
    }

    #[test]
    fn genre_category_speculative_detection() {
        assert_eq!(
            GenreCategory::from_genre(Some("东方玄幻")),
            GenreCategory::Speculative
        );
        assert_eq!(
            GenreCategory::from_genre(Some("硬科幻")),
            GenreCategory::Speculative
        );
        assert_eq!(
            GenreCategory::from_genre(Some("Sci-Fi")),
            GenreCategory::Speculative
        );
    }

    #[test]
    fn genre_category_mystery_detection() {
        assert_eq!(
            GenreCategory::from_genre(Some("悬疑推理")),
            GenreCategory::Mystery
        );
        assert_eq!(
            GenreCategory::from_genre(Some("侦探小说")),
            GenreCategory::Mystery
        );
    }

    #[test]
    fn genre_category_unknown_for_empty_or_unmatched() {
        assert_eq!(GenreCategory::from_genre(None), GenreCategory::Unknown);
        assert_eq!(GenreCategory::from_genre(Some("")), GenreCategory::Unknown);
        assert_eq!(
            GenreCategory::from_genre(Some("武侠")),
            GenreCategory::Unknown
        );
    }

    #[test]
    fn style_slice_only_for_realism_and_mystery() {
        assert!(GenreCategory::RealismEmotional.include_style_slice());
        assert!(GenreCategory::Mystery.include_style_slice());
        assert!(!GenreCategory::Speculative.include_style_slice());
        assert!(!GenreCategory::Unknown.include_style_slice());
    }

    #[test]
    fn parse_antipatterns_json_array() {
        let result = parse_antipatterns(&Some(
            r#"["主角突然觉醒血脉", "无铺垫神级法器"]"#.to_string(),
        ));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "主角突然觉醒血脉");
    }

    #[test]
    fn parse_antipatterns_newline_separated() {
        let result = parse_antipatterns(&Some("第一行反模式\n第二行反模式".to_string()));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_antipatterns_empty() {
        assert!(parse_antipatterns(&None).is_empty());
        assert!(parse_antipatterns(&Some("".to_string())).is_empty());
    }

    #[test]
    fn truncate_respects_char_boundary() {
        let long =
            "一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十";
        let t = truncate(long, 10);
        assert!(t.ends_with("...（已截断）"));
        // 截断后（不含后缀）应 <= 10 字符
        let body = t.trim_end_matches("...（已截断）");
        assert!(body.chars().count() <= 10);
    }

    #[test]
    fn extract_redline_from_json_field() {
        let json = r#"{"redlines": "修炼者不可凭空变出实物"}"#;
        let text = extract_redline_text(json);
        assert!(text.contains("修炼者不可凭空变出实物"));
    }

    #[test]
    fn extract_redline_fallback_to_raw() {
        let raw = "这不是JSON只是一段纯文本红线描述";
        let text = extract_redline_text(raw);
        assert!(text.contains("纯文本红线"));
    }

    #[test]
    fn to_prompt_redlines_appear_first() {
        let bundle = WriteTimeBundle {
            contract_redlines: Some(r#"{"redlines": "绝对红线内容"}"#.to_string()),
            core_characters: vec![CoreCharacter {
                name: "测试角色".to_string(),
                identity: None,
                physical_state: None,
                mental_state: None,
                location: None,
                personality: None,
            }],
            scene_outline: None,
            genre_antipatterns: vec!["某反模式".to_string()],
            style_slice: None,
            story_meta: StoryMeta {
                title: "测试".to_string(),
                genre: Some("玄幻".to_string()),
                tone: None,
                pacing: None,
                description: None,
            },
            genre_category: GenreCategory::Speculative,
        };
        let prompt = bundle.to_prompt();
        let redline_pos = prompt.find("绝对红线内容").unwrap_or(usize::MAX);
        let char_pos = prompt.find("测试角色").unwrap_or(usize::MAX);
        let anti_pos = prompt.find("某反模式").unwrap_or(usize::MAX);
        // 红线必须最前
        assert!(redline_pos < char_pos, "红线应在角色之前");
        assert!(redline_pos < anti_pos, "红线应在反模式之前");
        // Speculative 题材不应有风格片段
        assert!(!prompt.contains("风格指引"));
    }
}
