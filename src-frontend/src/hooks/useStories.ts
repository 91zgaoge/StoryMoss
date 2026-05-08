import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { listStories, createStory, updateStory, deleteStory } from '@services/tauri';
import type { CreateStoryRequest, Story } from '@/types/index';
import toast from 'react-hot-toast';

const STORIES_KEY = 'stories';

export function useStories() {
  return useQuery<Story[]>({
    queryKey: [STORIES_KEY],
    queryFn: listStories,
  });
}

export function useCreateStory() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: createStory,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [STORIES_KEY] });
      toast.success('故事创建成功');
    },
    onError: (error: Error) => {
      toast.error('创建失败: ' + error.message);
    },
  });
}

export function useUpdateStory() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: ({ id, updates }: { id: string; updates: Partial<Story> }) => 
      updateStory(id, updates),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [STORIES_KEY] });
      toast.success('故事更新成功');
    },
    onError: (error: Error) => {
      toast.error('更新失败: ' + error.message);
    },
  });
}

export function useDeleteStory() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: deleteStory,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [STORIES_KEY] });
      toast.success('故事已删除');
    },
    onError: (error: Error) => {
      toast.error('删除失败: ' + error.message);
    },
  });
}
