import { loggedInvoke } from './core';

// ==================== 文思泉涌 ====================

export const autoWrite = (params: {
  story_id: string;
  chapter_id: string;
  target_chars: number;
  chars_per_loop: number;
  reference_text?: string;
  style_weight?: number;
}) =>
  loggedInvoke<{ task_id: string; actual_chars: number; loops: number; status: string }>(
    'auto_write',
    { request: params }
  );

export const autoWriteCancel = (_taskId: string) => Promise.resolve();

export const autoRevise = (params: {
  story_id: string;
  chapter_id?: string;
  scope: string;
  selected_text?: string;
  revision_type: string;
}) =>
  loggedInvoke<{ task_id: string; revised_text: string; status: string }>('auto_revise', {
    request: params,
  });

export const autoReviseCancel = (_taskId: string) => Promise.resolve();

// Window communication
export const notifyFrontstageDataRefresh = (entity: string) =>
  loggedInvoke<void>('notify_frontstage_data_refresh', { entity });
