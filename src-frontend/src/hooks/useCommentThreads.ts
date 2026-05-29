import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import type { CommentThread, CommentMessage, CommentThreadWithMessages } from '@/types/v3';

const COMMENT_THREADS_KEY = 'commentThreads';

export function useCommentThreads(sceneId: string | undefined, chapterId: string | undefined) {
  return useQuery({
    queryKey: [COMMENT_THREADS_KEY, sceneId, chapterId],
    queryFn: () => {
      if (sceneId) {
        return loggedInvoke<CommentThreadWithMessages[]>('get_comment_threads', {
          scene_id: sceneId,
        });
      }
      if (chapterId) {
        return loggedInvoke<CommentThreadWithMessages[]>('get_comment_threads', {
          chapter_id: chapterId,
        });
      }
      return Promise.resolve([]);
    },
    enabled: !!sceneId || !!chapterId,
  });
}

export function useCreateCommentThread() {
  const queryClient = useQueryClient();

  return useMutation<
    CommentThread,
    Error,
    {
      versionId?: string;
      anchorType: 'TextRange' | 'SceneLevel';
      sceneId?: string;
      chapterId?: string;
      fromPos?: number;
      toPos?: number;
      selectedText?: string;
    }
  >({
    mutationFn: ({ versionId, anchorType, sceneId, chapterId, fromPos, toPos, selectedText }) =>
      loggedInvoke<CommentThread>('create_comment_thread', {
        version_id: versionId ?? null,
        anchor_type: anchorType,
        scene_id: sceneId ?? null,
        chapter_id: chapterId ?? null,
        from_pos: fromPos ?? null,
        to_pos: toPos ?? null,
        selected_text: selectedText ?? null,
      }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [COMMENT_THREADS_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useAddCommentMessage() {
  const queryClient = useQueryClient();

  return useMutation<
    CommentMessage,
    Error,
    {
      threadId: string;
      content: string;
      authorId?: string;
      sceneId?: string;
      chapterId?: string;
    }
  >({
    mutationFn: ({ threadId, content, authorId }) =>
      loggedInvoke<CommentMessage>('add_comment_message', {
        thread_id: threadId,
        content,
        author_id: authorId,
      }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [COMMENT_THREADS_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useResolveCommentThread() {
  const queryClient = useQueryClient();

  return useMutation<
    number,
    Error,
    {
      threadId: string;
      sceneId?: string;
      chapterId?: string;
    }
  >({
    mutationFn: ({ threadId }) =>
      loggedInvoke<number>('resolve_comment_thread', { thread_id: threadId }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [COMMENT_THREADS_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useReopenCommentThread() {
  const queryClient = useQueryClient();

  return useMutation<
    number,
    Error,
    {
      threadId: string;
      sceneId?: string;
      chapterId?: string;
    }
  >({
    mutationFn: ({ threadId }) =>
      loggedInvoke<number>('reopen_comment_thread', { thread_id: threadId }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [COMMENT_THREADS_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}

export function useDeleteCommentThread() {
  const queryClient = useQueryClient();

  return useMutation<
    number,
    Error,
    {
      threadId: string;
      sceneId?: string;
      chapterId?: string;
    }
  >({
    mutationFn: ({ threadId }) =>
      loggedInvoke<number>('delete_comment_thread', { thread_id: threadId }),
    onSuccess: (_, vars) => {
      queryClient.invalidateQueries({
        queryKey: [COMMENT_THREADS_KEY, vars.sceneId, vars.chapterId],
      });
    },
  });
}
