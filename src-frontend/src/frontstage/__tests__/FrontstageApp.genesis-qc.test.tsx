import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// v0.30.35：捕获每个事件名的 listen 回调，便于主动触发 genesis-qc-result。
// 用 vi.hoisted 确保 Map 在被提升的 vi.mock 工厂执行时已初始化。
const { listenCallbacks } = vi.hoisted(() => ({
  listenCallbacks: new Map<string, (e: { payload: unknown }) => void>(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    listenCallbacks.set(event, cb);
    return Promise.resolve(() => {});
  }),
}));

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn((cmd: string) => {
    if (cmd === 'get_gateway_status') {
      return Promise.resolve({
        last_probe_at: undefined,
        primary_model_id: undefined,
        models: [],
        is_probing: false,
      });
    }
    return Promise.resolve(undefined);
  }),
  recordFeedback: vi.fn(),
  smartExecute: vi.fn(),
  getInputHint: vi.fn(),
  runRefine: vi.fn(),
  runReview: vi.fn(),
  runFinalize: vi.fn(),
  getPipelineActiveDraft: vi.fn(),
}));

vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: function MockRichTextEditor() {
    return React.createElement('div', { 'data-testid': 'rich-text-editor' }, '编辑器内容');
  },
}));

vi.mock('../components/IngestHealthIndicator', () => ({
  IngestHealthIndicator: function MockIngestHealthIndicator() {
    return null;
  },
}));

vi.mock('@/hooks/useSubscription', () => ({ useSubscription: () => ({ isPro: false }) }));
vi.mock('@/hooks/useSyncStore', () => ({ useSyncStore: () => {} }));
vi.mock('@/hooks/usePipelineProgress', () => ({
  usePipelineProgress: () => ({ data: null }),
  usePipelineComplete: () => null,
}));
vi.mock('@/hooks/useCharacters', () => ({ useCharacters: () => ({ data: [] }) }));
vi.mock('@/hooks/useSettings', () => ({
  useSettings: () => ({ data: null }),
  useModels: () => ({ data: [] }),
}));
vi.mock('@/stores/modelConnectionStore', () => ({
  useModelConnectionStore: () => ({ states: {} }),
}));
vi.mock('react-hot-toast', () => ({ default: { success: vi.fn(), error: vi.fn() } }));

import FrontstageApp from '../FrontstageApp';
import { useGenerationStore } from '@/stores/generationStore';

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});
const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

/** 触发 genesis-qc-result 回调并读取写入 orchestratorStatus 的 toast 文案。 */
async function fireQc(payload: Record<string, unknown>): Promise<string> {
  const cb = listenCallbacks.get('genesis-qc-result');
  cb!({ payload });
  return useGenerationStore.getState().orchestratorStatus?.message ?? '';
}

describe('FrontstageApp genesis-qc-result 事件（v0.30.35 后台质检反馈）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenCallbacks.clear();
    useGenerationStore.getState().setOrchestratorStatus(null);
  });

  it('注册 genesis-qc-result 监听', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => {
      expect(listenCallbacks.has('genesis-qc-result')).toBe(true);
    });
  });

  it('质检通过显示成功提示', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => {
      expect(listenCallbacks.has('genesis-qc-result')).toBe(true);
    });
    const msg = await fireQc({ story_id: 's1', passed: true, salvaged: false });
    expect(msg).toContain('编辑审计质检通过');
  });

  it('质检降级放行显示警告提示', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => {
      expect(listenCallbacks.has('genesis-qc-result')).toBe(true);
    });
    const msg = await fireQc({
      story_id: 's1',
      passed: true,
      salvaged: true,
      reason: '审计超时',
    });
    expect(msg).toContain('降级放行');
  });

  it('质检不合格显示问题清单', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => {
      expect(listenCallbacks.has('genesis-qc-result')).toBe(true);
    });
    const msg = await fireQc({
      story_id: 's1',
      passed: false,
      salvaged: false,
      issues: ['主角动机缺失', '开篇拖沓'],
    });
    expect(msg).toContain('质检不合格');
    expect(msg).toContain('主角动机缺失');
  });

  // v0.59.0 续写质检闭环：mode=continue 时不得再提示「重新创世」（那是创世动作），
  // 而是把问题挂到可操作条，用户可以一键按审查意见修订本章。
  it('续写质检不合格：不提示重新创世，出现可修订操作条', async () => {
    const { container } = render(<FrontstageApp />, { wrapper });
    await waitFor(() => {
      expect(listenCallbacks.has('genesis-qc-result')).toBe(true);
    });
    const msg = await fireQc({
      story_id: 's1',
      passed: false,
      salvaged: false,
      issues: ['死人复活', '场景重复'],
      mode: 'continue',
      chapter_number: 7,
    });
    expect(msg).not.toContain('重新创世');
    expect(msg).toContain('修订');
    await waitFor(() => {
      expect(container.querySelector('[data-testid="qc-revise-bar"]')).not.toBeNull();
    });
    expect(container.querySelector('[data-testid="qc-revise-button"]')).not.toBeNull();
  });

  it('续写质检通过：文案区分「本拍」而非创世首章', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => {
      expect(listenCallbacks.has('genesis-qc-result')).toBe(true);
    });
    const msg = await fireQc({
      story_id: 's1',
      passed: true,
      salvaged: false,
      mode: 'continue',
    });
    expect(msg).toContain('本拍质检通过');
  });
});
