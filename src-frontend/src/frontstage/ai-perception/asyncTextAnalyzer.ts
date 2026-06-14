/**
 * 智能文思 — 感知层：异步分析入口
 *
 * 通过 Web Worker 执行 analyzeText，支持 AbortSignal 取消。
 * 在 Worker 不可用的环境（测试/SSR）自动降级为同步分析。
 */

import type { PerceptionResult } from './types';
import TextAnalyzerWorker from './textAnalyzer.worker.ts?worker';

let worker: Worker | null = null;
let nextTaskId = 1;

interface PendingTask {
  resolve: (value: PerceptionResult) => void;
  reject: (reason: Error) => void;
  signal?: AbortSignal;
}

const pendingTasks = new Map<number, PendingTask>();

function getWorker(): Worker {
  if (!worker) {
    worker = new TextAnalyzerWorker();
    worker.onmessage = (
      event: MessageEvent<{ id: number; result?: PerceptionResult; error?: string }>
    ) => {
      const { id, result, error } = event.data;
      const task = pendingTasks.get(id);
      if (!task) return;
      pendingTasks.delete(id);
      if (task.signal?.aborted) return;
      if (error) {
        task.reject(new Error(error));
      } else if (result) {
        task.resolve(result);
      } else {
        task.reject(new Error('Worker returned empty result'));
      }
    };
    worker.onerror = (err: ErrorEvent) => {
      // Worker 全局错误：清空所有待处理任务
      const message = err.message || 'Text analyzer worker error';
      for (const [, task] of pendingTasks) {
        if (!task.signal?.aborted) {
          task.reject(new Error(message));
        }
      }
      pendingTasks.clear();
    };
  }
  return worker;
}

/**
 * 异步分析 HTML 内容。
 * @param htmlContent TipTap 编辑器 HTML
 * @param signal 用于取消本次分析的 AbortSignal
 * @returns PerceptionResult
 */
export function analyzeTextAsync(
  htmlContent: string,
  signal?: AbortSignal
): Promise<PerceptionResult> {
  // 测试或 SSR 环境降级：直接走同步分析
  if (typeof Worker === 'undefined') {
    if (signal?.aborted) {
      return Promise.reject(signal.reason);
    }
    return import('./textAnalyzer').then(({ analyzeText }) => analyzeText(htmlContent));
  }

  const id = nextTaskId++;
  return new Promise<PerceptionResult>((resolve, reject) => {
    if (signal?.aborted) {
      reject(signal.reason);
      return;
    }

    pendingTasks.set(id, { resolve, reject, signal });

    const cleanup = () => pendingTasks.delete(id);
    const handleAbort = () => {
      cleanup();
      // 通知 Worker 取消该任务，避免无意义回传
      if (typeof Worker !== 'undefined') {
        try {
          getWorker().postMessage({ type: 'cancel', id });
        } catch {
          // ignore
        }
      }
      reject(signal!.reason);
    };
    signal?.addEventListener('abort', handleAbort, { once: true });

    try {
      getWorker().postMessage({ type: 'analyze', id, htmlContent });
    } catch (err) {
      pendingTasks.delete(id);
      signal?.removeEventListener('abort', handleAbort);
      reject(err instanceof Error ? err : new Error(String(err)));
    }
  });
}
