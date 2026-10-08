//! 续写 run 内冻结节拍卡 / 阵容 / 已截断大纲。
//! 对照 grok-bot `resolveFrozenMemoryPrompt`：同一拍重试不因 Ingest 改提示词。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::agency::{beat_card::SceneBeatCard, continue_director::DirectorLock};

#[derive(Debug, Clone)]
pub struct FrozenContinueShot {
    pub card: SceneBeatCard,
    pub admitted: Vec<String>,
    pub l2: Vec<String>,
    pub user: String,
    pub lock: DirectorLock,
}

#[derive(Debug, Default)]
pub struct ContinueFreezeMap {
    inner: Mutex<HashMap<String, FrozenContinueShot>>,
}

impl ContinueFreezeMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// 第一次 pin 赢。之后同一 `run_id` 忽略新快照。
    pub fn pin(&self, run_id: &str, shot: FrozenContinueShot) -> FrozenContinueShot {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.entry(run_id.to_string()).or_insert(shot).clone()
    }

    pub fn thaw(&self, run_id: &str) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.remove(run_id);
    }
}

/// `write_beat_once` 返回（含提前 Err）后解冻，避免批量续写下章复用上一拍。
pub struct ThawGuard {
    map: Arc<ContinueFreezeMap>,
    run_id: String,
}

impl ThawGuard {
    pub fn new(map: Arc<ContinueFreezeMap>, run_id: &str) -> Self {
        Self {
            map,
            run_id: run_id.to_string(),
        }
    }
}

impl Drop for ThawGuard {
    fn drop(&mut self) {
        self.map.thaw(&self.run_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::{
        beat_card::{ConflictMove, EmotionBeat, SceneBeatCard},
        continue_director::{IdentityLock, LifeStatus},
    };

    fn empty_card(dead: &[&str]) -> SceneBeatCard {
        SceneBeatCard {
            cast: vec![],
            conflict_move: ConflictMove {
                action: String::new(),
                parties: vec![],
                stage: crate::agency::beat_card::default_conflict_stage(),
            },
            emotion_beat: EmotionBeat {
                summary: String::new(),
            },
            next_outline_node: String::new(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: None,
            open_review_issues: vec![],
            dead: dead.iter().map(|s| (*s).to_string()).collect(),
            change_delta: crate::agency::beat_card::ChangeDelta::default(),
        }
    }

    fn shot(user: &str, dead: &[&str]) -> FrozenContinueShot {
        FrozenContinueShot {
            card: empty_card(dead),
            admitted: vec!["苏会山".into()],
            l2: vec!["苏会山".into()],
            user: user.to_string(),
            lock: DirectorLock::default(),
        }
    }

    #[test]
    fn pin_keeps_director_lock() {
        let map = ContinueFreezeMap::new();
        let mut first = shot("死人退场", &["苏会山"]);
        first.lock = DirectorLock {
            identities: vec![IdentityLock {
                canonical: "曹元佩".into(),
                aliases: vec!["琬公主曹元佩".into()],
                status: LifeStatus::Living,
                kin: Some("苏会山之子".into()),
            }],
            beat_move: "写后果".into(),
            forbidden: vec!["禁止拆成两人".into()],
            relations: vec!["苏会山 — 苏亦铁：父子。禁止写成叔侄。".into()],
            relations_dropped: vec![],
        };
        map.pin("run-lock", first.clone());
        let second = shot("金敏秀走进大堂", &[]);
        let got = map.pin("run-lock", second);
        assert_eq!(got.lock.identities.len(), 1);
        assert_eq!(got.lock.identities[0].canonical, "曹元佩");
        assert!(got.lock.relations.iter().any(|r| r.contains("父子")));
        assert_eq!(got.lock.beat_move, "写后果");
    }

    #[test]
    fn pin_keeps_first_shot_when_later_cast_changes() {
        let map = ContinueFreezeMap::new();
        let first = shot("死人退场，写后果。苏会山已死。", &["苏会山"]);
        map.pin("run-1", first.clone());
        let second = shot("金敏秀走进大堂再刺一刀。", &[]);
        let got = map.pin("run-1", second);
        assert!(got.user.contains("苏会山已死"));
        assert!(!got.user.contains("金敏秀"));
        assert_eq!(got.card.dead, vec!["苏会山".to_string()]);
        assert_eq!(got.admitted, first.admitted);
    }

    #[test]
    fn thaw_allows_new_shot_for_next_beat() {
        let map = ContinueFreezeMap::new();
        map.pin("run-1", shot("第一拍", &["苏会山"]));
        map.thaw("run-1");
        let got = map.pin("run-1", shot("第二拍金敏秀不得进冻结件", &[]));
        assert!(got.user.contains("第二拍"));
        assert!(got.card.dead.is_empty());
    }

    #[test]
    fn thaw_guard_drops_freeze() {
        let map = Arc::new(ContinueFreezeMap::new());
        {
            let _g = ThawGuard::new(map.clone(), "run-1");
            map.pin("run-1", shot("冻结中", &["苏会山"]));
            let still = map.pin("run-1", shot("应被忽略", &[]));
            assert!(still.user.contains("冻结中"));
        }
        let after = map.pin("run-1", shot("解冻后", &[]));
        assert!(after.user.contains("解冻后"));
    }
}
