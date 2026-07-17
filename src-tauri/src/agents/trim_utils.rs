//! 自重复裁剪工具函数 — 迁自旧 narrative/genesis 模块（v0.26.19 Phase 3.1）。
//!
//! 旧创世管线 已删除，但这 3 个纯函数仍被
//! `agents::orchestrator` 的 execute_trishot 续写路径复用（8% 自重复重试闸门），
//! 故原样迁至本模块，行为与可见性（`pub(crate)`）保持不变。

/// v0.26.19 Phase 3.1: 计算自重复裁剪比例（纯函数，供测试编码 8% 闸门契约）。
///
/// `trim_ratio = 1 - cleaned/raw`；raw 为空时返回 0.0 避免除零。
/// 此函数把 `FirstChapterGenerationStep::execute` 中内联的比例计算提升为
/// 可独立测试的契约，确保 8% 重试闸门阈值不因实现漂移而失效。
pub(crate) fn compute_trim_ratio(raw_chars: usize, cleaned_chars: usize) -> f32 {
    if raw_chars == 0 {
        return 0.0;
    }
    1.0 - (cleaned_chars as f32 / raw_chars as f32)
}

/// v0.26.19 Phase 3.1: 判定是否需要触发 anti-repeat 重试（纯函数）。
///
/// 契约：仅当 `trim_ratio >= 0.08` **且** `raw_chars > 100` 时触发。
/// - 8% 阈值：低于此值视为首尾呼应等良性结构，不重试（避免误伤）。
/// - 100 字下限：短文本的自重复比例波动大，不触发重试（与
///   `trim_self_repetition` 的 40 字短文本旁路对齐，但此处更保守）。
pub(crate) fn should_retry_self_repetition(trim_ratio: f32, raw_chars: usize) -> bool {
    trim_ratio >= 0.08 && raw_chars > 100
}

/// v0.26.19 Phase 3.1: 选择最终第一章正文（纯函数，编码重试接受/拒绝契约）。
///
/// 重试更干净（`retry_trim_ratio < original_trim_ratio`）则采用重试清理结果；
/// 否则保留首次清理结果。重试 LLM 失败由调用方在 `Err` 分支保留首次清理结果，
/// 此函数仅处理 `Ok` 分支的选择逻辑。
pub(crate) fn select_first_chapter_content(
    original_trim_ratio: f32,
    retry_trim_ratio: f32,
    original_cleaned: String,
    retry_cleaned: String,
) -> String {
    if retry_trim_ratio < original_trim_ratio {
        retry_cleaned
    } else {
        original_cleaned
    }
}

#[cfg(test)]
mod first_chapter_retry_gate_tests {
    use super::*;

    // v0.26.19 Phase 3.1 契约：compute_trim_ratio 在 raw 为空时返回 0.0（不除零），
    //   在 cleaned == raw 时返回 0.0（无裁剪），在 cleaned = raw/2 时返回 0.5。
    #[test]
    fn compute_trim_ratio_handles_empty_and_half_trim() {
        assert_eq!(compute_trim_ratio(0, 0), 0.0);
        assert_eq!(compute_trim_ratio(100, 100), 0.0);
        assert!((compute_trim_ratio(100, 50) - 0.5).abs() < f32::EPSILON);
    }

    // v0.26.19 Phase 3.1 契约：should_retry_self_repetition 仅在
    //   trim_ratio >= 0.08 且 raw_chars > 100 时触发。
    //   - 8% 阈值边界：0.079 不触发，0.08 触发。
    //   - 100 字下限边界：trim_ratio 高但 raw=100 不触发，raw=101 触发。
    #[test]
    fn should_retry_self_repetition_threshold_boundary() {
        // 8% 阈值边界
        assert!(!should_retry_self_repetition(0.079, 500));
        assert!(should_retry_self_repetition(0.08, 500));
        assert!(should_retry_self_repetition(0.20, 500));
        // 100 字下限边界
        assert!(!should_retry_self_repetition(0.20, 100));
        assert!(should_retry_self_repetition(0.20, 101));
        // 短文本高比例不触发（与 trim_self_repetition 40 字旁路对齐，更保守）
        assert!(!should_retry_self_repetition(0.50, 50));
    }

    // v0.26.19 Phase 3.1 契约：select_first_chapter_content 在重试更干净时
    //   采用重试结果，否则保留首次清理结果。
    #[test]
    fn select_first_chapter_content_prefers_cleaner_retry() {
        let original = "原清理结果".to_string();
        let retry = "重试清理结果".to_string();
        // 重试更干净 (0.02 < 0.10) → 采用重试
        assert_eq!(
            select_first_chapter_content(0.10, 0.02, original.clone(), retry.clone()),
            retry
        );
        // 重试更脏 (0.15 > 0.10) → 保留原
        assert_eq!(
            select_first_chapter_content(0.10, 0.15, original.clone(), retry.clone()),
            original
        );
        // 相等 → 保留原（严格 <，相等不算更干净）
        assert_eq!(
            select_first_chapter_content(0.10, 0.10, original.clone(), retry.clone()),
            original
        );
    }
}
