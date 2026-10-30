import { useQuery } from '@tanstack/react-query';
import {
  getCorpus,
  listCorpus,
  readCorpusMedia,
  type CorpusListParams,
} from '@/services/api/corpus';

export function useCorpus(storyId: string | null, params: CorpusListParams = {}) {
  return useQuery({
    queryKey: ['corpus', storyId, params.kind ?? 'all', params.status ?? 'all', params.query ?? ''],
    queryFn: () => listCorpus(storyId!, params),
    enabled: !!storyId,
  });
}

export function useCorpusDetail(id: string | null) {
  return useQuery({
    queryKey: ['corpus-detail', id],
    queryFn: () => getCorpus(id!),
    enabled: !!id,
  });
}

export function useCorpusMedia(id: string | null, enabled: boolean) {
  return useQuery({
    queryKey: ['corpus-media', id],
    queryFn: () => readCorpusMedia(id!),
    enabled: !!id && enabled,
  });
}
