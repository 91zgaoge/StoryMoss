import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { confirmDialog, ConfirmPortal } from '../confirm';

describe('confirmDialog', () => {
  afterEach(() => vi.restoreAllMocks());

  it('未挂 ConfirmPortal 时回退 window.confirm（单测/组件隔离渲染不破）', async () => {
    const spy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    await expect(confirmDialog({ title: '确定要删除吗？' })).resolves.toBe(false);
    expect(spy).toHaveBeenCalledWith('确定要删除吗？');
  });

  it('ConfirmPortal 挂载后走统一 ConfirmDialog，确认/取消各自归还布尔', async () => {
    const user = userEvent.setup();
    render(<ConfirmPortal />);

    const confirmed = confirmDialog({ title: '确定要删除这个故事吗？此操作不可撤销。' });
    expect(await screen.findByText('确定要删除这个故事吗？此操作不可撤销。')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '确认' }));
    await expect(confirmed).resolves.toBe(true);

    const cancelled = confirmDialog({ title: '删除这条语料及其文件？' });
    await screen.findByText('删除这条语料及其文件？');
    await user.click(screen.getByRole('button', { name: '取消' }));
    await expect(cancelled).resolves.toBe(false);

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});
