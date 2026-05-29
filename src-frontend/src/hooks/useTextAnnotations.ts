import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { TextAnnotation } from '@/types/v3';
import {
  createTextAnnotation,
  getTextAnnotationsByChapter,
  getTextAnnotationsByScene,
  updateTextAnnotation,
  resolveTextAnnotation,
  unresolveTextAnnotation,
  deleteTextAnnotation,
} from '@/services/tauri';

export function useTextAnnotationsByChapter(chapterId: string | null) {
  return useQuery({
    queryKey: ['text-annotations', 'chapter', chapterId],
    queryFn: () => (chapterId ? getTextAnnotationsByChapter(chapterId) : Promise.resolve([])),
    enabled: !!chapterId,
  });
}

export function useTextAnnotationsByScene(sceneId: string | null) {
  return useQuery({
    queryKey: ['text-annotations', 'scene', sceneId],
    queryFn: () => (sceneId ? getTextAnnotationsByScene(sceneId) : Promise.resolve([])),
    enabled: !!sceneId,
  });
}

export function useCreateTextAnnotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createTextAnnotation,
    onSuccess: (_, vars) => {
      if (vars.chapter_id) {
        queryClient.invalidateQueries({
          queryKey: ['text-annotations', 'chapter', vars.chapter_id],
        });
      }
      if (vars.scene_id) {
        queryClient.invalidateQueries({ queryKey: ['text-annotations', 'scene', vars.scene_id] });
      }
    },
  });
}

export function useUpdateTextAnnotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ annotationId, content }: { annotationId: string; content: string }) =>
      updateTextAnnotation(annotationId, content),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['text-annotations'] });
    },
  });
}

export function useResolveTextAnnotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: resolveTextAnnotation,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['text-annotations'] });
    },
  });
}

export function useUnresolveTextAnnotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: unresolveTextAnnotation,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['text-annotations'] });
    },
  });
}

export function useDeleteTextAnnotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: deleteTextAnnotation,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['text-annotations'] });
    },
  });
}

export const TEXT_ANNOTATION_TYPE_LABELS: Record<TextAnnotation['annotation_type'], string> = {
  note: '笔记',
  todo: '待办',
  warning: '注意',
  idea: '灵感',
};

export const TEXT_ANNOTATION_TYPE_COLORS: Record<TextAnnotation['annotation_type'], string> = {
  note: 'bg-blue-500',
  todo: 'bg-orange-500',
  warning: 'bg-red-500',
  idea: 'bg-purple-500',
};
