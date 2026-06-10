//! 高密度状态世界构建法 (High-Density World Building)
//!
//! 源于90年代经典老游戏在极致资源约束下的结构智慧。
//! 核心理念：用极少的元素，通过状态驱动、桥节点连接、事件回流与多功能重用，
//! 构建出远大于实际篇幅的"活的世界"。

use std::str::FromStr;

use super::Methodology;

/// 高密度世界构建法的四个阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldBuildingPhase {
    Seed = 1,         // 1. 最小世界种子
    StateExpansion,   // 2. 状态网扩张
    Convergence,      // 3. 多线交织与回流
    DensityIteration, // 4. 密度迭代与克制
}

const SEED_PROMPT: &str = r#"高密度世界构建法 - 第1阶段：最小世界种子

请设计一个高密度"世界切片"，要求：

1. 核心种子（1个锚点场景/地点/事件）：
   - 选择一个极具张力的"小切片"作为世界入口
   - 这个切片必须能同时承载：叙事推进、世界观展示、角色关系揭示

2. 状态向量（为核心人物定义动态状态）：
   每个重要人物维持：
   - 身份/位置
   - 资源（物质、金钱、知识、信息、物品）
   - 关系旗标（对关键人物的态度、恩怨、秘密）
   - 历史/事件旗标
   - 心理/目标状态

3. 桥节点（3-5个高连接度元素）：
   - 设计必须承担多条线、多重功能的节点
   - 节点类型：人物、地点、关键物件、秘密/事件
   - 每个桥节点必须至少连接3条不同的叙事线或命运线

规则：
- 情节不是预设的，而是"当前状态 × 世界规则"的函数输出
- 任何行动或事件都应同时修改相关状态
- 不同人物在相同状态下应自然产生不同行动与冲突"#;

const EXPANSION_PROMPT: &str = r#"高密度世界构建法 - 第2阶段：状态网扩张

基于第1阶段的世界种子，进行状态网扩张：

1. 主角群扩展：
   - 为每个新增主角赋予独特初始状态
   - 确保所有主角共享至少1个桥节点
   - 列出每人与其他角色的关系旗标（友好/敌对/利用/误解/秘密）

2. 状态触发表：
   列出关键的状态组合及其触发的事件：
   - "资源匮乏 + 关系敌对" → 冲突事件
   - "掌握秘密 + 身份低微" → 敲诈/逆袭事件
   - "目标一致 + 方法分歧" → 内讧/合作转折
   - "历史恩怨 + 共同危机" → 被迫合作/背叛

3. 世界规则显式化：
   - 这个世界有哪些不可打破的规则？（物理/社会/魔法/经济）
   - 这些规则如何限制或推动人物行动？
   - 规则的例外情况是什么？（例外往往产生最强戏剧张力）

4. 信息不对称矩阵：
   - 每个角色知道什么？不知道什么？
   - 哪些信息差会造成误解或戏剧性反转？
   - 读者知道但角色不知道的"戏剧反讽"有哪些？"#;

const CONVERGENCE_PROMPT: &str = r#"高密度世界构建法 - 第3阶段：多线交织与回流

用桥节点连接多条主人公线，规划事件回流：

1. 桥节点多线映射：
   对每个桥节点，列出：
   - 线A看到什么？（正面/主动视角）
   - 线B看到什么？（侧面/被动视角）
   - 线C看到什么？（误解/信息差视角）
   - 这些不同侧面如何产生戏剧性张力？

2. 回流点规划：
   - 每3-5章至少安排一次回流
   - 回流类型：共享桥节点交汇、共同危机、资源门槛、周期性事件
   - 每次回流必须同时修改至少2个主角的状态向量

3. 事件多功能重用：
   确保每个重要事件同时承担至少3种功能：
   - 叙事功能：推动主线或支线
   - 世界构建功能：展示世界规则或文化
   - 象征/主题功能：强化核心主题
   - 驱动/门槛功能：改变人物可行动范围

4. 伏笔与回响网络：
   - 早期细节在后期产生回响（种子化写作）
   - 每个桥节点至少埋下1个长期伏笔
   - 规划至少3个"重读时刻"（读者重读时发现早就被暗示）"#;

const ITERATION_PROMPT: &str = r#"高密度世界构建法 - 第4阶段：密度迭代与克制

对世界构建进行密度检验与优化：

1. 克制检查清单：
   - 每引入一个新元素，问自己：它能否承载现有功能？能否被现有元素替代？
   - 检查元素利用率：每个已有人物/地点/物件是否承担了至少3种功能？
   - 新人物数量 vs. 现有角色的可扩展性

2. 未写出的世界：
   - 读者能否通过暗示感知到大量未直接描写的运转部分？
   - 世界的留白是否激发读者想象？
   - 检查哪些世界运转部分可以展示而非讲述

3. 状态一致性审计：
   - 所有人物的状态向量在不同章节是否保持一致？
   - 状态变化是否有明确的事件触发？
   - 关系旗标的演变是否符合人物性格和利益逻辑？

4. 涌现性验证：
   - 给定当前所有状态向量，不同人物是否会自然产生冲突？
   - 世界的规则是否足够清晰，让读者能预测（或反向理解）人物选择？
   - 是否存在作者强行推动的痕迹？如何改为状态驱动？

5. 重读价值优化：
   - 确保早期细节在后期产生回响
   - 同一桥节点在不同线中呈现不同侧面
   - 信息不对称在重读时呈现全新层次"#;

impl WorldBuildingPhase {
    pub fn number(&self) -> u8 {
        *self as u8
    }

    pub fn name(&self) -> &'static str {
        match self {
            WorldBuildingPhase::Seed => "最小世界种子",
            WorldBuildingPhase::StateExpansion => "状态网扩张",
            WorldBuildingPhase::Convergence => "多线交织与回流",
            WorldBuildingPhase::DensityIteration => "密度迭代与克制",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            WorldBuildingPhase::Seed => {
                "设计一个高密度'小切片'（一个港口、一场聚会、一个关键物件），\
                 定义核心状态向量和3-5个桥节点"
            }
            WorldBuildingPhase::StateExpansion => {
                "扩展主角群，每人赋予独特初始状态但共享部分桥节点，列出'状态触发表'"
            }
            WorldBuildingPhase::Convergence => {
                "用桥节点连接多条主人公线，规划回流点：每3-5章至少一次通过桥节点产生的交汇"
            }
            WorldBuildingPhase::DensityIteration => {
                "每引入一个新元素问自己能否被现有元素替代，控制新人物/地点数量：宁缺毋滥"
            }
        }
    }

    pub fn prompt_instruction(&self) -> &'static str {
        match self {
            WorldBuildingPhase::Seed => SEED_PROMPT,
            WorldBuildingPhase::StateExpansion => EXPANSION_PROMPT,
            WorldBuildingPhase::Convergence => CONVERGENCE_PROMPT,
            WorldBuildingPhase::DensityIteration => ITERATION_PROMPT,
        }
    }
}

impl FromStr for WorldBuildingPhase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "seed" | "1" | "world_seed" => Ok(WorldBuildingPhase::Seed),
            "state_expansion" | "2" | "expansion" => Ok(WorldBuildingPhase::StateExpansion),
            "convergence" | "3" | "interweaving" => Ok(WorldBuildingPhase::Convergence),
            "density_iteration" | "4" | "iteration" => Ok(WorldBuildingPhase::DensityIteration),
            _ => Err(format!("Unknown world building phase: {}", s)),
        }
    }
}

/// 高密度世界构建法实现
pub struct HighDensityWorldBuildingMethodology {
    phase: WorldBuildingPhase,
}

impl HighDensityWorldBuildingMethodology {
    pub fn new(phase: WorldBuildingPhase) -> Self {
        Self { phase }
    }

    pub fn phase(&self) -> WorldBuildingPhase {
        self.phase
    }

    /// 获取当前阶段及核心上下文
    pub fn cumulative_context(&self) -> String {
        let phase_num = self.phase.number();
        let mut context = format!(
            "你正在使用高密度状态世界构建法进行创作，当前处于第 {} 阶段（共4阶段）。\n\n",
            phase_num
        );

        context.push_str("高密度世界构建法核心原则：\n");
        context.push_str("1. 世界感不来自素材量，而来自结构密度与元素交互\n");
        context.push_str("2. 克制即创造力：硬约束逼出纪律，让少量元素自发产生复杂性\n");
        context.push_str("3. 情节不是预设的，而是'当前状态 × 世界规则'的函数输出\n");
        context.push_str("4. 每一个重要元素都应至少承担3种功能（叙事、世界构建、象征/驱动）\n");
        context
            .push_str("5. 读者在一条线看到另一条线的影子，世界瞬间变厚、有机且大于任何单视角\n\n");

        context.push_str("当前阶段：");
        context.push_str(self.phase.name());
        context.push('\n');
        context.push_str(self.phase.description());
        context.push_str("\n\n");

        context.push_str("当前阶段要求：\n");
        context.push_str(self.phase.prompt_instruction());
        context.push('\n');

        context
    }
}

impl Methodology for HighDensityWorldBuildingMethodology {
    fn name(&self) -> &'static str {
        "高密度世界构建法"
    }

    fn description(&self) -> &'static str {
        "用极少元素通过状态驱动、桥节点连接、事件回流构建活的世界"
    }

    fn system_prompt_extension(&self) -> String {
        self.cumulative_context()
    }

    fn output_schema(&self) -> Option<String> {
        match self.phase {
            WorldBuildingPhase::Seed => Some(
                r##"{"seed": {"anchor_scene": "", "state_vectors": [{"character": "", "identity": "", "resources": "", "relationship_flags": "", "history_flags": "", "psychology": ""}], "bridge_nodes": [{"name": "", "type": "", "connected_lines": [], "functions": []}]}}"##.to_string()
            ),
            WorldBuildingPhase::StateExpansion => Some(
                r##"{"protagonists": [{"name": "", "initial_state": "", "shared_bridges": [], "relationship_matrix": ""}], "trigger_table": [{"state_combination": "", "triggered_event": ""}], "world_rules": [{"rule": "", "constraint": "", "exception": ""}], "information_asymmetry": [{"character": "", "knows": "", "doesnt_know": ""}]}"##.to_string()
            ),
            WorldBuildingPhase::Convergence => Some(
                r##"{"bridge_perspectives": [{"node": "", "line_a_view": "", "line_b_view": "", "line_c_view": ""}], "convergence_points": [{"chapter": 0, "type": "", "affected_states": []}], "event_functions": [{"event": "", "narrative": "", "worldbuilding": "", "symbolic": "", "driver": ""}], "foreshadowing": [{"early_detail": "", "late_payoff": ""}]}"##.to_string()
            ),
            WorldBuildingPhase::DensityIteration => Some(
                r##"{"restraint_check": {"new_elements": [], "replaceable_by_existing": []}, "unwritten_world": {"implied_but_not_shown": []}, "state_consistency": [{"character": "", "states_across_chapters": ""}], "emergence_validation": {"natural_conflicts": [], "rule_based_predictions": []}, "reread_value": [{"early_seed": "", "late_resonance": ""}]}"##.to_string()
            ),
        }
    }

    fn current_step(&self) -> Option<String> {
        Some(format!("{:?}", self.phase))
    }
}
