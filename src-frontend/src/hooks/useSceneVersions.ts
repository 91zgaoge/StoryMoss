import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import type {
  SceneVersion,
  VersionDiff,
  VersionChainNode,
  VersionStats,
  CreatorType,
} from '@/types';

const VERSIONS_KEY = 'scene-versions';

// ==================== Queries ====================

export function useSceneVersions(sceneId: string | null) {
  return useQuery({
    queryKey: [VERSIONS_KEY, sceneId],
    queryFn: async () => {
      if (!sceneId) return [];
      return loggedInvoke<SceneVersion[]>('get_scene_versions', { scene_id: sceneId });
    },
    enabled: !!sceneId,
  });
}

export function useSceneVersion(versionId: string | null) {
  return useQuery({
    queryKey: [VERSIONS_KEY, 'detail', versionId],
    queryFn: async () => {
      if (!versionId) return null;
      return loggedInvoke<SceneVersion | null>('get_scene_version', { version_id: versionId });
    },
    enabled: !!versionId,
  });
}

export function useVersionDiff(fromVersionId: string | null, toVersionId: string | null) {
  return useQuery({
    queryKey: [VERSIONS_KEY, 'diff', fromVersionId, toVersionId],
    queryFn: async () => {
      if (!fromVersionId || !toVersionId) return null;
      return loggedInvoke<VersionDiff>('compare_scene_versions', {
        from_version_id: fromVersionId,
        to_version_id: toVersionId,
      });
    },
    enabled: !!fromVersionId && !!toVersionId,
  });
}

export function useVersionChain(sceneId: string | null) {
  return useQuery({
    queryKey: [VERSIONS_KEY, 'chain', sceneId],
    queryFn: async () => {
      if (!sceneId) return [];
      return loggedInvoke<VersionChainNode[]>('get_scene_version_chain', { scene_id: sceneId });
    },
    enabled: !!sceneId,
  });
}

export function useVersionStats(sceneId: string | null) {
  return useQuery({
    queryKey: [VERSIONS_KEY, 'stats', sceneId],
    queryFn: async () => {
      if (!sceneId) return null;
      return loggedInvoke<VersionStats>('get_scene_version_stats', { scene_id: sceneId });
    },
    enabled: !!sceneId,
  });
}

// ==================== Mutations ====================

export function useCreateSceneVersion() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      sceneId: string;
      changeSummary: string;
      createdBy: CreatorType;
      confidenceScore?: number;
    }) => {
      return loggedInvoke<SceneVersion>('create_scene_version', {
        scene_id: params.sceneId,
        change_summary: params.changeSummary,
        created_by: params.createdBy,
        confidence_score: params.confidenceScore,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, variables.sceneId] });
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, 'stats', variables.sceneId] });
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, 'chain', variables.sceneId] });
    },
  });
}

export function useRestoreSceneVersion() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { sceneId: string; versionId: string; restoredBy: CreatorType }) => {
      return loggedInvoke<SceneVersion>('restore_scene_version', {
        scene_id: params.sceneId,
        version_id: params.versionId,
        restored_by: params.restoredBy,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, variables.sceneId] });
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, 'stats', variables.sceneId] });
      queryClient.invalidateQueries({ queryKey: ['scenes'] });
    },
  });
}

export function useDeleteSceneVersion() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { versionId: string; sceneId: string }) => {
      return loggedInvoke<number>('delete_scene_version', {
        version_id: params.versionId,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, variables.sceneId] });
      queryClient.invalidateQueries({ queryKey: [VERSIONS_KEY, 'stats', variables.sceneId] });
    },
  });
}

// ==================== Helpers ====================

export function getCreatorTypeLabel(type: CreatorType): string {
  const labels: Record<CreatorType, string> = {
    user: '用户',
    ai: 'AI',
    system: '系统',
  };
  return labels[type] || type;
}

export function getCreatorTypeColor(type: CreatorType): string {
  const colors: Record<CreatorType, string> = {
    user: '#3b82f6', // blue
    ai: '#10b981', // emerald
    system: '#6b7280', // gray
  };
  return colors[type] || '#6b7280';
}

export function getCreatorTypeIcon(type: CreatorType): string {
  const icons: Record<CreatorType, string> = {
    user: '👤',
    ai: '🤖',
    system: '⚙️',
  };
  return icons[type] || '❓';
}

export function formatVersionNumber(num: number): string {
  return `v${num}`;
}

export function getConfidenceColor(score?: number): string {
  if (score === undefined) return '#9ca3af';
  if (score >= 0.8) return '#10b981'; // green
  if (score >= 0.6) return '#3b82f6'; // blue
  if (score >= 0.4) return '#f59e0b'; // amber
  if (score >= 0.2) return '#ef4444'; // red
  return '#6b7280'; // gray
}

export function getConfidenceLabel(score?: number): string {
  if (score === undefined) return '未知';
  if (score >= 0.8) return '高置信度';
  if (score >= 0.6) return '较高置信度';
  if (score >= 0.4) return '中等置信度';
  if (score >= 0.2) return '低置信度';
  return '极低置信度';
}

export function calculateWordCountDelta(
  current: number,
  previous: number
): {
  delta: number;
  percentage: number;
  isIncrease: boolean;
} {
  const delta = current - previous;
  const percentage = previous > 0 ? Math.abs(delta / previous) * 100 : 0;
  return {
    delta,
    percentage,
    isIncrease: delta >= 0,
  };
}
