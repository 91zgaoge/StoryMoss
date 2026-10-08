//! Prompt assembly：Instruction + Context + Tools → (system, user)。
//! 哑拼接器。本模块不得依赖 `agency`。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    System,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Instruction,
    Context,
    Tools,
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: &'static str,
    pub kind: LayerKind,
    pub slot: Slot,
    pub body: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledPrompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssembleError {
    DuplicateId(&'static str),
    MissingRequired(&'static str),
}

impl std::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "prompt assembly: duplicate layer id `{id}`"),
            Self::MissingRequired(id) => {
                write!(f, "prompt assembly: required layer `{id}` is empty")
            }
        }
    }
}

impl std::error::Error for AssembleError {}

pub fn assemble(layers: &[Layer]) -> Result<AssembledPrompt, AssembleError> {
    let mut seen = std::collections::HashSet::new();
    let mut system_parts: Vec<&str> = Vec::new();
    let mut user_parts: Vec<&str> = Vec::new();

    for layer in layers {
        if !seen.insert(layer.id) {
            return Err(AssembleError::DuplicateId(layer.id));
        }
        let body = layer.body.trim();
        if body.is_empty() {
            if layer.required {
                return Err(AssembleError::MissingRequired(layer.id));
            }
            continue;
        }
        match layer.slot {
            Slot::System => system_parts.push(body),
            Slot::User => user_parts.push(body),
        }
    }

    Ok(AssembledPrompt {
        system: system_parts.join("\n\n"),
        user: user_parts.join("\n\n"),
    })
}

pub const GENESIS_FIRST_CHAPTER_SYSTEM: &str = "你是小说主创，只输出章节正文。人设、世界观与已埋伏笔以下方资产区为准，不得自相矛盾、不得发明与资产冲突的角色或设定。禁止重复：同一段落/句子不得出现两次，不得复述已有正文。开篇须有可见行动与未解问题。";

pub const GENESIS_PROSE_FALLBACK_SYSTEM: &str = "你是小说主创，只输出章节正文。人设、世界观与已埋伏笔以下方资产区为准，不得自相矛盾。禁止重复：同一段落/句子不得出现两次，不得复述已有正文。开篇须有可见行动与未解问题。";

pub const CONTINUE_BEAT_SYSTEM: &str = "你是小说主创。只输出本章正文，不要标题、分析或清单。\n\
1. 按节拍任务写下一拍；人设与世界以资产区为准。\n\
2. 死人退场，写后果；禁止重演已写完的行刺或死亡。\n\
3. 只用本拍名单；禁止按书名发明未出场角色。\n\
4. 禁止复述已有正文，禁止同一段出现两次。\n\
5. 一人一号；在场者可不出声；禁止点名式每人一段。\n\
6. 本拍必须兑现节拍卡「必须改变」；禁止只复述末句或全员表态。\n\
7. 文笔（人类文笔基线）：情绪四模式混用——行为为主，可直说「她害怕」，身体反应只留峰值；\n\
   句长参差，不连续三句以上长度相近；不解释主题、不写「她终于明白」与格言收尾；\n\
   对话标签重复「说」即可；用写得出名字的实物替代「一股说不清的感觉」。\n\
Wrong：大堂里再写一遍飞身扑上、刀刺苏会山。\n\
Right：苏会山已死，写现场后果与在场活人的反应。\n\
Wrong：先输出节拍任务、状态网或约束清单。\n\
Right：直接写场面，只输出正文。\n\
Wrong：凭书名拉出一个本拍名单没有的人。\n\
Right：只用阵容里的名字。\n\
Wrong：曹元佩僵住了。琬公主曹元佩蜷缩在角落。琬公主曹元佩抱着曹元佩的衣角。\n\
Right：曹元佩是镇北王妃，一人；已死者不再看、不再审视。\n\
Wrong：众人依次震惊，场面停在原地。\n\
Right：一件不可逆新信息或选择落地，其余人可沉默。\n\
Wrong：她心里一紧，背脊发凉，喉咙发紧。（每个情绪都写成身体反应）\n\
Right：她害怕。他把灯芯剪短了一截，没说话。";

pub const DRAMA_BEAT_SYSTEM: &str = "你是短剧编剧。只输出可拍剧本，不要分析或清单。\n\
1. 按节拍任务写下一场；人设与世界以资产区为准。\n\
2. 每场标头必须含场次、内景或外景、地点、时间。\n\
3. 动作写正在发生的可见可听内容；禁止镜号、景别、运镜或影片模型参数。\n\
4. 台词推进冲突，不解说背景；本拍必须兑现「必须改变」。\n\
5. 只用本拍名单；一人一号；禁止复述已有场次。\n\
6. 情绪用可见动作与台词演（剧本语域）——不要用「心里一紧、背脊发凉」类内心描写代替可拍内容；\n\
   台词允许语气词（啊/吧/呢/嘛），允许重复「说」，不必轮换低语/咕哝/嗤笑。\n\
Wrong：【场景】夜里便利店，众人轮流说话。\n\
Right：1. 内景・便利商店・夜\n\
Wrong：加入镜头推进与 Sora 提示词。\n\
Right：只写动作、角色名与台词。\n\
Wrong：她心里一紧，背脊发凉。（不可拍）\n\
Right：她把手里的袋子捏皱了，往后退了半步。「你别过来。」\n\
Wrong：众人依次震惊，场面停在原地。\n\
Right：一件不可逆新信息落地，切黑留下下一步问题。";

pub const TOOL_LOOP_PROTOCOL: &str = "你只能输出一个 JSON action，不要输出其他内容：\n\
- 调用工具: {\"type\":\"tool\",\"name\":\"<工具名>\",\"args\":{...}}\n\
- 完成任务: {\"type\":\"final\",\"content\":\"<最终产出>\"}";

const GENESIS_FIRST_CHAPTER_TASK: &str = "写作要求：第一章正文，1500-2500 字，只输出正文，不写标题。须紧扣故事大纲的起因（第一幕）开篇。故事前提是你的创作方向；创作资产（世界观/大纲/伏笔）是硬约束，须在硬约束内落实前提核心意图，不得自相矛盾。";

const GENESIS_PROSE_FALLBACK_TASK: &str = "写作要求：章节正文，1500-2500 字，只输出正文，不写标题。故事前提是你的创作方向；创作资产（世界观/大纲/伏笔）是硬约束，须在硬约束内落实前提核心意图，不得自相矛盾。";

fn l(id: &'static str, kind: LayerKind, slot: Slot, body: String, required: bool) -> Layer {
    Layer {
        id,
        kind,
        slot,
        body,
        required,
    }
}

pub fn assemble_genesis_first_chapter(
    premise: &str,
    concept_json: &str,
    assets_ctx: &str,
) -> Result<AssembledPrompt, AssembleError> {
    assemble_genesis_first_chapter_for(premise, concept_json, assets_ctx, "novel")
}

pub fn assemble_genesis_prose_fallback(
    premise: &str,
    assets_ctx: &str,
) -> Result<AssembledPrompt, AssembleError> {
    assemble(&[
        l(
            "instruction",
            LayerKind::Instruction,
            Slot::System,
            GENESIS_PROSE_FALLBACK_SYSTEM.to_string(),
            true,
        ),
        l(
            "premise",
            LayerKind::Context,
            Slot::User,
            format!("故事前提：{premise}"),
            true,
        ),
        l(
            "assets",
            LayerKind::Context,
            Slot::User,
            format!("创作资产：\n{assets_ctx}"),
            true,
        ),
        l(
            "task",
            LayerKind::Context,
            Slot::User,
            GENESIS_PROSE_FALLBACK_TASK.to_string(),
            true,
        ),
    ])
}

pub const GENESIS_DRAMA_FIRST_CHAPTER_SYSTEM: &str = "你是短剧编剧，只输出可拍剧本。人设、世界观以下方资产区为准，不得自相矛盾。开篇须有可见行动与未解问题。场次标头含内景或外景、地点、时间。禁止镜号、景别、运镜或影片模型参数。";

pub fn assemble_continue_beat(user: &str) -> Result<AssembledPrompt, AssembleError> {
    assemble_continue_beat_for(user, "novel")
}

pub fn assemble_continue_beat_for(
    user: &str,
    story_format: &str,
) -> Result<AssembledPrompt, AssembleError> {
    let system = if story_format == "short_drama" {
        DRAMA_BEAT_SYSTEM
    } else {
        CONTINUE_BEAT_SYSTEM
    };
    assemble(&[
        l(
            "instruction",
            LayerKind::Instruction,
            Slot::System,
            system.to_string(),
            true,
        ),
        l(
            "continue_user",
            LayerKind::Context,
            Slot::User,
            user.to_string(),
            true,
        ),
    ])
}

pub fn assemble_genesis_first_chapter_for(
    premise: &str,
    concept_json: &str,
    assets_ctx: &str,
    story_format: &str,
) -> Result<AssembledPrompt, AssembleError> {
    let system = if story_format == "short_drama" {
        GENESIS_DRAMA_FIRST_CHAPTER_SYSTEM
    } else {
        GENESIS_FIRST_CHAPTER_SYSTEM
    };
    assemble(&[
        l(
            "instruction",
            LayerKind::Instruction,
            Slot::System,
            system.to_string(),
            true,
        ),
        l(
            "premise",
            LayerKind::Context,
            Slot::User,
            format!("故事前提：{premise}"),
            true,
        ),
        l(
            "concept",
            LayerKind::Context,
            Slot::User,
            format!("概念设定：{concept_json}"),
            true,
        ),
        l(
            "assets",
            LayerKind::Context,
            Slot::User,
            format!("创作资产：\n{assets_ctx}"),
            true,
        ),
        l(
            "task",
            LayerKind::Context,
            Slot::User,
            if story_format == "short_drama" {
                "写作要求：第 1 集可拍剧本，只输出剧本。须有场次标头、可见动作、冲突与集尾钩子。故事前提是创作方向；资产是硬约束。"
                    .to_string()
            } else {
                GENESIS_FIRST_CHAPTER_TASK.to_string()
            },
            true,
        ),
    ])
}

pub fn assemble_tool_loop_head(
    catalog: &str,
    task: &str,
) -> Result<AssembledPrompt, AssembleError> {
    // P0 对外串等价；trim 与历史 format 对 catalog 尾 NL 不一致。
    assemble(&[
        l(
            "tools",
            LayerKind::Tools,
            Slot::User,
            catalog.to_string(),
            true,
        ),
        l(
            "protocol",
            LayerKind::Instruction,
            Slot::User,
            TOOL_LOOP_PROTOCOL.to_string(),
            true,
        ),
        l(
            "task",
            LayerKind::Context,
            Slot::User,
            format!("任务：\n{task}"),
            true,
        ),
    ])?;
    Ok(AssembledPrompt {
        system: String::new(),
        user: format!("{}\n\n{}\n\n任务：\n{}", catalog, TOOL_LOOP_PROTOCOL, task),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(id: &'static str, slot: Slot, body: &str, required: bool) -> Layer {
        Layer {
            id,
            kind: LayerKind::Context,
            slot,
            body: body.to_string(),
            required,
        }
    }

    #[test]
    fn assemble_joins_system_and_user_with_blank_line() {
        let out = assemble(&[
            layer("a", Slot::System, "SYS-A", true),
            layer("b", Slot::System, "SYS-B", true),
            layer("c", Slot::User, "USR-C", true),
            layer("d", Slot::User, "USR-D", true),
        ])
        .unwrap();
        assert_eq!(out.system, "SYS-A\n\nSYS-B");
        assert_eq!(out.user, "USR-C\n\nUSR-D");
    }

    #[test]
    fn assemble_rejects_duplicate_id() {
        let err = assemble(&[
            layer("same", Slot::System, "x", true),
            layer("same", Slot::User, "y", true),
        ])
        .unwrap_err();
        assert_eq!(err, AssembleError::DuplicateId("same"));
    }

    #[test]
    fn assemble_rejects_missing_required() {
        let err = assemble(&[layer("need", Slot::System, "  \n", true)]).unwrap_err();
        assert_eq!(err, AssembleError::MissingRequired("need"));
    }

    #[test]
    fn assemble_skips_empty_optional() {
        let out = assemble(&[
            layer("keep", Slot::User, "BODY", true),
            layer("skip", Slot::User, "  ", false),
        ])
        .unwrap();
        assert_eq!(out.user, "BODY");
        assert!(out.system.is_empty());
    }

    #[test]
    fn genesis_first_chapter_matches_legacy_format() {
        let premise = "一部间谍小说";
        let concept = "{\"logline\":\"x\"}";
        let assets = "【世界观】双星";
        let out = assemble_genesis_first_chapter(premise, concept, assets).unwrap();
        assert_eq!(
            out.system,
            "你是小说主创，只输出章节正文。人设、世界观与已埋伏笔以下方资产区为准，不得自相矛盾、不得发明与资产冲突的角色或设定。禁止重复：同一段落/句子不得出现两次，不得复述已有正文。开篇须有可见行动与未解问题。"
        );
        let expected_user = format!(
            "故事前提：{}\n\n概念设定：{}\n\n创作资产：\n{}\n\n写作要求：第一章正文，1500-2500 字，只输出正文，不写标题。须紧扣故事大纲的起因（第一幕）开篇。故事前提是你的创作方向；创作资产（世界观/大纲/伏笔）是硬约束，须在硬约束内落实前提核心意图，不得自相矛盾。",
            premise, concept, assets
        );
        assert_eq!(out.user, expected_user);
    }

    #[test]
    fn genesis_drama_first_chapter_uses_drama_system() {
        let out =
            assemble_genesis_first_chapter_for("竖屏短剧", "{}", "资产", "short_drama").unwrap();
        assert_eq!(out.system, GENESIS_DRAMA_FIRST_CHAPTER_SYSTEM);
        assert!(out.user.contains("第 1 集可拍剧本"));
        assert!(out.system.contains("禁止镜号"));
    }

    #[test]
    fn genesis_prose_fallback_matches_legacy_format() {
        let out = assemble_genesis_prose_fallback("前提", "资产正文").unwrap();
        assert_eq!(
            out.system,
            "你是小说主创，只输出章节正文。人设、世界观与已埋伏笔以下方资产区为准，不得自相矛盾。禁止重复：同一段落/句子不得出现两次，不得复述已有正文。开篇须有可见行动与未解问题。"
        );
        assert_eq!(
            out.user,
            "故事前提：前提\n\n创作资产：\n资产正文\n\n写作要求：章节正文，1500-2500 字，只输出正文，不写标题。故事前提是你的创作方向；创作资产（世界观/大纲/伏笔）是硬约束，须在硬约束内落实前提核心意图，不得自相矛盾。"
        );
    }

    /// v0.65.1 契约：主续写路径的 system 提示词必须携带人类文笔基线准则
    /// （它是内联常量、用户不可覆盖，是准则送达的最后一道保证）。
    #[test]
    fn continue_beat_system_carries_human_voice_doctrine() {
        for needle in [
            "人类文笔基线",
            "情绪四模式混用",
            "句长参差",
            "不解释主题",
            "对话标签重复「说」",
        ] {
            assert!(
                CONTINUE_BEAT_SYSTEM.contains(needle),
                "CONTINUE_BEAT_SYSTEM 缺少「{needle}」"
            );
        }
    }

    /// v0.65.2 契约：短剧路径的 system 提示词携带剧本语域的文笔约束
    /// （情绪要靠可拍的动作与台词，不用内心身体描写替代）。
    #[test]
    fn drama_beat_system_bans_unfilmable_interiority() {
        for needle in ["可见动作与台词", "不可拍", "语气词"] {
            assert!(
                DRAMA_BEAT_SYSTEM.contains(needle),
                "DRAMA_BEAT_SYSTEM 缺少「{needle}」"
            );
        }
    }

    #[test]
    fn continue_beat_keeps_user_opaque_and_locks_system() {
        let user = "【节拍任务】\n去码头";
        let out = assemble_continue_beat(user).unwrap();
        assert_eq!(out.system, CONTINUE_BEAT_SYSTEM);
        assert_eq!(out.user, user);
    }

    #[test]
    fn continue_beat_operational_contract_has_three_examples() {
        let lines: Vec<&str> = CONTINUE_BEAT_SYSTEM
            .lines()
            .filter(|l| !l.is_empty())
            .collect();
        // v0.65.2：加入人类文笔基线条款（1 条文笔准则 + 1 组「别把情绪都写成
        // 身体反应」示例），上界相应上调；仍设上界，防止这个 system
        // 提示词无界膨胀。
        assert!(
            (11..=24).contains(&lines.len()),
            "合同应为 11–24 行，实际 {} 行: {CONTINUE_BEAT_SYSTEM}",
            lines.len()
        );
        assert!(CONTINUE_BEAT_SYSTEM.contains("Wrong：大堂里再写一遍飞身扑上"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Right：苏会山已死"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Wrong：先输出节拍任务、状态网或约束清单"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Right：直接写场面，只输出正文"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Wrong：凭书名拉出一个本拍名单没有的人"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Right：只用阵容里的名字"));
        assert!(CONTINUE_BEAT_SYSTEM.contains(
            "Wrong：曹元佩僵住了。琬公主曹元佩蜷缩在角落。琬公主曹元佩抱着曹元佩的衣角。"
        ));
        assert!(CONTINUE_BEAT_SYSTEM
            .contains("Right：曹元佩是镇北王妃，一人；已死者不再看、不再审视。"));
        assert!(!CONTINUE_BEAT_SYSTEM.contains("asset_read"));
        assert!(!CONTINUE_BEAT_SYSTEM.contains("JSON action"));
    }

    #[test]
    fn continue_system_has_stall_example() {
        assert!(CONTINUE_BEAT_SYSTEM.contains("必须兑现节拍卡「必须改变」"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Wrong：众人依次震惊，场面停在原地。"));
        assert!(CONTINUE_BEAT_SYSTEM.contains("Right：一件不可逆新信息或选择落地，其余人可沉默。"));
    }

    #[test]
    fn drama_beat_system_requires_scene_heading_not_shot_list() {
        let out = assemble_continue_beat_for("从门口写", "short_drama").unwrap();
        assert_eq!(out.system, DRAMA_BEAT_SYSTEM);
        assert!(out.system.contains("内景或外景"));
        assert!(out.system.contains("禁止镜号"));
        assert!(!out.system.contains("asset_read"));
        let novel = assemble_continue_beat("从门口写").unwrap();
        assert_eq!(novel.system, CONTINUE_BEAT_SYSTEM);
        assert!(!novel.system.contains("内景或外景"));
    }

    #[test]
    fn continue_beat_complete_does_not_require_tools() {
        let out = assemble_continue_beat("从飞身扑上之后写").unwrap();
        assert!(!out.user.contains("可用工具"));
        assert!(!out.system.contains("JSON action"));
        assert!(!out.user.contains("board_read"));
        assert!(!out.user.contains("asset_read"));
        assert!(!out.system.contains("asset_read"));
        assert!(crate::llm::adapter::GenerateRequest::default()
            .tools
            .is_none());
    }

    #[test]
    fn continue_beat_anti_repeat_suffix_is_concat() {
        let out = assemble_continue_beat("u").unwrap();
        let retry = format!("{} 禁止重复同一段落或意象循环，不得首尾回环。", out.system);
        assert!(retry.starts_with(&out.system));
        assert!(retry.contains("不得首尾回环"));
    }

    #[test]
    fn tool_loop_head_matches_legacy_format() {
        let catalog = "可用工具（JSON action 调用）：\n- board_read: 读\n  参数: {}\n";
        let task = "写一章";
        let out = assemble_tool_loop_head(catalog, task).unwrap();
        assert!(out.system.is_empty());
        let expected = format!(
            "{}\n\n你只能输出一个 JSON action，不要输出其他内容：\n\
             - 调用工具: {{\"type\":\"tool\",\"name\":\"<工具名>\",\"args\":{{...}}}}\n\
             - 完成任务: {{\"type\":\"final\",\"content\":\"<最终产出>\"}}\n\n任务：\n{}",
            catalog, task
        );
        assert_eq!(out.user, expected);
    }

    #[test]
    fn tool_loop_head_rejects_empty_catalog() {
        let err = assemble_tool_loop_head(" ", "写一章").unwrap_err();
        assert_eq!(err, AssembleError::MissingRequired("tools"));
    }
}
