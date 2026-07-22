import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getCharacterRelationships,
  createCharacterRelationship,
  updateCharacterRelationship,
  deleteCharacterRelationship,
} from '@/services/tauri';
import type { CharacterRelationship } from '@/types/index';
import toast from 'react-hot-toast';

const CHARACTER_RELATIONSHIPS_KEY = 'character-relationships';

export function useCharacterRelationships(storyId: string | undefined) {
  return useQuery<CharacterRelationship[]>({
    queryKey: [CHARACTER_RELATIONSHIPS_KEY, storyId],
    queryFn: () => (storyId ? getCharacterRelationships(storyId) : Promise.resolve([])),
    enabled: !!storyId,
  });
}

export function useCreateCharacterRelationship() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (params: {
      story_id: string;
      character_a_id: string;
      character_b_id: string;
      relationship_type: string;
      description?: string;
    }) => createCharacterRelationship(params),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({
        queryKey: [CHARACTER_RELATIONSHIPS_KEY, variables.story_id],
      });
      toast.success('关系添加成功');
    },
    onError: (error: Error) => {
      toast.error('添加关系失败: ' + error.message);
    },
  });
}

export function useDeleteCharacterRelationship() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ relationshipId, storyId }: { relationshipId: string; storyId: string }) =>
      deleteCharacterRelationship(relationshipId),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({
        queryKey: [CHARACTER_RELATIONSHIPS_KEY, variables.storyId],
      });
      toast.success('关系已删除');
    },
    onError: (error: Error) => {
      toast.error('删除关系失败: ' + error.message);
    },
  });
}

// v0.30.16: 编辑角色关系（关系类型/描述）
export function useUpdateCharacterRelationship() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      relationshipId,
      storyId,
      relationship_type,
      description,
    }: {
      relationshipId: string;
      storyId: string;
      relationship_type?: string;
      description?: string;
    }) => updateCharacterRelationship(relationshipId, { relationship_type, description }),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({
        queryKey: [CHARACTER_RELATIONSHIPS_KEY, variables.storyId],
      });
      toast.success('关系已更新');
    },
    onError: (error: Error) => {
      toast.error('更新关系失败: ' + error.message);
    },
  });
}
