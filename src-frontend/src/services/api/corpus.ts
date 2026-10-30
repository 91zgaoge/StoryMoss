import { loggedInvoke } from './core';

export interface CorpusSummary {
  id: string;
  story_id: string;
  kind: string;
  title: string;
  summary: string;
  source_url?: string | null;
  source_name?: string | null;
  tags: string[];
  status: string;
  pinned: boolean;
  created_at: string;
  updated_at: string;
}

export interface CorpusLink {
  id: string;
  corpus_id: string;
  target_type: string;
  target_id: string;
  note?: string | null;
  created_at: string;
}

export interface CorpusDistillation {
  version: number;
  model: string;
  generated_at: string;
  facts: string[];
  conflicts: string[];
  archetypes: string[];
  world_hooks: string[];
  adaptation_angles: string[];
  cautions: string[];
}

export interface CorpusDetail extends CorpusSummary {
  content: string;
  file_name?: string | null;
  mime_type?: string | null;
  file_size?: number | null;
  distilled?: CorpusDistillation | null;
  links: CorpusLink[];
}

export interface CorpusPromotionDraft {
  target_type: string;
  title: string;
  fields: Record<string, unknown>;
}

export interface CorpusMedia {
  mime: string;
  data_url: string;
}

export interface CorpusUpdate {
  kind?: string;
  title?: string;
  summary?: string;
  content?: string;
  source_url?: string;
  source_name?: string;
  tags?: string[];
  status?: string;
  pinned?: boolean;
}

export interface CorpusListParams {
  kind?: string;
  status?: string;
  query?: string;
  limit?: number;
  offset?: number;
}

export const listCorpus = (storyId: string, params: CorpusListParams = {}) =>
  loggedInvoke<CorpusSummary[]>('corpus_list', {
    story_id: storyId,
    kind: params.kind ?? null,
    status: params.status ?? null,
    query: params.query ?? null,
    limit: params.limit ?? 200,
    offset: params.offset ?? 0,
  });

export const getCorpus = (id: string) => loggedInvoke<CorpusDetail>('corpus_get', { id });

export const createCorpus = (params: {
  story_id: string;
  kind: string;
  title: string;
  content?: string;
  summary?: string;
  source_url?: string;
  source_name?: string;
  tags?: string[];
}) =>
  loggedInvoke<CorpusDetail>('corpus_create', {
    story_id: params.story_id,
    kind: params.kind,
    title: params.title,
    content: params.content ?? null,
    summary: params.summary ?? null,
    source_url: params.source_url ?? null,
    source_name: params.source_name ?? null,
    tags: params.tags ?? null,
  });

export const updateCorpus = (id: string, updates: CorpusUpdate) =>
  loggedInvoke<CorpusDetail>('corpus_update', { id, updates });

export const deleteCorpus = (id: string) => loggedInvoke<void>('corpus_delete', { id });

export const importCorpusFile = (params: {
  story_id: string;
  kind: string;
  file_path: string;
  title?: string;
  tags?: string[];
}) =>
  loggedInvoke<CorpusDetail>('corpus_import_file', {
    story_id: params.story_id,
    kind: params.kind,
    file_path: params.file_path,
    title: params.title ?? null,
    tags: params.tags ?? null,
  });

export const readCorpusMedia = (id: string) =>
  loggedInvoke<CorpusMedia>('corpus_read_media', { id });

export const distillCorpus = (id: string) =>
  loggedInvoke<CorpusDistillation>('corpus_distill', { id });

export const promoteCorpus = (id: string, targetType: string) =>
  loggedInvoke<CorpusPromotionDraft>('corpus_promote', { id, target_type: targetType });

export const linkCorpus = (params: {
  corpus_id: string;
  target_type: string;
  target_id: string;
  note?: string;
}) =>
  loggedInvoke<void>('corpus_link', {
    corpus_id: params.corpus_id,
    target_type: params.target_type,
    target_id: params.target_id,
    note: params.note ?? null,
  });

export const unlinkCorpus = (linkId: string) =>
  loggedInvoke<void>('corpus_unlink', { link_id: linkId });
