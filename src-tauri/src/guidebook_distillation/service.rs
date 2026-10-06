//! 指导书提炼 Service：上传校验 → 去重 → 解析 → 任务系统执行提炼。

use std::{path::Path, sync::Arc};

use chrono::Local;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::{
    distiller::GuidebookDistiller,
    models::*,
    repository::{CustomMethodologyRepository, GuidebookRepository},
};
use crate::{
    book_deconstruction::{
        chunker::create_chunks,
        models::{AnalysisError, ParseError, TextChunk},
        parser::parse_book,
    },
    db::DbPool,
    error::AppError,
    llm::LlmService,
    task_system::{models::CreateTaskRequest, service::TaskService},
};

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB

pub struct GuidebookDistillationService {
    pool: DbPool,
    llm_service: LlmService,
    app_handle: AppHandle,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuidebookStatusResponse {
    pub guidebook_id: String,
    pub status: String,
    pub progress: i32,
    pub current_step: Option<String>,
    pub error: Option<String>,
}

impl GuidebookDistillationService {
    pub fn new(pool: DbPool, llm_service: LlmService, app_handle: AppHandle) -> Self {
        Self {
            pool,
            llm_service,
            app_handle,
        }
    }

    // ==================== 上传并提炼 ====================

    pub async fn upload_and_distill(
        &self,
        file_path: &Path,
        merge_into: Option<&str>,
    ) -> Result<String, ParseError> {
        self.validate_file(file_path)?;
        // fold-in：合并目标必须是存在的自定义方法论
        if let Some(target) = merge_into {
            if !crate::domain::methodology::is_custom_methodology_id(target) {
                return Err(ParseError::InvalidFormat(
                    "合并目标必须是自定义方法论（custom_ 前缀）".to_string(),
                ));
            }
            let exists = CustomMethodologyRepository::new(self.pool.clone())
                .get_by_id(target)
                .ok()
                .flatten()
                .is_some();
            if !exists {
                return Err(ParseError::StorageError(format!(
                    "合并目标方法论 {} 不存在",
                    target
                )));
            }
        }
        let file_hash = self.compute_file_hash(file_path).await?;

        let repo = GuidebookRepository::new(self.pool.clone());
        // 先取出 id 与去重决策，避免把非 Send 的 RepoResult 临时量带过 await
        let dedup = match repo.get_by_hash(&file_hash) {
            Ok(Some(existing)) => Some((
                existing.id,
                existing.merge_into_methodology_id,
                dedup_decision(&existing.status),
            )),
            _ => None,
        };
        if let Some((existing_id, existing_merge, decision)) = dedup {
            // 重复文件带合并目标且旧记录未落合并意图时回写，
            // 避免合并意图静默丢失
            let backfill_merge_intent = || {
                if let Some(target) = merge_into {
                    if existing_merge.is_none() {
                        if let Err(e) = repo.set_merge_into(&existing_id, target) {
                            log::warn!(
                                "[GuidebookDistillation] 回写合并意图失败 {}: {}",
                                existing_id,
                                e
                            );
                        }
                    }
                }
            };
            match decision {
                DedupDecision::ReturnExisting => {
                    log::info!(
                        "[GuidebookDistillation] File already exists: {}",
                        existing_id
                    );
                    backfill_merge_intent();
                    return Ok(existing_id);
                }
                DedupDecision::RetryExisting => {
                    // 失败/已取消的记录：复用已存文件重新提炼，不重复落文件
                    log::info!(
                        "[GuidebookDistillation] Retrying failed/cancelled distillation: {}",
                        existing_id
                    );
                    // 回写必须先于重试调度：retry_distillation 的 fallback 会
                    // 立即 spawn run_distillation，后者开头即从 DB 读
                    // merge_into_methodology_id，先调度再回写存在竞态丢失。
                    // reset_for_retry 只清 status/progress/error，不动
                    // merge_into， 提前回写安全。
                    backfill_merge_intent();
                    self.retry_distillation(&existing_id).await?;
                    return Ok(existing_id);
                }
            }
        }

        let guidebook_id = Uuid::new_v4().to_string();
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let dir = app_dir.join("guidebooks");
        std::fs::create_dir_all(&dir)
            .map_err(|e| ParseError::IoError(format!("Failed to create guidebooks dir: {}", e)))?;
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_lowercase();
        let dest_path = dir.join(format!("{}.{}", guidebook_id, ext));
        tokio::fs::copy(file_path, &dest_path)
            .await
            .map_err(|e| ParseError::IoError(format!("Failed to copy file: {}", e)))?;

        let parsed = parse_book(&dest_path, None)?;

        let now = Local::now();
        let book = Guidebook {
            id: guidebook_id.clone(),
            title: parsed.title.clone().unwrap_or_else(|| "未命名".to_string()),
            author: parsed.author.clone(),
            subject: None,
            word_count: Some(parsed.word_count as i64),
            file_format: Some(ext),
            file_hash: Some(file_hash),
            file_path: Some(dest_path.to_string_lossy().to_string()),
            methodology_id: None,
            status: DistillationStatus::Pending,
            progress: 0,
            error: None,
            task_id: None,
            merge_into_methodology_id: merge_into.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };
        repo.create(&book)
            .map_err(|e| ParseError::StorageError(format!("Failed to create record: {}", e)))?;

        let payload = serde_json::json!({
            "guidebook_id": guidebook_id,
            "file_path": dest_path.to_string_lossy().to_string(),
        })
        .to_string();
        let task_req = CreateTaskRequest {
            name: format!(
                "指导书提炼: {}",
                parsed.title.clone().unwrap_or_else(|| "未命名".to_string())
            ),
            description: Some(format!("提炼 {} 字的指导书", parsed.word_count)),
            task_type: "guidebook_distillation".to_string(),
            schedule_type: "once".to_string(),
            cron_pattern: None,
            payload: Some(payload),
            enabled: Some(true),
            max_retries: Some(3),
            // 心跳：600s 无进度则 HeartbeatMonitor 杀僵死任务；墙钟由
            // TaskService 对 guidebook_distillation 放宽到 12h（同拆书）。
            heartbeat_timeout_seconds: Some(600),
        };

        let task_service = self.app_handle.state::<TaskService>();
        match task_service.create_task(task_req) {
            Ok(task) => {
                let _ = repo.update_task_id(&guidebook_id, &task.id);
                let _ = repo.update_status(&guidebook_id, DistillationStatus::Pending, 0);
            }
            Err(e) => {
                log::error!(
                    "[GuidebookDistillation] 任务创建失败，回退直接后台提炼: {}",
                    e
                );
                let pool = self.pool.clone();
                let llm_service = self.llm_service.clone();
                let app_handle = self.app_handle.clone();
                let gid = guidebook_id.clone();
                let chunks = create_chunks(&parsed);
                tauri::async_runtime::spawn(async move {
                    let service =
                        GuidebookDistillationService::new(pool.clone(), llm_service, app_handle);
                    if let Err(e) = service.run_distillation(&gid, &chunks, None, None).await {
                        log::error!("[GuidebookDistillation] 回退提炼失败 {}: {}", gid, e);
                        let repo = GuidebookRepository::new(pool.clone());
                        let _ = repo.update_error(&gid, &e.to_string());
                    }
                });
            }
        }

        Ok(guidebook_id)
    }

    /// 执行提炼（任务系统与回退路径共用）
    pub async fn run_distillation(
        &self,
        guidebook_id: &str,
        chunks: &[TextChunk],
        heartbeat: Option<Box<dyn Fn() + Send + Sync>>,
        cancel_check: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    ) -> Result<(), AnalysisError> {
        let repo = GuidebookRepository::new(self.pool.clone());
        repo.update_status(guidebook_id, DistillationStatus::Distilling, 0)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        let concurrency = {
            let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
            crate::config::AppConfig::load(&app_dir)
                .map(|c| c.book_deconstruction_concurrency)
                .unwrap_or(3)
        };

        let distiller = GuidebookDistiller::new(
            self.llm_service.clone(),
            self.app_handle.clone(),
            self.pool.clone(),
            concurrency,
        );

        // fold-in：从 guidebook 记录读合并意图（重试路径同样生效）
        let fold_target = repo
            .get_by_id(guidebook_id)
            .ok()
            .flatten()
            .and_then(|g| g.merge_into_methodology_id);
        let fold_cm = fold_target.as_deref().and_then(|mid| {
            CustomMethodologyRepository::new(self.pool.clone())
                .get_by_id(mid)
                .ok()
                .flatten()
        });
        if fold_target.is_some() && fold_cm.is_none() {
            return Err(AnalysisError::StorageError(
                "合并目标方法论已不存在".to_string(),
            ));
        }

        let output = distiller
            .distill(
                guidebook_id,
                chunks,
                fold_cm.as_ref(),
                heartbeat,
                cancel_check,
            )
            .await?;

        // 落库前确定性清洗（剔除空条目/去重/截断）
        let (output, clean_report) =
            crate::guidebook_distillation::validator::validate_and_clean(output);
        log::info!(
            "[GuidebookDistillation] {} 清洗: 技巧 {}（剔 {} 重 {}）规则 {}（剔 {}）反模式 {}（剔 {}）截断 {} 处",
            guidebook_id,
            output.techniques.len(),
            clean_report.removed_techniques,
            clean_report.deduped_techniques,
            output.cheatsheet.decision_rules.len(),
            clean_report.removed_rules,
            output.cheatsheet.anti_patterns.len(),
            clean_report.removed_anti_patterns,
            clean_report.truncated_fields,
        );

        if let Some(target_cm) = fold_cm {
            // fold-in：更新现有 CM（name/description/enabled 保留），替换
            // steps/patterns/cheatsheet
            let new_steps: Vec<MethodologyStep> = output
                .methodology
                .steps
                .iter()
                .map(|s| MethodologyStep {
                    title: s.title.clone(),
                    instruction: s.instruction.clone(),
                    checklist: s.checklist.clone(),
                })
                .collect();
            let cm_repo = CustomMethodologyRepository::new(self.pool.clone());
            cm_repo
                .update(
                    &target_cm.id,
                    None,
                    None,
                    Some(&new_steps),
                    None,
                    Some(&output.techniques),
                    Some(&output.cheatsheet),
                )
                .map_err(|e| AnalysisError::StorageError(e.to_string()))?;
            // 引用故事的 step 顶格 clamp 到新 max_steps
            let max_steps = (new_steps.len() as i32).max(1);
            clamp_stories_step_in(&self.pool, &target_cm.id, max_steps)
                .map_err(|e| AnalysisError::StorageError(e.to_string()))?;
            repo.update_distilled(
                guidebook_id,
                output.metadata.title.as_deref(),
                output.metadata.author.as_deref(),
                output.metadata.subject.as_deref(),
                &target_cm.id,
            )
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;
            repo.update_status(guidebook_id, DistillationStatus::Completed, 100)
                .map_err(|e| AnalysisError::StorageError(e.to_string()))?;
            log::info!(
                "[GuidebookDistillation] {} fold-in 完成 → 合并进 {}（{}）",
                guidebook_id,
                target_cm.id,
                target_cm.name
            );
            return Ok(());
        }

        // 落库：自定义方法论
        let methodology_id = format!("custom_{}", Uuid::new_v4());
        let now = Local::now();
        let cm = CustomMethodology {
            id: methodology_id.clone(),
            guidebook_id: Some(guidebook_id.to_string()),
            name: output.methodology.name.clone(),
            description: output.methodology.description.clone(),
            steps: output
                .methodology
                .steps
                .iter()
                .map(|s| MethodologyStep {
                    title: s.title.clone(),
                    instruction: s.instruction.clone(),
                    checklist: s.checklist.clone(),
                })
                .collect(),
            patterns: output.techniques.clone(),
            cheatsheet: output.cheatsheet.clone(),
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        CustomMethodologyRepository::new(self.pool.clone())
            .create(&cm)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        repo.update_distilled(
            guidebook_id,
            output.metadata.title.as_deref(),
            output.metadata.author.as_deref(),
            output.metadata.subject.as_deref(),
            &methodology_id,
        )
        .map_err(|e| AnalysisError::StorageError(e.to_string()))?;
        repo.update_status(guidebook_id, DistillationStatus::Completed, 100)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;
        log::info!(
            "[GuidebookDistillation] {} 提炼完成 → 方法论 {}（{}）",
            guidebook_id,
            methodology_id,
            cm.name
        );
        Ok(())
    }

    // ==================== 查询与管理 ====================

    pub fn get_status(&self, guidebook_id: &str) -> Result<GuidebookStatusResponse, AppError> {
        let repo = GuidebookRepository::new(self.pool.clone());
        let book = repo
            .get_by_id(guidebook_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("guidebook", guidebook_id))?;
        Ok(GuidebookStatusResponse {
            guidebook_id: book.id,
            status: book.status.to_string(),
            progress: book.progress,
            current_step: None,
            error: book.error,
        })
    }

    pub fn get_result(&self, guidebook_id: &str) -> Result<GuidebookResult, AppError> {
        let repo = GuidebookRepository::new(self.pool.clone());
        let book = repo
            .get_by_id(guidebook_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("guidebook", guidebook_id))?;
        let methodology = book.methodology_id.as_deref().and_then(|mid| {
            CustomMethodologyRepository::new(self.pool.clone())
                .get_by_id(mid)
                .ok()
                .flatten()
        });
        Ok(GuidebookResult {
            guidebook: book,
            methodology,
        })
    }

    pub fn list_guidebooks(&self) -> Result<Vec<GuidebookListItem>, AppError> {
        GuidebookRepository::new(self.pool.clone())
            .list_all()
            .map_err(AppError::from)
    }

    pub fn delete_guidebook(&self, guidebook_id: &str) -> Result<(), AppError> {
        let repo = GuidebookRepository::new(self.pool.clone());
        if let Ok(Some(book)) = repo.get_by_id(guidebook_id) {
            if let Some(path) = book.file_path {
                let _ = std::fs::remove_file(path);
            }
        }
        repo.delete(guidebook_id).map_err(AppError::from)
    }

    /// 取消：置 Cancelled；任务系统侧取消照搬
    /// BookDeconstructionService::cancel_analysis 的做法（有 task_id 则
    /// 通过 TaskService 取消任务，executor 的 cancel_check 负责中断 LLM）。
    pub fn cancel_distillation(&self, guidebook_id: &str) -> Result<(), AppError> {
        let repo = GuidebookRepository::new(self.pool.clone());
        if let Ok(Some(book)) = repo.get_by_id(guidebook_id) {
            if let Some(task_id) = book.task_id {
                let task_service = self.app_handle.state::<TaskService>();
                if let Err(e) = task_service.cancel_task(&task_id) {
                    log::warn!(
                        "[GuidebookDistillation] Failed to cancel task {}: {}",
                        task_id,
                        e
                    );
                }
            }
        }
        repo.update_status(guidebook_id, DistillationStatus::Cancelled, 0)
            .map_err(AppError::from)
    }

    /// 重试提炼：复用已存文件重建任务（仅 failed/cancelled 可重试）。
    /// 与 upload_and_distill 的任务创建/回退路径同款。
    pub async fn retry_distillation(&self, guidebook_id: &str) -> Result<(), ParseError> {
        let repo = GuidebookRepository::new(self.pool.clone());
        let book = repo
            .get_by_id(guidebook_id)
            .map_err(|e| ParseError::StorageError(format!("查询指导书失败: {}", e)))?
            .ok_or_else(|| ParseError::StorageError("指导书不存在".to_string()))?;
        match book.status {
            DistillationStatus::Failed | DistillationStatus::Cancelled => {}
            _ => {
                return Err(ParseError::StorageError(
                    "仅失败或已取消的指导书可重试".to_string(),
                ))
            }
        }
        let file_path = book.file_path.clone().ok_or_else(|| {
            ParseError::StorageError("原始文件记录缺失，请删除后重新上传".to_string())
        })?;
        let path = std::path::PathBuf::from(&file_path);
        if !path.exists() {
            return Err(ParseError::IoError(
                "原始文件已丢失，请删除后重新上传".to_string(),
            ));
        }
        let parsed = parse_book(&path, None)?;
        repo.reset_for_retry(guidebook_id)
            .map_err(|e| ParseError::StorageError(format!("重置状态失败: {}", e)))?;

        let payload = serde_json::json!({
            "guidebook_id": guidebook_id,
            "file_path": file_path,
        })
        .to_string();
        let task_req = CreateTaskRequest {
            name: format!("指导书提炼（重试）: {}", book.title),
            description: Some(format!("重试提炼 {} 字的指导书", parsed.word_count)),
            task_type: "guidebook_distillation".to_string(),
            schedule_type: "once".to_string(),
            cron_pattern: None,
            payload: Some(payload),
            enabled: Some(true),
            max_retries: Some(3),
            heartbeat_timeout_seconds: Some(600),
        };

        let task_service = self.app_handle.state::<TaskService>();
        match task_service.create_task(task_req) {
            Ok(task) => {
                let _ = repo.update_task_id(guidebook_id, &task.id);
                let _ = repo.update_status(guidebook_id, DistillationStatus::Pending, 0);
            }
            Err(e) => {
                log::error!(
                    "[GuidebookDistillation] 重试任务创建失败，回退直接后台提炼: {}",
                    e
                );
                let pool = self.pool.clone();
                let llm_service = self.llm_service.clone();
                let app_handle = self.app_handle.clone();
                let gid = guidebook_id.to_string();
                let chunks = create_chunks(&parsed);
                tauri::async_runtime::spawn(async move {
                    let service =
                        GuidebookDistillationService::new(pool.clone(), llm_service, app_handle);
                    if let Err(e) = service.run_distillation(&gid, &chunks, None, None).await {
                        log::error!("[GuidebookDistillation] 回退重试提炼失败 {}: {}", gid, e);
                        let repo = GuidebookRepository::new(pool.clone());
                        let _ = repo.update_error(&gid, &e.to_string());
                    }
                });
            }
        }
        Ok(())
    }

    // ==================== 文件工具（与拆书同款规则） ====================

    fn validate_file(&self, file_path: &Path) -> Result<(), ParseError> {
        if !file_path.exists() {
            return Err(ParseError::IoError("文件不存在".to_string()));
        }
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !["txt", "pdf", "epub"].contains(&ext.as_str()) {
            return Err(ParseError::InvalidFormat(format!(
                "不支持的格式 .{}，仅支持 txt/pdf/epub",
                ext
            )));
        }
        let size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
        if size > MAX_FILE_SIZE {
            return Err(ParseError::FileTooLarge(format!(
                "文件大小 {}MB 超过 100MB 上限",
                size / 1024 / 1024
            )));
        }
        Ok(())
    }

    async fn compute_file_hash(&self, file_path: &Path) -> Result<String, ParseError> {
        let content = tokio::fs::read(file_path)
            .await
            .map_err(|e| ParseError::IoError(format!("读取文件失败: {}", e)))?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(format!("{:x}", hasher.finalize()))
    }
}

/// 续写注入点用：渲染自定义方法论当前步骤的约束文本 + 技巧参考 + 决策速查。
/// 未知 id / 已禁用 / 无步骤 → None（调用方静默跳过注入）。
pub fn render_custom_methodology_extension(
    pool: &DbPool,
    methodology_id: &str,
    step: i32,
) -> Option<String> {
    let cm = CustomMethodologyRepository::new(pool.clone())
        .get_by_id(methodology_id)
        .ok()
        .flatten()?;
    if !cm.enabled || cm.steps.is_empty() {
        return None;
    }
    let idx = ((step.max(1) as usize) - 1).min(cm.steps.len() - 1);
    let s = &cm.steps[idx];
    let checklist = if s.checklist.is_empty() {
        String::new()
    } else {
        format!(
            "\n检查清单：\n{}",
            s.checklist
                .iter()
                .map(|c| format!("- {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let mut out = format!(
        "【创作方法论（{}·第{}步：{}）】\n{}{}",
        cm.name,
        idx + 1,
        s.title,
        s.instruction,
        checklist
    );

    // 技巧参考：按步骤轮转取最多 3 条（确定性、零额外 LLM 调用）
    if !cm.patterns.is_empty() {
        let start = idx % cm.patterns.len();
        let n = 3.min(cm.patterns.len());
        let lines = (0..n)
            .map(|k| {
                let t = &cm.patterns[(start + k) % cm.patterns.len()];
                format!(
                    "- {}：{}→{}",
                    clip_chars(&t.name, 40),
                    clip_chars(&t.when_to_use, 80),
                    clip_chars(&t.how, 160)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str(&format!("\n\n【技巧参考】\n{}", lines));
    }

    // 决策速查：最多 3 条规则 + 1 条反模式
    if !cm.cheatsheet.decision_rules.is_empty() || !cm.cheatsheet.anti_patterns.is_empty() {
        let mut lines = Vec::new();
        for r in cm.cheatsheet.decision_rules.iter().take(3) {
            lines.push(format!("- {}", clip_chars(r, 120)));
        }
        if let Some(ap) = cm.cheatsheet.anti_patterns.first() {
            lines.push(format!(
                "- 避免：{}（{}）",
                clip_chars(&ap.what, 60),
                clip_chars(&ap.why, 80)
            ));
        }
        out.push_str(&format!("\n\n【决策速查】\n{}", lines.join("\n")));
    }

    Some(out)
}

/// 截断到 max 字（chars），防爆 token
fn clip_chars(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() > max {
        t.chars().take(max).collect()
    } else {
        t.to_string()
    }
}

/// hash 去重决策：失败/已取消的记录走重试，其余直接返回旧 id
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DedupDecision {
    ReturnExisting,
    RetryExisting,
}

pub(crate) fn dedup_decision(status: &DistillationStatus) -> DedupDecision {
    match status {
        DistillationStatus::Failed | DistillationStatus::Cancelled => DedupDecision::RetryExisting,
        _ => DedupDecision::ReturnExisting,
    }
}

/// 引用某方法论的故事 methodology_step 顶格 clamp（fold-in 后 steps
/// 数可能变少）
pub(crate) fn clamp_stories_step_in(
    pool: &DbPool,
    methodology_id: &str,
    max_steps: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE stories SET methodology_step = ?1 \
         WHERE methodology_id = ?2 AND methodology_step > ?1",
        rusqlite::params![max_steps, methodology_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod extension_tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_cm(pool: &DbPool, enabled: bool) {
        CustomMethodologyRepository::new(pool.clone())
            .create(&CustomMethodology {
                id: "custom_t1".into(),
                guidebook_id: None,
                name: "冲突驱动法".into(),
                description: None,
                steps: vec![
                    MethodologyStep {
                        title: "立冲突".into(),
                        instruction: "确立核心冲突".into(),
                        checklist: vec!["冲突明确吗？".into()],
                    },
                    MethodologyStep {
                        title: "升级".into(),
                        instruction: "升级冲突".into(),
                        checklist: vec![],
                    },
                ],
                patterns: vec![],
                cheatsheet: Cheatsheet::default(),
                enabled,
                created_at: chrono::Local::now(),
                updated_at: chrono::Local::now(),
            })
            .unwrap();
    }

    fn seed_cm_with_assets(pool: &DbPool) {
        CustomMethodologyRepository::new(pool.clone())
            .create(&CustomMethodology {
                id: "custom_a1".into(),
                guidebook_id: None,
                name: "冲突驱动法".into(),
                description: None,
                steps: vec![
                    MethodologyStep {
                        title: "立冲突".into(),
                        instruction: "确立核心冲突".into(),
                        checklist: vec![],
                    },
                    MethodologyStep {
                        title: "升级".into(),
                        instruction: "升级冲突".into(),
                        checklist: vec![],
                    },
                ],
                patterns: vec![
                    Technique {
                        name: "场景目标法".into(),
                        when_to_use: "每场开场".into(),
                        how: "给 POV 角色一个当场可达成的具体目标".into(),
                    },
                    Technique {
                        name: "灾难收尾".into(),
                        when_to_use: "场景结尾".into(),
                        how: "让目标受挫并引出新难题".into(),
                    },
                    Technique {
                        name: "情感节拍".into(),
                        when_to_use: "反应段".into(),
                        how: "先情感后理性再行动".into(),
                    },
                    Technique {
                        name: "第四技巧".into(),
                        when_to_use: "w4".into(),
                        how: "h4".into(),
                    },
                ],
                cheatsheet: Cheatsheet {
                    decision_rules: vec![
                        "当节奏拖沓时删场景，因为每场景必须推进冲突".into(),
                        "当人物扁平时加矛盾欲望，因为冲突来自内心".into(),
                    ],
                    anti_patterns: vec![AntiPattern {
                        what: "信息倾倒".into(),
                        why: "读者失去探索欲".into(),
                    }],
                },
                enabled: true,
                created_at: chrono::Local::now(),
                updated_at: chrono::Local::now(),
            })
            .unwrap();
    }

    #[test]
    fn clamp_stories_step_caps_to_new_max() {
        use crate::db::{dto::CreateStoryRequest, repositories::StoryRepository};
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "clamp 测试".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE stories SET methodology_id = 'custom_clamp', methodology_step = 5 WHERE id = ?1",
                rusqlite::params![story.id],
            )
            .unwrap();
        }
        clamp_stories_step_in(&pool, "custom_clamp", 3).unwrap();
        let conn = pool.get().unwrap();
        let step: i32 = conn
            .query_row(
                "SELECT methodology_step FROM stories WHERE id = ?1",
                rusqlite::params![story.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(step, 3);
    }

    #[test]
    fn dedup_decision_only_failed_and_cancelled_retry() {
        assert!(matches!(
            dedup_decision(&DistillationStatus::Failed),
            DedupDecision::RetryExisting
        ));
        assert!(matches!(
            dedup_decision(&DistillationStatus::Cancelled),
            DedupDecision::RetryExisting
        ));
        for s in [
            DistillationStatus::Pending,
            DistillationStatus::Extracting,
            DistillationStatus::Distilling,
            DistillationStatus::Merging,
            DistillationStatus::Completed,
        ] {
            assert!(matches!(dedup_decision(&s), DedupDecision::ReturnExisting));
        }
    }

    #[test]
    fn render_extension_includes_techniques_and_cheatsheet() {
        let pool = create_test_pool().unwrap();
        seed_cm_with_assets(&pool);
        let text = render_custom_methodology_extension(&pool, "custom_a1", 1).unwrap();
        assert!(text.contains("【技巧参考】"));
        assert!(text.contains("场景目标法"));
        assert!(text.contains("给 POV 角色一个当场可达成的具体目标"));
        assert!(text.contains("【决策速查】"));
        assert!(text.contains("当节奏拖沓时删场景"));
        assert!(text.contains("避免：信息倾倒"));
    }

    #[test]
    fn render_extension_rotates_techniques_by_step() {
        let pool = create_test_pool().unwrap();
        seed_cm_with_assets(&pool);
        // step 1（idx 0）：从 patterns[0] 起取 3 条
        let s1 = render_custom_methodology_extension(&pool, "custom_a1", 1).unwrap();
        assert!(s1.contains("场景目标法"));
        assert!(s1.contains("灾难收尾"));
        assert!(s1.contains("情感节拍"));
        assert!(!s1.contains("第四技巧"));
        // step 2（idx 1）：轮转从 patterns[1] 起
        let s2 = render_custom_methodology_extension(&pool, "custom_a1", 2).unwrap();
        assert!(s2.contains("灾难收尾"));
        assert!(s2.contains("情感节拍"));
        assert!(s2.contains("第四技巧"));
        assert!(!s2.contains("场景目标法"));
    }

    #[test]
    fn render_extension_omits_sections_when_no_assets() {
        let pool = create_test_pool().unwrap();
        seed_cm(&pool, true); // 无 patterns/cheatsheet 的旧数据
        let text = render_custom_methodology_extension(&pool, "custom_t1", 1).unwrap();
        assert!(!text.contains("【技巧参考】"));
        assert!(!text.contains("【决策速查】"));
    }

    #[test]
    fn render_extension_picks_step_and_formats() {
        let pool = create_test_pool().unwrap();
        seed_cm(&pool, true);
        let text = render_custom_methodology_extension(&pool, "custom_t1", 2).unwrap();
        assert!(text.contains("冲突驱动法"));
        assert!(text.contains("第2步：升级"));
        assert!(text.contains("升级冲突"));
        // 越界 step 钳到最后一步
        let clamped = render_custom_methodology_extension(&pool, "custom_t1", 99).unwrap();
        assert!(clamped.contains("第2步"));
        // 第 1 步带检查清单
        let step1 = render_custom_methodology_extension(&pool, "custom_t1", 1).unwrap();
        assert!(step1.contains("检查清单"));
        assert!(step1.contains("冲突明确吗？"));
    }

    #[test]
    fn render_extension_none_when_disabled_or_unknown() {
        let pool = create_test_pool().unwrap();
        seed_cm(&pool, false);
        assert!(render_custom_methodology_extension(&pool, "custom_t1", 1).is_none());
        assert!(render_custom_methodology_extension(&pool, "custom_unknown", 1).is_none());
    }
}
