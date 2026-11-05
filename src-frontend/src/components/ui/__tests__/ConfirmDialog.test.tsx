import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ConfirmDialog } from '../ConfirmDialog';

describe('ConfirmDialog', () => {
  it('确认/取消各触发一次回调', async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        open
        onConfirm={onConfirm}
        onCancel={onCancel}
        title="确定要删除这个故事吗？此操作不可撤销。"
      />
    );
    await user.click(screen.getByRole('button', { name: '确认' }));
    await user.click(screen.getByRole('button', { name: '取消' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).toHaveBeenCalledTimes(1);
    // 危险确认文案原样保留（附录 B2 契约）
    expect(screen.getByText('确定要删除这个故事吗？此操作不可撤销。')).toBeInTheDocument();
  });

  it('danger 时确认按钮为危险样式，description 展示', () => {
    render(
      <ConfirmDialog
        open
        onConfirm={() => {}}
        onCancel={() => {}}
        title="删除"
        description="引用它的故事将恢复为无方法论。"
      />
    );
    expect(screen.getByRole('button', { name: '确认' })).toHaveClass('text-status-danger');
    expect(screen.getByText('引用它的故事将恢复为无方法论。')).toBeInTheDocument();
  });

  it('loading 时按钮禁用', () => {
    render(<ConfirmDialog open onConfirm={() => {}} onCancel={() => {}} title="删除" loading />);
    expect(screen.getByRole('button', { name: '确认' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled();
  });
});
