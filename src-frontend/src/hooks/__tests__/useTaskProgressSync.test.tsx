/**
 * v0.98.9: 任务进度事件必须同时刷新任务列表与执行日志。
 *
 * 回归场景：任务详情展开后，头部进度随 5s 轮询更新到 40%，但日志面板
 * 只在 status 变化时刷新，最新一条仍是展开时的「进度 5%」。
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import type { ReactNode } from 'react';
import { useTaskProgressSync } from '../useTaskProgressSync';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

function setup() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries').mockResolvedValue(undefined);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  renderHook(() => useTaskProgressSync(), { wrapper });
  return { invalidate };
}

describe('useTaskProgressSync', () => {
  beforeEach(() => {
    vi.mocked(listen).mockClear();
  });

  it('task-progress 同时失效任务列表与该任务日志', async () => {
    const { invalidate } = setup();
    await waitFor(() => expect(listen).toHaveBeenCalledWith('task-progress', expect.any(Function)));
    const callback = vi.mocked(listen).mock.calls[0][1] as (e: { payload: unknown }) => void;

    act(() => callback({ payload: { task_id: 'task-1', progress: 40 } }));

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['tasks'] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['task_logs', 'task-1'] });
  });

  it('缺失 task_id 的事件不触发刷新', async () => {
    const { invalidate } = setup();
    await waitFor(() => expect(listen).toHaveBeenCalled());
    const callback = vi.mocked(listen).mock.calls[0][1] as (e: { payload: unknown }) => void;

    act(() => callback({ payload: { progress: 40 } }));

    expect(invalidate).not.toHaveBeenCalled();
  });
});
