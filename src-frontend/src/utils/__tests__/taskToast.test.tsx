/**
 * v0.93.12: 任务通知「查看任务」跳转契约。
 *
 * 任务完成/失败 toast 必须能一键落到「全部任务」页的具体任务行；
 * 生成任务（拆书/指导书/训练）创建后同样走这一入口。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppStore } from '@/stores/appStore';

const { customMock, dismissMock } = vi.hoisted(() => ({
  customMock: vi.fn((_node: unknown, _options?: unknown) => 'task-toast-1'),
  dismissMock: vi.fn(),
}));

vi.mock('react-hot-toast', () => ({
  default: {
    custom: customMock,
    dismiss: dismissMock,
    success: vi.fn(),
    error: vi.fn(),
  },
}));

import { focusTaskInAllTasks, showTaskToast } from '../taskToast';

describe('taskToast', () => {
  beforeEach(() => {
    customMock.mockClear();
    dismissMock.mockClear();
    useAppStore.setState({ currentView: 'dashboard', pendingTaskFocusId: null });
  });

  it('focusTaskInAllTasks 写入待聚焦 ID 并切到全部任务页', () => {
    focusTaskInAllTasks('task-abc');
    const state = useAppStore.getState();
    expect(state.pendingTaskFocusId).toBe('task-abc');
    expect(state.currentView).toBe('tasks');
  });

  it('showTaskToast 点击「查看任务」后关闭 toast 并聚焦任务', () => {
    showTaskToast({
      taskId: 'task-abc',
      title: '后台任务失败',
      message: '续写管线失败',
      tone: 'error',
    });

    expect(customMock).toHaveBeenCalledTimes(1);
    const element = customMock.mock.calls[0][0] as unknown as {
      props: { title: string; message?: string; onOpen: () => void };
    };
    expect(element.props.title).toBe('后台任务失败');
    expect(element.props.message).toBe('续写管线失败');

    element.props.onOpen();

    expect(dismissMock).toHaveBeenCalledWith('task-toast-1');
    const state = useAppStore.getState();
    expect(state.currentView).toBe('tasks');
    expect(state.pendingTaskFocusId).toBe('task-abc');
  });
});
