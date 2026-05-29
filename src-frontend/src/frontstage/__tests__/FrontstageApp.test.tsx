import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FrontstageApp from '../FrontstageApp';

// Mock Tauri API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock Tauri services
vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn(),
  recordFeedback: vi.fn(),
  smartExecute: vi.fn(),
  getInputHint: vi.fn(),
  runRefine: vi.fn(),
  runReview: vi.fn(),
  runFinalize: vi.fn(),
  getPipelineActiveDraft: vi.fn(),
}));

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
    render(<FrontstageApp />);

    // Header 应该存在
    expect(screen.getByText('草苔')).toBeInTheDocument();

    // Sidebar 按钮应该存在
    expect(screen.getByTitle('修订模式')).toBeInTheDocument();
    expect(screen.getByTitle('打开幕后工作室')).toBeInTheDocument();

    // BottomBar 应该存在
    expect(screen.getByPlaceholderText('输入任意指令…')).toBeInTheDocument();

    // RichTextEditor 应该存在
    await waitFor(() => {
      expect(screen.getByTestId('rich-text-editor')).toBeInTheDocument();
    });
  });

  it('不应该渲染窥视面板按钮（已移除）', () => {
    render(<FrontstageApp />);
    expect(screen.queryByTitle('窥视面板')).not.toBeInTheDocument();
    expect(screen.queryByTitle(/窥视/)).not.toBeInTheDocument();
  });

  it('点击禅模式按钮应该进入禅模式并隐藏干扰元素', async () => {
    render(<FrontstageApp />);

    const zenBtn = screen.getByTitle('F11 禅模式');
    await userEvent.click(zenBtn);

    // 禅模式下 Header 右侧按钮应该消失
    await waitFor(() => {
      expect(screen.queryByTitle('F11 禅模式')).not.toBeInTheDocument();
    });

    // 禅模式下 Sidebar 应该消失
    expect(screen.queryByTitle('修订模式')).not.toBeInTheDocument();

    // 禅模式下 BottomBar 应该消失
    expect(screen.queryByPlaceholderText('输入任意指令…')).not.toBeInTheDocument();

    // 应该出现退出禅模式按钮
    expect(screen.getByText('退出禅模式 (F11)')).toBeInTheDocument();
  });

  it('禅模式下点击退出按钮应该恢复正常布局', async () => {
    render(<FrontstageApp />);

    // 进入禅模式
    await userEvent.click(screen.getByTitle('F11 禅模式'));
    await waitFor(() => {
      expect(screen.queryByTitle('F11 禅模式')).not.toBeInTheDocument();
    });

    // 退出禅模式
    await userEvent.click(screen.getByText('退出禅模式 (F11)'));

    // 恢复正常
    await waitFor(() => {
      expect(screen.getByTitle('F11 禅模式')).toBeInTheDocument();
      expect(screen.getByTitle('修订模式')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('输入任意指令…')).toBeInTheDocument();
    });
  });

  it('点击修订模式按钮应该切换状态', async () => {
    render(<FrontstageApp />);

    const revisionBtn = screen.getByTitle('修订模式');
    expect(revisionBtn).not.toHaveClass('active');

    await userEvent.click(revisionBtn);
    await waitFor(() => {
      expect(screen.getByTitle('修订模式')).toHaveClass('active');
    });

    await userEvent.click(revisionBtn);
    await waitFor(() => {
      expect(screen.getByTitle('修订模式')).not.toHaveClass('active');
    });
  });
});
