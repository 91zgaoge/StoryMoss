import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import React from 'react';

// useExecutionState 走 '@/services/tauri'  barrel，而 services/api/stories.ts
// 直接引用 './core'，两处必须指向同一个 mock。
const { loggedInvokeMock } = vi.hoisted(() => ({ loggedInvokeMock: vi.fn() }));

vi.mock('@/services/tauri', () => ({ loggedInvoke: loggedInvokeMock }));
vi.mock('@/services/api/core', () => ({ loggedInvoke: loggedInvokeMock }));

import { loggedInvoke } from '@/services/tauri';
import { useExecutionState } from '../useExecutionState';

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

function mockBackend(canonical: unknown) {
  vi.mocked(loggedInvoke).mockImplementation(async (cmd: string) => {
    if (cmd === 'get_canonical_state') {
      if (canonical instanceof Error) throw canonical;
      return canonical;
    }
    if (cmd === 'get_story_word_count') return { total_chars: 4321, scene_count: 0 };
    return [];
  });
}

describe('useExecutionState', () => {
  beforeEach(() => {
    vi.mocked(loggedInvoke).mockReset();
  });

  it('从 get_canonical_state 取叙事阶段与逾期伏笔（不再走本地兜底）', async () => {
    mockBackend({
      narrative_phase: 'Climax',
      story_context: {
        overdue_payoffs: [{ foreshadowing_id: 'f1' }, { foreshadowing_id: 'f2' }],
      },
    });

    const { result } = renderHook(() => useExecutionState('story-1'), { wrapper });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(loggedInvoke).toHaveBeenCalledWith('get_canonical_state', { story_id: 'story-1' });
    expect(result.current.state.narrativePhase).toBe('Climax');
    expect(result.current.state.overduePayoffs).toBe(2);
    expect(result.current.state.totalWordCount).toBe(4321);
  });

  it('未收录的阶段值降级为 Setup', async () => {
    mockBackend({ narrative_phase: 'Epilogue', story_context: { overdue_payoffs: [] } });

    const { result } = renderHook(() => useExecutionState('story-1'), { wrapper });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.state.narrativePhase).toBe('Setup');
  });

  it('命令失败时静默降级，不抛错也不卡住加载态', async () => {
    mockBackend(new Error('Command get_canonical_state not found'));

    const { result } = renderHook(() => useExecutionState('story-1'), { wrapper });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.state.narrativePhase).toBe('Setup');
    expect(result.current.state.overduePayoffs).toBe(0);
  });
});
