//! 情感回路（Story Emotion Circuit, SEC）。
//!
//! 设计：`docs/plans/2026-10-31-story-emotion-circuit-design.md`
//!
//! 本模块是纯领域层：情绪向量、OCC 评估、载体分配、压力场、探针与读者面板
//! 的数据结构都可单测，不依赖 `agency`。I/O 只在 `repo` 子模块。

pub mod adaptation;
pub mod appraisal;
pub mod carriers;
pub mod circuit;
pub mod modulation;
pub mod probe;
pub mod reader;
pub mod repo;
pub mod types;
pub mod vector;
pub mod whitebox;

pub use adaptation::{palette_for_genre, peak_budget_for_genre, suggest_contract};
pub use appraisal::{
    action_tendency, appraise_event, emotion_from_appraisal, sensitivities_from, ActionTendency,
    Agency, Appraisal, EventKind, Sensitivities,
};
pub use carriers::{select_carriers, Carrier, CarrierPlan};
pub use circuit::{select_next_beat, BeatSelectionInput, CastEmotionProfile, EmotionPlan};
pub use modulation::{
    render_chapter_emotion_plan, render_emotion_block, render_emotion_state_block,
};
pub use probe::{run_emotion_probes, EmotionProbe, ProbeSeverity};
pub use reader::{
    aggregate_reviews, build_reader_panel_prompt, parse_reader_review, ReaderAggregate,
    ReaderReview, ReaderScores, READER_PERSONAS,
};
pub use repo::EmotionCircuitRepository;
pub use types::{
    CharacterEmotionState, DialoguePolicy, EmotionBeat, EmotionContract, EmotionDebt,
    EmotionProbeLog, EmotionTension, ReaderResonanceReview, ReaderTarget, SceneEmotionBeat,
};
pub use vector::{EmotionKind, EmotionVector};
pub use whitebox::{whitebox_available, NoopWhiteboxAdapter, WhiteboxEmotionAdapter};
