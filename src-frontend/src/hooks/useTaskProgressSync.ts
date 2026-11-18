/**
 * v0.98.9: 任务进度事件 → 任务列表 + 执行日志同步刷新。
 *
 * 后端 `update_progress` 一次写入 `tasks.progress`、心跳与一条
 * `task_logs`（三者共用同一个 now，见 v0.93.14）。前端此前只轮询任务列表，
 * `task_logs` 仅在 status 变化时才失效——展开的日志面板会停在展开时刻的
 * 进度（真机表现为头部 40%、日志最新一条仍是 5%）。这里让每个进度事件
 * 同时失效两份查询，任务行与日志在下一次渲染时使用同一批数据。
 */
import { useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useQueryClient } from '@tanstack/react-query';
import { createLogger } from '@/utils/logger';

interface TaskProgressPayload {
  task_id?: string;
  step?: string;
  progress?: number;
  message?: string;
  timestamp?: string;
}

export function useTaskProgressSync() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    void listen<TaskProgressPayload>('task-progress', event => {
      const taskId = event.payload?.task_id;
      if (!taskId) return;
      void queryClient.invalidateQueries({ queryKey: ['tasks'] });
      void queryClient.invalidateQueries({ queryKey: ['task_logs', taskId] });
    })
      .then(fn => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(error => {
        createLogger('ui:App').error('Failed to setup task progress listener', { error });
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient]);
}
