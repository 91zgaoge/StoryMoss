import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';

/**
 * v0.64.0 「运行维护」页数据源：
 * - 质量债（P3-A）：质检降级/未解决问题的台账
 * - 待确认队列（P3-B）：分析自动新增的规则类资产
 * - 文风偏好（P2-B）：从作者手改中提炼的文风规则
 * - 成本账本（P2-D）：按故事聚合的调用/token 与计费盲区告警
 */

// ---------- 质量债 ----------

export interface QualityDebt {
  id: string;
  story_id: string;
  scene_id?: string | null;
  chapter_number?: number | null;
  source: string;
  severity: string;
  detail: string;
  suggested_window?: string | null;
  status: string;
  created_at: string;
  updated_at: string;
}

export function useQualityDebts(storyId?: string, status?: string) {
  return useQuery({
    queryKey: ['quality_debts', storyId, status ?? 'open'],
    queryFn: async () => {
      if (!storyId) return [] as QualityDebt[];
      return loggedInvoke<QualityDebt[]>('list_quality_debts', {
        story_id: storyId,
        status: status ?? null,
      });
    },
    enabled: !!storyId,
    refetchInterval: 30000,
  });
}

export function useResolveQualityDebt() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ debtId, status }: { debtId: string; status: 'resolved' | 'dismissed' }) =>
      loggedInvoke<number>('resolve_quality_debt', { debt_id: debtId, status }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['quality_debts'] }),
  });
}

// ---------- 待确认队列 ----------

export interface PendingReview {
  id: string;
  story_id: string;
  kind: string;
  subject: string;
  detail?: string | null;
  source: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export function usePendingReviews(storyId?: string, status?: string) {
  return useQuery({
    queryKey: ['pending_reviews', storyId, status ?? 'pending'],
    queryFn: async () => {
      if (!storyId) return [] as PendingReview[];
      return loggedInvoke<PendingReview[]>('list_pending_reviews', {
        story_id: storyId,
        status: status ?? null,
      });
    },
    enabled: !!storyId,
    refetchInterval: 30000,
  });
}

export function useResolvePendingReview() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      reviewId,
      status,
    }: {
      reviewId: string;
      status: 'confirmed' | 'rejected';
    }) => loggedInvoke<number>('resolve_pending_review', { review_id: reviewId, status }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['pending_reviews'] }),
  });
}

// ---------- 文风偏好 ----------

export interface StylePreference {
  id: string;
  story_id: string;
  pattern: string;
  evidence?: string | null;
  source: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export function useStylePreferences(storyId?: string, status?: string) {
  return useQuery({
    queryKey: ['style_preferences', storyId, status ?? 'all'],
    queryFn: async () => {
      if (!storyId) return [] as StylePreference[];
      return loggedInvoke<StylePreference[]>('list_style_preferences', {
        story_id: storyId,
        status: status ?? null,
      });
    },
    enabled: !!storyId,
    refetchInterval: 30000,
  });
}

export function useSetStylePreferenceStatus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ preferenceId, active }: { preferenceId: string; active: boolean }) =>
      loggedInvoke<number>('set_style_preference_status', {
        preference_id: preferenceId,
        active,
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['style_preferences'] }),
  });
}

// ---------- 成本账本 ----------

export interface StoryCostSummary {
  story_id: string;
  total_calls: number;
  total_tokens: number;
  prompt_tokens: number;
  completion_tokens: number;
  failed_calls: number;
  zero_token_calls: number;
  first_call_at?: string | null;
  last_call_at?: string | null;
  budget_warning: boolean;
  warn_threshold: number;
}

export interface CostAnomaly {
  kind: string;
  detail: string;
}

export interface StoryCostReport {
  summary: StoryCostSummary;
  anomalies: CostAnomaly[];
}

export function useStoryCost(storyId?: string) {
  return useQuery({
    queryKey: ['story_cost', storyId],
    queryFn: async () => {
      if (!storyId) return null;
      return loggedInvoke<StoryCostReport>('get_story_cost_summary', { story_id: storyId });
    },
    enabled: !!storyId,
    refetchInterval: 30000,
  });
}

// ---------- 物料失效与重算（v0.64.11） ----------

export interface StaleMaterial {
  kind: string;
  from_chapter: number;
  reason?: string | null;
  updated_at: string;
}

export interface RecomputeReport {
  from_chapter: number;
  chapter_summaries: number;
  summary_debts: string[];
  segment_summaries_deleted: number;
  book_summary_deleted: boolean;
  checkpoints_rewritten: number;
  segment_pending_llm: boolean;
}

/** 编辑旧章后失效的跨章物料（章节摘要 / 分层摘要与全书纲要 / 连续性快照）。 */
export function useStaleMaterials(storyId?: string) {
  return useQuery({
    queryKey: ['stale_materials', storyId],
    queryFn: async () => {
      if (!storyId) return [] as StaleMaterial[];
      return loggedInvoke<StaleMaterial[]>('list_stale_materials', { story_id: storyId });
    },
    enabled: !!storyId,
    refetchInterval: 30000,
  });
}

/** 一键重算第 N 章及以后的物料；缺省取最小失效章。 */
export function useRecomputeMaterial() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ storyId, fromChapter }: { storyId: string; fromChapter?: number }) =>
      loggedInvoke<RecomputeReport>('recompute_story_material', {
        story_id: storyId,
        from_chapter: fromChapter ?? null,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['stale_materials'] });
      queryClient.invalidateQueries({ queryKey: ['quality_debts'] });
    },
  });
}
