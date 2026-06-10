/**
 * 自动保存调度器 — W4-F7
 *
 * 将序列化 + IPC 调用移出输入关键路径，避免主线程阻塞。
 * 策略：
 * 1. 输入事件（onChange）只更新本地状态，不触发保存。
 * 2. debounce 到期后，使用 requestIdleCallback（或 setTimeout 0）将保存任务排入空闲队列。
 * 3. 保存任务内部使用 startTransition（若可用）标记为非紧急更新。
 * 4. 保存进行中时，新的输入自动取消旧保存并重新调度。
 */

import { startTransition } from 'react';

export interface AutoSaveTask {
  cancel: () => void;
}

interface SavePayload {
  chapterId: string;
  title?: string;
  content: string;
  wordCount: number;
}

type SaveFn = (payload: SavePayload) => Promise<void>;

const DEFAULT_DEBOUNCE_MS = 2000;

let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let idleCallbackId: ReturnType<typeof setTimeout> | number | null = null;
let isSaving = false;

/**
 * 调度一次自动保存
 * @param payload 保存数据
 * @param saveFn 实际执行保存的异步函数
 * @param debounceMs 防抖间隔
 * @returns 可取消的任务句柄
 */
export function scheduleAutoSave(
  payload: SavePayload,
  saveFn: SaveFn,
  debounceMs: number = DEFAULT_DEBOUNCE_MS
): AutoSaveTask {
  // 取消旧的调度
  cancelAutoSave();

  const task: AutoSaveTask = {
    cancel: () => cancelAutoSave(),
  };

  debounceTimer = setTimeout(() => {
    debounceTimer = null;

    // 使用 requestIdleCallback 将保存任务推入浏览器空闲时段，
    // 确保输入、动画等用户可见任务优先执行。
    const scheduleSave = () => {
      isSaving = true;

      const executeSave = async () => {
        try {
          await saveFn(payload);
        } finally {
          isSaving = false;
          idleCallbackId = null;
        }
      };

      // React 18+: 使用 startTransition 标记状态更新为非紧急，
      // 避免保存相关的副作用（如 setIsSaved）触发高优先级重新渲染。
      if (typeof startTransition === 'function') {
        startTransition(() => {
          executeSave();
        });
      } else {
        executeSave();
      }
    };

    if (typeof window !== 'undefined' && 'requestIdleCallback' in window) {
      idleCallbackId = window.requestIdleCallback(scheduleSave, { timeout: 500 });
    } else {
      // Fallback: 使用 setTimeout 0 让出主线程
      idleCallbackId = setTimeout(scheduleSave, 0);
    }
  }, debounceMs);

  return task;
}

/**
 * 取消待执行的自动保存
 */
export function cancelAutoSave(): void {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  if (idleCallbackId !== null) {
    if (typeof window !== 'undefined' && 'cancelIdleCallback' in window) {
      window.cancelIdleCallback(idleCallbackId as number);
    } else {
      clearTimeout(idleCallbackId as number);
    }
    idleCallbackId = null;
  }
}

/**
 * 查询是否正在保存中
 */
export function getIsSaving(): boolean {
  return isSaving;
}
