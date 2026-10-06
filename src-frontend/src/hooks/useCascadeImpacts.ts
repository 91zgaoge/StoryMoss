import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';

/**
 * v0.60.0 P0-T4：改稿级联影响报告。
 *
 * 编辑旧章节后，后台自动分析受影响的后续章节并做冲突扫描，产出这些条目。
 * 三条/四条动作全部由作者决策（去查看 / 忽略 / 重跑分析 / 触发级联改写），
 * 系统不会自动改写后文。
 */
export interface CascadeImpact {
  id: string;
  story_id: string;
  batch_id: string;
  source_scene_id: string;
  source_chapter_number: number | null;
  target_scene_id: string;
  target_chapter_number: number | null;
  impact_score: number;
  /** mention = 实体共享；conflict = LLM 冲突扫描命中 */
  impact_kind: 'mention' | 'conflict' | string;
  /** info | warning | critical */
  severity: 'info' | 'warning' | 'critical' | string;
  entity_ids: string[];
  detail?: string | null;
  evidence?: string | null;
  /** open | ignored | rewrite_requested */
  decision: 'open' | 'ignored' | 'rewrite_requested' | string;
  /** true = 该目标章的分析可能已失效（重跑分析后自动清除） */
  stale_flag: boolean;
  created_at: string;
  updated_at: string;
}

export const CASCADE_IMPACTS_KEY = 'cascade_impacts';

export function cascadeImpactsQueryKey(storyId?: string) {
  return storyId ? [CASCADE_IMPACTS_KEY, storyId] : [CASCADE_IMPACTS_KEY];
}

export function useCascadeImpacts(storyId?: string, decision?: string) {
  return useQuery({
    queryKey: [...cascadeImpactsQueryKey(storyId), decision ?? 'all'],
    queryFn: async () => {
      if (!storyId) return [] as CascadeImpact[];
      return loggedInvoke<CascadeImpact[]>('list_cascade_impacts', {
        story_id: storyId,
        decision: decision ?? null,
        limit: 200,
      });
    },
    enabled: !!storyId,
    refetchInterval: 15000, // 兜底轮询：后台分析事件丢失时仍能刷新
  });
}

function invalidateAll(queryClient: ReturnType<typeof useQueryClient>, storyId?: string) {
  queryClient.invalidateQueries({ queryKey: cascadeImpactsQueryKey(storyId) });
}

export function useIgnoreCascadeImpact() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (impactId: string) => {
      return loggedInvoke<number>('ignore_cascade_impact', { impact_id: impactId });
    },
    onSuccess: () => invalidateAll(queryClient),
  });
}

export function useReanalyzeScene() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (sceneId: string) => {
      return loggedInvoke<void>('reanalyze_scene', { scene_id: sceneId });
    },
    onSuccess: () => {
      invalidateAll(queryClient);
      queryClient.invalidateQueries({ queryKey: ['scenes'] });
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
    },
  });
}

export function useTriggerCascadeRewriteForImpact() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (impactId: string) => {
      return loggedInvoke<string>('trigger_cascade_rewrite_for_impact', {
        impact_id: impactId,
      });
    },
    onSuccess: () => {
      invalidateAll(queryClient);
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
    },
  });
}
