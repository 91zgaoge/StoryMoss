import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FrontstageSidebar from '../FrontstageSidebar';

describe('FrontstageSidebar', () => {
  const defaultProps = {
    isZenMode: false,
    isRevisionMode: false,
    hasCurrentStory: true,
    onToggleRevisionMode: vi.fn(),
    onGenerateCommentary: vi.fn(),
    onOpenBackstage: vi.fn(),
  };

  it('应该渲染三个主要按钮', () => {
    render(<FrontstageSidebar {...defaultProps} />);

    expect(screen.getByTitle('修订模式')).toBeInTheDocument();
    expect(screen.getByTitle('生成古典评点')).toBeInTheDocument();
    expect(screen.getByTitle('打开幕后工作室')).toBeInTheDocument();
  });

  it('不应该显示窥视面板按钮（已移除）', () => {
    render(<FrontstageSidebar {...defaultProps} />);

    expect(screen.queryByTitle('窥视面板')).not.toBeInTheDocument();
    expect(screen.queryByTitle(/窥视/)).not.toBeInTheDocument();
  });

  it('禅模式下应该完全隐藏', () => {
    const { container } = render(<FrontstageSidebar {...defaultProps} isZenMode={true} />);

    expect(container.firstChild).toBeNull();
  });

  it('点击修订模式按钮应该触发回调', async () => {
    const onToggleRevisionMode = vi.fn();
    render(<FrontstageSidebar {...defaultProps} onToggleRevisionMode={onToggleRevisionMode} />);

    await userEvent.click(screen.getByTitle('修订模式'));
    expect(onToggleRevisionMode).toHaveBeenCalledTimes(1);
  });

  it('修订模式激活时按钮应该有 active 样式', () => {
    render(<FrontstageSidebar {...defaultProps} isRevisionMode={true} />);

    const revisionBtn = screen.getByTitle('修订模式');
    expect(revisionBtn).toHaveClass('active');
  });

  it('点击幕后工作室按钮应该触发回调', async () => {
    const onOpenBackstage = vi.fn();
    render(<FrontstageSidebar {...defaultProps} onOpenBackstage={onOpenBackstage} />);

    await userEvent.click(screen.getByTitle('打开幕后工作室'));
    expect(onOpenBackstage).toHaveBeenCalledTimes(1);
  });

  it('没有当前故事时，生成古典评点按钮应该被禁用', () => {
    render(<FrontstageSidebar {...defaultProps} hasCurrentStory={false} />);

    const commentaryBtn = screen.getByTitle('生成古典评点');
    expect(commentaryBtn).toBeDisabled();
  });
});
