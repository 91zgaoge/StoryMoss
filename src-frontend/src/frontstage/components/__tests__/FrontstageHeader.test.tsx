import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FrontstageHeader from '../FrontstageHeader';

// Mock IngestHealthIndicator — 它内部使用 Tauri listen API，在 jsdom 中无法运行
vi.mock('../IngestHealthIndicator', () => ({
  IngestHealthIndicator: () => null,
}));

describe('FrontstageHeader', () => {
  const defaultProps = {
    currentStory: { id: '1', title: '测试故事' },
    currentChapter: { id: 'c1', story_id: '1', title: '第一章', chapter_number: 1 },
    wordCount: 1234,
    totalWordCount: 5678,
    fontSize: 18,
    isSaved: true,
    isZenMode: false,
    wensiMode: 'passive' as const,
    orchestratorStatus: null,
    bootstrapProgress: null,
    onOpenBackstage: vi.fn(),
    onCycleWensiMode: vi.fn(),
    onToggleZenMode: vi.fn(),
  };

  it('应该显示当前故事名称', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByText('测试故事')).toBeInTheDocument();
  });

  it('默认状态下应该显示"草苔"', () => {
    render(<FrontstageHeader {...defaultProps} currentStory={null} />);
    expect(screen.getByText('草苔')).toBeInTheDocument();
  });

  it('应该显示章节标题和字数统计', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByText('第一章')).toBeInTheDocument();
    expect(screen.getByText(/1234 字/)).toBeInTheDocument();
    expect(screen.getByText(/5678 字/)).toBeInTheDocument();
  });

  it('应该显示字体大小', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByText('18px')).toBeInTheDocument();
  });

  it('未保存时应该显示"保存中..."提示', () => {
    render(<FrontstageHeader {...defaultProps} isSaved={false} />);
    expect(screen.getByText('保存中...')).toBeInTheDocument();
  });

  it('应该显示禅模式按钮和文思模式按钮', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByTitle('F11 禅模式')).toBeInTheDocument();
    expect(screen.getByTitle(/文思/)).toBeInTheDocument();
  });

  it('禅模式下右侧控制按钮应该隐藏', () => {
    render(<FrontstageHeader {...defaultProps} isZenMode={true} />);
    expect(screen.queryByTitle('F11 禅模式')).not.toBeInTheDocument();
    expect(screen.queryByTitle(/文思/)).not.toBeInTheDocument();
  });

  it('点击故事名称应该触发打开幕后工作室', async () => {
    const onOpenBackstage = vi.fn();
    render(<FrontstageHeader {...defaultProps} onOpenBackstage={onOpenBackstage} />);

    await userEvent.click(screen.getByText('测试故事'));
    expect(onOpenBackstage).toHaveBeenCalledTimes(1);
  });

  it('点击禅模式按钮应该触发回调', async () => {
    const onToggleZenMode = vi.fn();
    render(<FrontstageHeader {...defaultProps} onToggleZenMode={onToggleZenMode} />);

    await userEvent.click(screen.getByTitle('F11 禅模式'));
    expect(onToggleZenMode).toHaveBeenCalledTimes(1);
  });

  it('点击文思模式按钮应该触发回调', async () => {
    const onCycleWensiMode = vi.fn();
    render(<FrontstageHeader {...defaultProps} onCycleWensiMode={onCycleWensiMode} />);

    await userEvent.click(screen.getByTitle(/文思/));
    expect(onCycleWensiMode).toHaveBeenCalledTimes(1);
  });

  it('文思活跃模式应该显示正确的提示', () => {
    render(<FrontstageHeader {...defaultProps} wensiMode="active" />);
    expect(screen.getByTitle('文思活跃 — Ctrl+Enter 续写')).toBeInTheDocument();
  });

  it('文思关闭模式应该显示正确的提示', () => {
    render(<FrontstageHeader {...defaultProps} wensiMode="off" />);
    expect(screen.getByTitle('文思关闭')).toBeInTheDocument();
  });
});
