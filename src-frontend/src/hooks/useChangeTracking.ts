import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import type { ChangeTrack } from '@/types/v3';

const PENDING_CHANGES_KEY = 'pendingChanges';

export function usePendingChanges(sceneId: string | undefined, chapterId: string | undefined) {
  return useQuery({
    queryKey: [PENDING_CHANGES_KEY, sceneId, chapterId],
    queryFn: () => {
      if (sceneId) {
        return loggedInvoke<ChangeTrack[]>('get_pending_changes', { scene_id: sceneId });
      }
      if (chapterId) {
        return loggedInvoke<ChangeTrack[]>('get_pending_changes', { chapter_id: chapterId });
      }
      return Promise.resolve([]);
    },
    enabled: !!sceneId || !!chapterId,
  });
}

export function useVersionChangeTracks(versionId: string | undefined) {
  return useQuery({
    queryKey: ['versionChangeTracks', versionId],
    queryFn: () => {
      if (!versionId) return Promise.resolve([]);
      return loggedInvoke<ChangeTrack[]>('get_version_change_tracks', { version_id: versionId });
    },
    enabled: !!versionId,
  });
}

export function useTrackChange() {
  const queryClient = useQueryClient();

  return useMutation<
    ChangeTrack,
    Error,
    {
      sceneId?: string;
      chapterId?: string;
      changeType: 'Insert' | 'Delete' | 'Format';
      fromPos: number;
      toPos: number;
      content?: string;
      authorId?: string;
    }
  >({
    mutationFn: ({ sceneId, chapterId, changeType, fromPos, toPos, content, authorId }) =>
      loggedInvoke<ChangeTrack>('track_change', {
        scene_id: sceneId ?? null,
        chapter_id: chapterId ?? null,
        change_type: changeType,
        from_pos: fromPos,
        to_pos: toPos,
        content,
        author_id: authorId,
      }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [PENDING_CHANGES_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useAcceptChange() {
  const queryClient = useQueryClient();

  return useMutation<number, Error, { changeId: string; sceneId?: string; chapterId?: string }>({
    mutationFn: ({ changeId }) => loggedInvoke<number>('accept_change', { change_id: changeId }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [PENDING_CHANGES_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useRejectChange() {
  const queryClient = useQueryClient();

  return useMutation<number, Error, { changeId: string; sceneId?: string; chapterId?: string }>({
    mutationFn: ({ changeId }) => loggedInvoke<number>('reject_change', { change_id: changeId }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [PENDING_CHANGES_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useAcceptAllChanges() {
  const queryClient = useQueryClient();

  return useMutation<number, Error, { sceneId?: string; chapterId?: string }>({
    mutationFn: ({ sceneId, chapterId }) => {
      if (sceneId) {
        return loggedInvoke<number>('accept_all_changes', { scene_id: sceneId });
      }
      return loggedInvoke<number>('accept_all_changes', { chapter_id: chapterId });
    },
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [PENDING_CHANGES_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useRejectAllChanges() {
  const queryClient = useQueryClient();

  return useMutation<number, Error, { sceneId?: string; chapterId?: string }>({
    mutationFn: ({ sceneId, chapterId }) => {
      if (sceneId) {
        return loggedInvoke<number>('reject_all_changes', { scene_id: sceneId });
      }
      return loggedInvoke<number>('reject_all_changes', { chapter_id: chapterId });
    },
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [PENDING_CHANGES_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}
