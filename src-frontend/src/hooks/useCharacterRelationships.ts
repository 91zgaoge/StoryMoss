import { useQuery } from '@tanstack/react-query';
import { getCharacterRelationships } from '@services/tauri';
import type { CharacterRelationship } from '@/types/index';

const CHARACTER_RELATIONSHIPS_KEY = 'character-relationships';

export function useCharacterRelationships(storyId: string | undefined) {
  return useQuery<CharacterRelationship[]>({
    queryKey: [CHARACTER_RELATIONSHIPS_KEY, storyId],
    queryFn: () => (storyId ? getCharacterRelationships(storyId) : Promise.resolve([])),
    enabled: !!storyId,
  });
}
