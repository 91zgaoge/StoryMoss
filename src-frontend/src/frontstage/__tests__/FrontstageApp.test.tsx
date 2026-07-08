import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from '../FrontstageApp';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

// Mock Tauri API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock Tauri services
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

import { loggedInvoke, smartExecute } from '@/services/tauri';

// Mock RichTextEditor (TipTap 在 jsdom 中无法运行)
vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: function MockRichTextEditor() {
    const React = require('react');
    return React.createElement('div', { 'data-testid': 'rich-text-editor' }, '编辑器内容');
  },
}));

// Mock IngestHealthIndicator — 它内部使用 Tauri listen API，在 jsdom 中无法运行
vi.mock('../components/IngestHealthIndicator', () => ({
  IngestHealthIndicator: function MockIngestHealthIndicator() {
    return null;
  },
}));

// Mock hooks
vi.mock('@/hooks/useSubscription', () => ({
  useSubscription: () => ({ isPro: false }),
}));

vi.mock('@/hooks/useSyncStore', () => ({
  useSyncStore: () => {},
}));

vi.mock('@/hooks/usePipelineProgress', () => ({
  usePipelineProgress: () => ({ data: null }),
  usePipelineComplete: () => null,
}));

vi.mock('@/hooks/useCharacters', () => ({
  useCharacters: () => ({ data: [] }),
}));

vi.mock('@/hooks/useSettings', () => ({
  useSettings: () => ({ data: null }),
  useModels: () => ({ data: [] }),
}));

vi.mock('@/stores/modelConnectionStore', () => ({
  useModelConnectionStore: () => ({ states: {} }),
}));

vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

describe('FrontstageApp', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应该渲染核心布局组件', async () => {
    render(<FrontstageApp />, { wrapper });

    // Header 应该存在
    expect(screen.getByText('草苔')).toBeInTheDocument();

    // 设置 / 幕后工作室按钮应该存在（已移到顶部色调设置旁边）
    expect(screen.getByTitle('打开设置 / 幕后工作室')).toBeInTheDocument();

    // BottomBar 应该存在
    expect(screen.getByPlaceholderText('输入任意指令…')).toBeInTheDocument();

    // RichTextEditor 应该存在
    await waitFor(() => {
      expect(screen.getByTestId('rich-text-editor')).toBeInTheDocument();
    });

    // 左侧边栏已完全删除
    expect(screen.queryByTitle('修订模式')).not.toBeInTheDocument();
    expect(screen.queryByTitle('生成古典评点')).not.toBeInTheDocument();
  });

  it('不应该渲染窥视面板按钮（已移除）', () => {
    render(<FrontstageApp />, { wrapper });
    expect(screen.queryByTitle('窥视面板')).not.toBeInTheDocument();
    expect(screen.queryByTitle(/窥视/)).not.toBeInTheDocument();
  });

  it('点击禅模式按钮应该进入禅模式并隐藏干扰元素', async () => {
    render(<FrontstageApp />, { wrapper });

    const zenBtn = screen.getByTitle('进入全屏禅写模式（F11）');
    await userEvent.click(zenBtn);

    // 禅模式下 Header 右侧按钮应该消失
    await waitFor(() => {
      expect(screen.queryByTitle('进入全屏禅写模式（F11）')).not.toBeInTheDocument();
    });

    // 禅模式下 BottomBar 应该消失
    expect(screen.queryByPlaceholderText('输入任意指令…')).not.toBeInTheDocument();

    // 应该出现退出禅模式按钮
    expect(screen.getByText('退出禅模式 (F11)')).toBeInTheDocument();
  });

  it('禅模式下点击退出按钮应该恢复正常布局', async () => {
    render(<FrontstageApp />, { wrapper });

    // 进入禅模式
    await userEvent.click(screen.getByTitle('进入全屏禅写模式（F11）'));
    await waitFor(() => {
      expect(screen.queryByTitle('进入全屏禅写模式（F11）')).not.toBeInTheDocument();
    });

    // 退出禅模式
    await userEvent.click(screen.getByText('退出禅模式 (F11)'));

    // 恢复正常
    await waitFor(() => {
      expect(screen.getByTitle('进入全屏禅写模式（F11）')).toBeInTheDocument();
      expect(screen.getByTitle('打开设置 / 幕后工作室')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('输入任意指令…')).toBeInTheDocument();
    });
  });

  it('加载章节后应立即显示当前章节字数', async () => {
    const chapterContent = '<p>这是一个测试章节。</p><p>Second paragraph.</p>';
    vi.mocked(loggedInvoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === 'get_gateway_status') {
        return {
          last_probe_at: undefined,
          primary_model_id: undefined,
          models: [],
          is_probing: false,
        };
      }
      if (cmd === 'list_stories') {
        return [{ id: 'story-1', title: '测试故事' }];
      }
      if (cmd === 'get_story_chapters_paged') {
        return [
          {
            id: 'chapter-1',
            story_id: 'story-1',
            title: '第一章',
            chapter_number: 1,
            content: chapterContent,
          },
        ];
      }
      if (cmd === 'get_story_scenes_paged') {
        return [];
      }
      if (cmd === 'get_story_word_count') {
        return { total_chars: 100 };
      }
      return undefined;
    });

    render(<FrontstageApp />, { wrapper });

    // 等待故事加载并显示章节字数
    await waitFor(() => {
      // 章节加载后应立即显示非零字数（不再等到自动保存成功）
      expect(screen.getByTitle('当前章节字数 / 全文字数')).not.toHaveTextContent(/^0 字/);
    });
    expect(screen.getByTitle('当前章节字数 / 全文字数')).toHaveTextContent(/100 字/);
  });

  it('取消生成应调用 agent_cancel_all_tasks 并清理前端状态', async () => {
    // 让 smartExecute 永远 pending，保持 isGenerating=true
    vi.mocked(smartExecute).mockImplementation(() => new Promise(() => {}));

    render(<FrontstageApp />, { wrapper });

    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '继续写下去');

    // 按 Enter 触发生成
    await userEvent.keyboard('{Enter}');

    // 等待取消按钮出现（生成中状态）
    const cancelBtn = await waitFor(() => screen.getByTitle('取消生成'));

    // 点击取消
    await userEvent.click(cancelBtn);

    // 验证后端取消命令被调用
    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('agent_cancel_all_tasks', {});
    });

    // 验证前端生成状态被清理：输入框重新启用、发送按钮恢复
    await waitFor(() => {
      expect(input).not.toBeDisabled();
    });
    expect(screen.getByTitle('发送')).toBeInTheDocument();
  });
});
