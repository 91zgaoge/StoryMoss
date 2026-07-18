use tauri::{AppHandle, Emitter};

use crate::agency::models::*;
use crate::agency::repository::AgencyRepository;
use crate::db::DbPool;
use crate::error::AppError;

pub const EVENT_BOARD_CHANGED: &str = "agency-board-changed";

#[derive(Debug, Clone, serde::Serialize)]
pub struct BoardSnapshot {
    pub assets: Vec<BoardItem>,
    pub drafts: Vec<BoardItem>,
    pub reviews: Vec<BoardItem>,
    pub schedules: Vec<BoardItem>,
}

impl BoardSnapshot {
    /// 三档压缩的第一档：目录（key + summary + version），带字符预算硬截断。
    /// 对应 ECC agent-compress 的 catalog 档位。
    pub fn to_catalog(&self, max_chars: usize) -> String {
        let mut out = String::new();
        let groups: [(&str, &Vec<BoardItem>); 4] = [
            ("asset", &self.assets),
            ("draft", &self.drafts),
            ("review", &self.reviews),
            ("schedule", &self.schedules),
        ];
        for (zone, items) in groups {
            for item in items {
                let line = format!(
                    "- [{}/{}] {} (v{}, {})\n",
                    zone, item.key, item.summary, item.version, item.status
                );
                if out.chars().count() + line.chars().count() > max_chars {
                    out.push_str("... (更多条目按需用 board_read 取全文)\n");
                    return out;
                }
                out.push_str(&line);
            }
        }
        out
    }

    /// 目录的 token 预算版：逐条累加并用 tokenizer 计量，超预算即截断并附取全文提示。
    /// 对应 ECC agent-compress 的 catalog 档位（token 版优先于字符版）。
    pub fn to_catalog_tokens(&self, max_tokens: usize, model_family: &str) -> String {
        let mut out = String::new();
        let groups: [(&str, &Vec<BoardItem>); 4] = [
            ("asset", &self.assets), ("draft", &self.drafts),
            ("review", &self.reviews), ("schedule", &self.schedules),
        ];
        let trailer = "... (更多条目按需用 board_read 取全文)\n";
        for (zone, items) in groups {
            for item in items {
                let line = format!("- [{}/{}] {} (v{}, {})\n",
                    zone, item.key, item.summary, item.version, item.status);
                let candidate = format!("{}{}", out, line);
                if crate::memory::tokenizer::count_tokens(&candidate, model_family) > max_tokens {
                    out.push_str(trailer);
                    return out;
                }
                out = candidate;
            }
        }
        out
    }
}

#[derive(Clone)]
pub struct BlackboardService {
    repo: AgencyRepository,
    app_handle: Option<AppHandle>,
}

impl BlackboardService {
    pub fn new(pool: DbPool) -> Self {
        Self { repo: AgencyRepository::new(pool), app_handle: None }
    }

    pub fn with_events(pool: DbPool, app_handle: &AppHandle) -> Self {
        Self { repo: AgencyRepository::new(pool), app_handle: Some(app_handle.clone()) }
    }

    pub fn repo(&self) -> &AgencyRepository {
        &self.repo
    }

    /// 写入黑板：分区 owner 直写 active；非 owner 降级为 proposed（提案）。
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        run_id: &str,
        story_id: &str,
        role: AgentRole,
        zone: BoardZone,
        item_type: &str,
        key: &str,
        content: &str,
        summary: &str,
    ) -> Result<BoardItem, AppError> {
        let status = if zone.owner() == role { "active" } else { "proposed" };
        let item = BoardItem::new(run_id, story_id, zone, item_type, key, content, summary, role, status);
        self.repo.insert_item(&item).map_err(AppError::from)?;
        self.emit_changed(&item);
        Ok(item)
    }

    /// 修订：仅分区 owner 可修订；版本乐观锁。
    pub fn revise(
        &self,
        item_id: &str,
        role: AgentRole,
        new_content: &str,
        new_summary: &str,
        expected_version: i32,
    ) -> Result<BoardItem, AppError> {
        let item = self.repo.get_item(item_id).map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed(format!("黑板条目不存在: {}", item_id), None::<String>))?;
        if item.zone.owner() != role {
            return Err(AppError::validation_failed(format!(
                "角色 {} 无权修订 {} 区条目（owner: {}）",
                role.as_str(), item.zone.as_str(), item.zone.owner().as_str()
            ), None::<String>));
        }
        let revised = self.repo.revise_item(item_id, new_content, new_summary, expected_version)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed(format!(
                "版本冲突: 条目 {} 当前版本已不是 v{}", item_id, expected_version
            ), None::<String>))?;
        self.emit_changed(&revised);
        Ok(revised)
    }

    /// 提案晋升为正式（协调器仲裁用）。
    pub fn promote(&self, item_id: &str) -> Result<(), AppError> {
        self.repo.promote_item(item_id).map_err(AppError::from)?;
        if let Ok(Some(item)) = self.repo.get_item(item_id) {
            self.emit_changed(&item);
        }
        Ok(())
    }

    pub fn snapshot(&self, run_id: &str) -> Result<BoardSnapshot, AppError> {
        let items = self.repo.list_items(run_id, None).map_err(AppError::from)?;
        let mut snap = BoardSnapshot { assets: vec![], drafts: vec![], reviews: vec![], schedules: vec![] };
        for item in items {
            match item.zone {
                BoardZone::Asset => snap.assets.push(item),
                BoardZone::Draft => snap.drafts.push(item),
                BoardZone::Review => snap.reviews.push(item),
                BoardZone::Schedule => snap.schedules.push(item),
            }
        }
        Ok(snap)
    }

    pub fn list_zone(&self, run_id: &str, zone: BoardZone) -> Result<Vec<BoardItem>, AppError> {
        self.repo.list_items(run_id, Some(zone)).map_err(AppError::from)
    }

    /// 按可选分区过滤列出条目（None = 全部），供 board_read 工具的 key 精确查找使用。
    pub fn list_zone_filtered(&self, run_id: &str, zone: Option<BoardZone>) -> Result<Vec<BoardItem>, AppError> {
        self.repo.list_items(run_id, zone).map_err(AppError::from)
    }

    fn emit_changed(&self, item: &BoardItem) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(EVENT_BOARD_CHANGED, item.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn board() -> BlackboardService {
        BlackboardService::new(create_test_pool().unwrap())
    }

    fn seed_run(svc: &BlackboardService, run_id: &str) {
        svc.repo().create_run(&AgencyRun::new(run_id, "前提")).unwrap();
    }

    #[test]
    fn test_owner_writes_active_non_owner_proposed() {
        let svc = board();
        seed_run(&svc, "r1");
        // Producer 是 Asset 区 owner → active
        let a = svc.write("r1", "s1", AgentRole::Producer, BoardZone::Asset,
            "world", "世界观", "双星废土", "双星废土").unwrap();
        assert_eq!(a.status, "active");
        // LeadWriter 写 Asset 区 → 降级为 proposed
        let p = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Asset,
            "world", "世界观补充", "浮空城", "浮空城").unwrap();
        assert_eq!(p.status, "proposed");
        // EditorAuditor 写 Draft 区 → proposed
        let d = svc.write("r1", "s1", AgentRole::EditorAuditor, BoardZone::Draft,
            "chapter", "第一章", "编辑代拟", "代拟").unwrap();
        assert_eq!(d.status, "proposed");
    }

    #[test]
    fn test_revise_enforces_ownership() {
        let svc = board();
        seed_run(&svc, "r1");
        let draft = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Draft,
            "chapter", "第一章", "初稿", "初稿").unwrap();
        // 非 owner 修订 → 报错
        let err = svc.revise(&draft.id, AgentRole::Producer, "篡改", "x", 1).unwrap_err();
        assert!(err.message().contains("无权"));
        // owner 修订 → 成功
        let ok = svc.revise(&draft.id, AgentRole::LeadWriter, "二稿", "二稿", 1).unwrap();
        assert_eq!(ok.version, 2);
        // 版本冲突 → 报错
        let conflict = svc.revise(&draft.id, AgentRole::LeadWriter, "三稿", "x", 1).unwrap_err();
        assert!(conflict.message().contains("版本冲突"));
    }

    #[test]
    fn test_snapshot_catalog_respects_budget() {
        let svc = board();
        seed_run(&svc, "r1");
        for i in 0..10 {
            svc.write("r1", "s1", AgentRole::Producer, BoardZone::Asset,
                "world", &format!("设定{}", i), "x", &format!("第{}条设定的摘要，内容比较长需要截断", i)).unwrap();
        }
        let snap = svc.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 10);
        let catalog = snap.to_catalog(200);
        assert!(catalog.chars().count() <= 260, "目录应接近预算上限: {}", catalog.len());
        assert!(catalog.contains("asset/"));
    }

    #[test]
    fn test_catalog_tokens_budget() {
        let svc = board();
        seed_run(&svc, "r1");
        for i in 0..20 {
            svc.write("r1", "s1", AgentRole::Producer, BoardZone::Asset,
                "world", &format!("设定{}", i), "x", &format!("第{}条设定摘要，这是一段用于消耗 token 的较长文本", i)).unwrap();
        }
        let snap = svc.snapshot("r1").unwrap();
        let catalog = snap.to_catalog_tokens(50, "cl100k");
        assert!(crate::memory::tokenizer::count_tokens(&catalog, "cl100k") <= 80,
            "目录应接近 token 预算（含截断标记）: {}", catalog.len());
        assert!(catalog.contains("asset/"));
        let full = snap.to_catalog_tokens(100_000, "cl100k");
        assert!(full.contains("设定19"));
    }

    #[test]
    fn test_promote() {
        let svc = board();
        seed_run(&svc, "r1");
        let p = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Asset,
            "world", "提案", "x", "提案").unwrap();
        assert_eq!(p.status, "proposed");
        svc.promote(&p.id).unwrap();
        let snap = svc.snapshot("r1").unwrap();
        assert_eq!(snap.assets[0].status, "active");
    }

    #[test]
    fn test_promote_emits_no_panic_without_handle() {
        // 无 app_handle 时 promote 不 panic（事件 best-effort）
        let svc = board();
        seed_run(&svc, "r1");
        let p = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Asset,
            "world", "提案", "x", "提案").unwrap();
        svc.promote(&p.id).unwrap();
        assert_eq!(svc.snapshot("r1").unwrap().assets[0].status, "active");
    }
}
