import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { WorldBuilding, WorldRule, Culture, WritingStyle, WritingStyleUpdate } from '@/types';

const WORLD_BUILDING_KEY = 'world_building';
const WRITING_STYLE_KEY = 'writing_style';

// ==================== World Building ====================

export function useWorldBuilding(storyId: string | null) {
  return useQuery({
    queryKey: [WORLD_BUILDING_KEY, storyId],
    queryFn: async () => {
      if (!storyId) return null;
      return invoke<WorldBuilding | null>('get_world_building', { story_id: storyId });
    },
    enabled: !!storyId,
  });
}

export function useCreateWorldBuilding() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (params: { storyId: string; concept: string }) => {
      return invoke<WorldBuilding>('create_world_building', {
        story_id: params.storyId,
        concept: params.concept,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [WORLD_BUILDING_KEY, variables.storyId] });
    },
  });
}

export function useUpdateWorldBuilding() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (params: {
      id: string;
      storyId: string;
      concept?: string;
      rules?: WorldRule[];
      history?: string;
      cultures?: Culture[];
    }) => {
      return invoke<number>('update_world_building', {
        id: params.id,
        concept: params.concept,
        rules: params.rules,
        history: params.history,
        cultures: params.cultures,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [WORLD_BUILDING_KEY, variables.storyId] });
    },
  });
}

export function useDeleteWorldBuilding() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { id: string; storyId: string }) => {
      return invoke<number>('delete_world_building', { id: params.id });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [WORLD_BUILDING_KEY, variables.storyId] });
    },
  });
}

// ==================== Writing Style ====================

export function useWritingStyle(storyId: string | null) {
  return useQuery({
    queryKey: [WRITING_STYLE_KEY, storyId],
    queryFn: async () => {
      if (!storyId) return null;
      return invoke<WritingStyle | null>('get_writing_style', { story_id: storyId });
    },
    enabled: !!storyId,
  });
}

export function useCreateWritingStyle() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (storyId: string) => {
      return invoke<WritingStyle>('create_writing_style', { story_id: storyId });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [WRITING_STYLE_KEY, variables] });
    },
  });
}

export function useUpdateWritingStyle() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (params: {
      id: string;
      storyId: string;
      updates: WritingStyleUpdate;
    }) => {
      return invoke<number>('update_writing_style', {
        id: params.id,
        updates: params.updates,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [WRITING_STYLE_KEY, variables.storyId] });
    },
  });
}
