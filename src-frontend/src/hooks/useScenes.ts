import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import type { Scene, CreateSceneRequest, UpdateSceneRequest, ConflictType, Chapter } from '@/types';

const SCENES_KEY = 'scenes';

// ==================== Queries ====================

export function useScenes(storyId: string | null) {
  return useQuery({
    queryKey: [SCENES_KEY, storyId],
    queryFn: async () => {
      if (!storyId) return [];
      return loggedInvoke<Scene[]>('get_story_scenes', { story_id: storyId });
    },
    enabled: !!storyId,
  });
}

export function useScene(sceneId: string | null) {
  return useQuery({
    queryKey: [SCENES_KEY, 'detail', sceneId],
    queryFn: async () => {
      if (!sceneId) return null;
      return loggedInvoke<Scene | null>('get_scene', { scene_id: sceneId });
    },
    enabled: !!sceneId,
  });
}

export function useSceneWithChapter(sceneId: string | null) {
  const { data: scene, isLoading: sceneLoading, isError: sceneError } = useScene(sceneId);
  const {
    data: chapter,
    isLoading: chapterLoading,
    isError: chapterError,
  } = useQuery({
    queryKey: [SCENES_KEY, 'chapter', sceneId],
    queryFn: async () => {
      if (!scene?.chapter_id) return null;
      return loggedInvoke<Chapter | null>('get_chapter', { id: scene.chapter_id });
    },
    enabled: !!scene?.chapter_id,
  });

  return {
    scene,
    chapter,
    isLoading: sceneLoading || chapterLoading,
    isError: sceneError || chapterError,
  };
}

// ==================== Mutations ====================

export function useCreateScene() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      storyId: string;
      sequenceNumber: number;
      title?: string;
      dramaticGoal?: string;
      externalPressure?: string;
      conflictType?: ConflictType;
      charactersPresent?: string[];
      settingLocation?: string;
      content?: string;
    }) => {
      return loggedInvoke<Scene>('create_scene', {
        story_id: params.storyId,
        sequence_number: params.sequenceNumber,
        title: params.title,
        dramatic_goal: params.dramaticGoal,
        external_pressure: params.externalPressure,
        conflict_type: params.conflictType,
        characters_present: params.charactersPresent || [],
        setting_location: params.settingLocation,
        content: params.content,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [SCENES_KEY, variables.storyId] });
    },
  });
}

export function useUpdateScene() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      sceneId: string;
      storyId: string;
      updates: UpdateSceneRequest;
    }) => {
      return loggedInvoke<number>('update_scene', {
        scene_id: params.sceneId,
        updates: params.updates,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [SCENES_KEY, variables.storyId] });
      queryClient.invalidateQueries({ queryKey: [SCENES_KEY, 'detail', variables.sceneId] });
    },
  });
}

export function useDeleteScene() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { sceneId: string; storyId: string }) => {
      return loggedInvoke<number>('delete_scene', { scene_id: params.sceneId });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [SCENES_KEY, variables.storyId] });
    },
  });
}

export function useReorderScenes() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { storyId: string; sceneIds: string[] }) => {
      return loggedInvoke<void>('reorder_scenes', {
        story_id: params.storyId,
        scene_ids: params.sceneIds,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [SCENES_KEY, variables.storyId] });
    },
  });
}

// ==================== Helpers ====================

export function getConflictTypeLabel(type: ConflictType): string {
  const labels: Record<ConflictType, string> = {
    ManVsMan: '人与人',
    ManVsSelf: '人与自我',
    ManVsSociety: '人与社会',
    ManVsNature: '人与自然',
    ManVsTechnology: '人与科技',
    ManVsFate: '人与命运',
    ManVsSupernatural: '人与超自然',
    ManVsTime: '人与时间',
    ManVsMorality: '人与道德',
    ManVsIdentity: '人与身份',
    FactionVsFaction: '群体冲突',
  };
  return labels[type] || type;
}

export function getConflictTypeColor(type: ConflictType): string {
  const colors: Record<ConflictType, string> = {
    ManVsMan: '#ef4444', // red
    ManVsSelf: '#8b5cf6', // purple
    ManVsSociety: '#f59e0b', // amber
    ManVsNature: '#10b981', // emerald
    ManVsTechnology: '#3b82f6', // blue
    ManVsFate: '#6366f1', // indigo
    ManVsSupernatural: '#ec4899', // pink
    ManVsTime: '#06b6d4', // cyan
    ManVsMorality: '#f43f5e', // rose
    ManVsIdentity: '#14b8a6', // teal
    FactionVsFaction: '#64748b', // slate
  };
  return colors[type] || '#6b7280';
}
