//! Asset snapshot domain types.
//!
//! Neutral view of the creative-asset snapshot consumed by agents.
//! The loader implementation stays in `crate::creative_engine::asset_snapshot`.

/// 角色状态快照（中性视图）
#[derive(Debug, Clone)]
pub struct CharacterStateSnapshot {
    pub character_id: String,
    pub name: String,
    pub current_location: Option<String>,
    pub current_emotion: Option<String>,
    pub active_goal: Option<String>,
    pub arc_progress: f32,
    /// v0.60.0 P0-T2：知识边界——该角色已知 / 尚不知道的秘密。
    /// 此前 adapter 转换时丢弃，导致 Agency 上下文看不到信息差。
    pub secrets_known: Vec<String>,
    pub secrets_unknown: Vec<String>,
}

/// 活跃冲突（中性视图）
#[derive(Debug, Clone)]
pub struct ActiveConflict {
    pub conflict_type: String,
    pub parties: Vec<String>,
    pub stakes: String,
}

/// 两条创作路径共享的精选资产快照（中性视图）。
#[derive(Debug, Clone)]
pub struct AssetSnapshot {
    /// 规范状态快照（叙事阶段 + 活跃冲突 + 伏笔状态 + 角色状态）
    pub narrative_phase_guidance: Option<String>,
    /// 主导风格一句话摘要
    pub style_dna_summary: Option<String>,
    /// 待回收伏笔（top n）
    pub pending_foreshadowings: Vec<String>,
    /// 逾期伏笔（top n）
    pub overdue_foreshadowings: Vec<String>,
    /// 角色当前状态
    pub character_states: Vec<CharacterStateSnapshot>,
    /// 活跃冲突
    pub active_conflicts: Vec<ActiveConflict>,
}
