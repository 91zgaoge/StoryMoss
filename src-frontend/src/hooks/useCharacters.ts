import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { 
  getStoryCharacters, 
  createCharacter, 
  updateCharacter, 
  deleteCharacter,
  notifyFrontstageDataRefresh
} from '@services/tauri';
import type { CreateCharacterRequest, Character } from '@/types/index';
import toast from 'react-hot-toast';
import { useEffect } from 'react';

const CHARACTERS_KEY = 'characters';

export function useCharacters(storyId: string | null) {
  const queryClient = useQueryClient();
  
  useEffect(() => {
    const handleRefresh = () => {
      if (storyId) {
        queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY, storyId] });
      }
    };
    window.addEventListener('characters-refreshed', handleRefresh);
    return () => window.removeEventListener('characters-refreshed', handleRefresh);
  }, [storyId, queryClient]);
  
  return useQuery<Character[]>({
    queryKey: [CHARACTERS_KEY, storyId],
    queryFn: () => storyId ? getStoryCharacters(storyId) : Promise.resolve([]),
    enabled: !!storyId,
  });
}

export function useCreateCharacter() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: createCharacter,
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ 
        queryKey: [CHARACTERS_KEY, variables.story_id] 
      });
      toast.success('角色创建成功');
    },
    onError: (error: Error) => {
      toast.error('创建失败: ' + error.message);
    },
  });
}

export function useUpdateCharacter() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: ({ id, updates }: { id: string; updates: Partial<Character> }) => 
      updateCharacter(id, updates),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY] });
      toast.success('角色更新成功');
    },
    onError: (error: Error) => {
      toast.error('更新失败: ' + error.message);
    },
  });
}

export function useDeleteCharacter() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: deleteCharacter,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY] });
      toast.success('角色已删除');
    },
    onError: (error: Error) => {
      toast.error('删除失败: ' + error.message);
    },
  });
}
