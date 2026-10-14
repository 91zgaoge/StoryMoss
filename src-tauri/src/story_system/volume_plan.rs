//! P1-3（v0.70.0）：卷/弧中层计划——VOLUME 合同。
//!
//! 缘起：docs/audits/2026-10-14-sherpa-benchmark-analysis.md——Sherpa
//! 的层级规划器是 「季→弧→序列→剧集→场景」；StoryMoss 的规划链是 三幕书纲 →
//! 逐章章纲 → 节拍卡， 章与书之间没有**正向计划层**（段摘要每 10
//! 章是回顾不是计划）。而 `story_contracts` 的 `VOLUME`
//! 类型早已定义、读路径已实现、全库无写入方——本模块把这个空位补齐：
//!
//! - **存**：`story_contracts`（contract_type='VOLUME'，内容 JSON）——不新增表；
//! - **读写**：`list/save`（按卷号原地更新，`version` 递增，与 CHAPTER
//!   合同同语义）；
//! - **注入**：章纲生成时带「本卷目标 +
//!   卷内进度」（`render_volume_block_for_chapter`）；
//! - **边界**：`volume_overview` 给出「无卷纲 / 卷内（余 N 章）/
//!   已越过卷末（该规划下一卷）」，
//!   由作者在运行维护页确认生成（渐进式规划：用到哪、规划到哪）。
//!
//! 纪律：卷纲只由作者显式触发生成（不自动改写）；
//! 短篇没有卷纲时行为不变（注入块不出现）。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::{DbPool, StoryContractRepository};

/// 单卷默认章数（创世/续写规模估计的落点；生成时作为建议交给模型，可被调整）。
pub const DEFAULT_VOLUME_CHAPTERS: i32 = 60;

/// 卷纲（VOLUME 合同 JSON 的结构化形态）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumePlan {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default = "default_contract_type")]
    pub contract_type: String,
    #[serde(default)]
    pub generator_version: String,
    pub volume_number: i32,
    pub start_chapter: i32,
    pub end_chapter: i32,
    /// 卷目标：这一卷要把故事推到哪。
    #[serde(default)]
    pub goal: String,
    /// 卷末高潮：这一卷结束时必须发生的事。
    #[serde(default)]
    pub climax: String,
    /// 须在本卷兑现的伏笔（正文/设定里已有的线）。
    #[serde(default)]
    pub foreshadowing_due: Vec<String>,
    /// 本卷允许新开的线索条数（防线头无限增殖）。
    #[serde(default)]
    pub new_thread_budget: u32,
    #[serde(default)]
    pub scale_estimate: String,
}

fn default_schema_version() -> String {
    "story-system/v1".to_string()
}

fn default_contract_type() -> String {
    "VOLUME".to_string()
}

impl VolumePlan {
    /// 归一：卷号/章范围修正（end ≥ start、卷号 ≥ 1）。
    pub fn normalize(&mut self) {
        if self.volume_number < 1 {
            self.volume_number = 1;
        }
        if self.start_chapter < 1 {
            self.start_chapter = 1;
        }
        if self.end_chapter < self.start_chapter {
            self.end_chapter = self.start_chapter + DEFAULT_VOLUME_CHAPTERS - 1;
        }
        self.foreshadowing_due.retain(|s| !s.trim().is_empty());
        self.foreshadowing_due.truncate(8);
    }

    /// 校验：目标非空且章范围合法。
    pub fn validate(&self) -> Result<(), String> {
        if self.goal.trim().is_empty() {
            return Err("卷纲缺少卷目标".to_string());
        }
        if self.end_chapter < self.start_chapter {
            return Err("卷纲章范围倒挂".to_string());
        }
        Ok(())
    }

    pub fn covers(&self, chapter: i32) -> bool {
        chapter >= self.start_chapter && chapter <= self.end_chapter
    }
}

/// 从合同 JSON 宽松解析卷纲（缺字段用默认值；解析失败返回 None）。
pub fn parse_volume_plan(json: &str) -> Option<VolumePlan> {
    let mut plan: VolumePlan = serde_json::from_str(json).ok()?;
    plan.normalize();
    Some(plan)
}

/// 列出全部卷纲（按卷号升序；同卷号取最新版本）。
pub fn list_volume_plans(pool: &DbPool, story_id: &str) -> Result<Vec<VolumePlan>, String> {
    let contracts = StoryContractRepository::new(pool.clone())
        .get_by_story(story_id)
        .map_err(|e| format!("查询合同失败: {e}"))?;
    // get_by_story 按 version DESC 排列：同卷号先到者即最新版本。
    let mut seen = std::collections::HashSet::new();
    let mut plans: Vec<VolumePlan> = Vec::new();
    for contract in contracts.iter().filter(|c| c.contract_type == "VOLUME") {
        let Some(plan) = parse_volume_plan(&contract.contract_json) else {
            continue;
        };
        if seen.insert(plan.volume_number) {
            plans.push(plan);
        }
    }
    plans.sort_by_key(|p| p.volume_number);
    Ok(plans)
}

/// 保存卷纲：同卷号已有合同则原地更新（version 递增），否则新建。
pub fn save_volume_plan(pool: &DbPool, story_id: &str, plan: &VolumePlan) -> Result<(), String> {
    let mut plan = plan.clone();
    plan.normalize();
    plan.validate()?;
    plan.generator_version = format!("v0.70.0");
    let json = serde_json::to_string(&plan).map_err(|e| format!("序列化卷纲失败: {e}"))?;
    let repo = StoryContractRepository::new(pool.clone());
    let contracts = repo
        .get_by_story(story_id)
        .map_err(|e| format!("查询合同失败: {e}"))?;
    let existing = contracts.iter().find(|c| {
        c.contract_type == "VOLUME"
            && parse_volume_plan(&c.contract_json)
                .map(|p| p.volume_number == plan.volume_number)
                .unwrap_or(false)
    });
    match existing {
        Some(contract) => repo
            .update(&contract.id, &json)
            .map(|_| ())
            .map_err(|e| format!("更新卷纲失败: {e}")),
        None => repo
            .create(story_id, "VOLUME", &json)
            .map(|_| ())
            .map_err(|e| format!("创建卷纲失败: {e}")),
    }
}

/// 覆盖第 `chapter` 章的卷纲（若有）。
pub fn current_volume_plan(pool: &DbPool, story_id: &str, chapter: i32) -> Option<VolumePlan> {
    list_volume_plans(pool, story_id)
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.covers(chapter))
}

/// 最新已写章（scenes.sequence_number 最大值，0 表示还没写）。
pub fn latest_chapter(pool: &DbPool, story_id: &str) -> i32 {
    pool.get()
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT MAX(sequence_number) FROM scenes WHERE story_id = ?1",
                params![story_id],
                |r| r.get::<_, Option<i32>>(0),
            )
            .ok()
            .flatten()
        })
        .unwrap_or(0)
}

/// 卷纲总览（运行维护页「卷纲」页签与命令的载荷）。
#[derive(Debug, Clone, Serialize)]
pub struct VolumeOverview {
    pub plans: Vec<VolumePlan>,
    pub latest_chapter: i32,
    /// `no_plan`（尚无卷纲）/ `in_volume`（在计划范围内）/
    /// `past_end`（越过最后一卷卷末）。
    pub state: String,
    pub current_volume_number: Option<i32>,
    pub current_volume_remaining: Option<i32>,
    /// 建议规划的下一个卷（`no_plan` 时为第 1 卷、起始第 1 章）。
    pub next_volume_number: Option<i32>,
    pub next_start_chapter: Option<i32>,
}

/// 卷纲状态机（确定性；不写库）。
pub fn volume_overview(pool: &DbPool, story_id: &str) -> Result<VolumeOverview, String> {
    let plans = list_volume_plans(pool, story_id)?;
    let latest = latest_chapter(pool, story_id);
    let mut overview = VolumeOverview {
        plans: plans.clone(),
        latest_chapter: latest,
        state: "no_plan".to_string(),
        current_volume_number: None,
        current_volume_remaining: None,
        next_volume_number: Some(1),
        next_start_chapter: Some(1),
    };
    let Some(last) = plans.last() else {
        return Ok(overview);
    };
    if latest > last.end_chapter {
        // 越过了最后一卷的卷末：该规划下一卷。
        overview.state = "past_end".to_string();
        overview.next_volume_number = Some(last.volume_number + 1);
        overview.next_start_chapter = Some(last.end_chapter + 1);
        return Ok(overview);
    }
    if let Some(current) = plans.iter().find(|p| p.covers(latest.max(1))) {
        overview.state = "in_volume".to_string();
        overview.current_volume_number = Some(current.volume_number);
        overview.current_volume_remaining = Some(current.end_chapter - latest);
        overview.next_volume_number = Some(current.volume_number + 1);
        overview.next_start_chapter = Some(current.end_chapter + 1);
    }
    Ok(overview)
}

/// 渲染注入块：本卷计划 + 卷内进度（无内容字段时省略对应行）。
pub fn render_volume_block(plan: &VolumePlan, current_chapter: i32) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "【本卷计划（第 {} 卷：第 {}–{} 章）】",
        plan.volume_number, plan.start_chapter, plan.end_chapter
    ));
    if !plan.goal.trim().is_empty() {
        lines.push(format!("卷目标：{}", plan.goal.trim()));
    }
    if !plan.climax.trim().is_empty() {
        lines.push(format!("卷末高潮：{}", plan.climax.trim()));
    }
    if !plan.foreshadowing_due.is_empty() {
        lines.push(format!(
            "须在本卷兑现：{}",
            plan.foreshadowing_due.join("；")
        ));
    }
    if plan.new_thread_budget > 0 {
        lines.push(format!("新开线索预算：≤{} 条", plan.new_thread_budget));
    }
    if current_chapter > 0 {
        lines.push(format!(
            "卷内进度：第 {} 章 / 共 {} 章",
            current_chapter, plan.end_chapter
        ));
    }
    lines.join("\n")
}

/// 覆盖第 `chapter` 章的注入块；无卷纲返回 None（短篇行为不变）。
pub fn render_volume_block_for_chapter(
    pool: &DbPool,
    story_id: &str,
    chapter: i32,
) -> Option<String> {
    current_volume_plan(pool, story_id, chapter).map(|plan| render_volume_block(&plan, chapter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "卷纲".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id
    }

    fn seed_scene(pool: &DbPool, story_id: &str, n: i32) {
        crate::db::repositories::SceneRepository::new(pool.clone())
            .create(story_id, n, Some(&format!("第{n}章")))
            .unwrap();
    }

    fn plan(volume_number: i32, start: i32, end: i32, goal: &str) -> VolumePlan {
        VolumePlan {
            schema_version: "story-system/v1".into(),
            contract_type: "VOLUME".into(),
            generator_version: String::new(),
            volume_number,
            start_chapter: start,
            end_chapter: end,
            goal: goal.into(),
            climax: "卷末：玉佩的来历揭开一半".into(),
            foreshadowing_due: vec!["断灯绳的来历".into()],
            new_thread_budget: 2,
            scale_estimate: "约 60 章".into(),
        }
    }

    #[test]
    fn save_list_and_current_round_trip() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        save_volume_plan(&pool, &story_id, &plan(1, 1, 60, "站稳柳林集")).unwrap();
        save_volume_plan(&pool, &story_id, &plan(2, 61, 120, "进京")).unwrap();

        let plans = list_volume_plans(&pool, &story_id).unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].volume_number, 1);
        assert_eq!(plans[1].goal, "进京");
        assert_eq!(
            current_volume_plan(&pool, &story_id, 30)
                .unwrap()
                .volume_number,
            1
        );
        assert_eq!(
            current_volume_plan(&pool, &story_id, 61)
                .unwrap()
                .volume_number,
            2
        );
        assert!(current_volume_plan(&pool, &story_id, 121).is_none());

        // 同卷号再存 → 原地更新（version 递增、行数不变）。
        save_volume_plan(&pool, &story_id, &plan(1, 1, 60, "站稳柳林集（修订）")).unwrap();
        let plans = list_volume_plans(&pool, &story_id).unwrap();
        assert_eq!(plans.len(), 2, "同卷号不得出现第二行");
        assert_eq!(plans[0].goal, "站稳柳林集（修订）");
        let contracts = StoryContractRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        let row = contracts
            .iter()
            .find(|c| {
                c.contract_type == "VOLUME"
                    && parse_volume_plan(&c.contract_json)
                        .map(|p| p.volume_number == 1)
                        .unwrap()
            })
            .unwrap();
        assert!(
            row.version >= 2,
            "更新应递增 version（实际 {}）",
            row.version
        );
    }

    #[test]
    fn overview_states_progressively() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_scene(&pool, &story_id, 12);

        // 无卷纲 → no_plan，建议第 1 卷从第 1 章起。
        let ov = volume_overview(&pool, &story_id).unwrap();
        assert_eq!(ov.state, "no_plan");
        assert_eq!(ov.next_volume_number, Some(1));
        assert_eq!(ov.next_start_chapter, Some(1));

        // 有卷纲且在第 1 卷内 → in_volume，余 48 章。
        save_volume_plan(&pool, &story_id, &plan(1, 1, 60, "站稳")).unwrap();
        let ov = volume_overview(&pool, &story_id).unwrap();
        assert_eq!(ov.state, "in_volume");
        assert_eq!(ov.current_volume_number, Some(1));
        assert_eq!(ov.current_volume_remaining, Some(48));
        assert_eq!(ov.next_volume_number, Some(2));
        assert_eq!(ov.next_start_chapter, Some(61));

        // 最新章越过卷末 → past_end。
        seed_scene(&pool, &story_id, 61);
        let ov = volume_overview(&pool, &story_id).unwrap();
        assert_eq!(ov.state, "past_end");
        assert_eq!(ov.next_volume_number, Some(2));
        assert_eq!(ov.next_start_chapter, Some(61));
    }

    #[test]
    fn render_block_carries_goal_climax_and_progress() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        save_volume_plan(&pool, &story_id, &plan(1, 1, 60, "站稳柳林集")).unwrap();
        let block = render_volume_block_for_chapter(&pool, &story_id, 30).expect("应有注入块");
        assert!(block.contains("第 1 卷：第 1–60 章"));
        assert!(block.contains("卷目标：站稳柳林集"));
        assert!(block.contains("卷末高潮："));
        assert!(block.contains("须在本卷兑现：断灯绳的来历"));
        assert!(block.contains("新开线索预算：≤2 条"));
        assert!(block.contains("卷内进度：第 30 章 / 共 60 章"));
        // 不在任何卷内 → 无块。
        assert!(render_volume_block_for_chapter(&pool, &story_id, 61).is_none());
        // 无卷纲的故事 → 无块（短篇行为不变）。
        let other = seed_story(&pool);
        assert!(render_volume_block_for_chapter(&pool, &other, 1).is_none());
    }

    #[test]
    fn parse_is_lenient_and_validation_rejects_empty_goal() {
        let parsed = parse_volume_plan(r#"{"volume_number":1,"start_chapter":1,"end_chapter":40}"#)
            .expect("缺字段应宽松解析");
        assert_eq!(parsed.end_chapter, 40);
        assert!(parsed.goal.is_empty());
        assert!(parsed.validate().is_err(), "空目标应校验失败");

        let mut inverted = plan(1, 10, 5, "目标");
        inverted.normalize();
        assert!(
            inverted.end_chapter >= inverted.start_chapter,
            "倒挂范围应被归一"
        );
    }
}
