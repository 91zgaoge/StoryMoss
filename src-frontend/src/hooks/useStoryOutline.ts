import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getStoryOutline, updateStoryOutline } from '@services/tauri';
import type { StoryOutline } from '@/types/index';

const STORY_OUTLINE_KEY = 'story-outline';

export function useStoryOutline(storyId: string | undefined) {
  return useQuery<StoryOutline | null>({
    queryKey: [STORY_OUTLINE_KEY, storyId],
    queryFn: () => (storyId ? getStoryOutline(storyId) : Promise.resolve(null)),
    enabled: !!storyId,
  });
}

export function useUpdateStoryOutline() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      storyId,
      content,
      structureJson,
    }: {
      storyId: string;
      content: string;
      structureJson?: string;
    }) => updateStoryOutline(storyId, content, structureJson),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: [STORY_OUTLINE_KEY, variables.storyId] });
    },
  });
}
