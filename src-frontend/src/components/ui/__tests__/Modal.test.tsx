import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Modal } from '../Modal';

describe('Modal', () => {
  it('open 时渲染标题与内容，closed 时不渲染', () => {
    const { rerender } = render(
      <Modal open onClose={() => {}} title="删除故事">
        <p>正文</p>
      </Modal>
    );
    expect(screen.getByRole('dialog', { name: '删除故事' })).toBeInTheDocument();
    expect(screen.getByText('正文')).toBeInTheDocument();

    rerender(
      <Modal open={false} onClose={() => {}} title="删除故事">
        <p>正文</p>
      </Modal>
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('Esc 关闭', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="t">
        <button>内</button>
      </Modal>
    );
    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('点击遮罩关闭，点击面板内不关闭', async () => {
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="t">
        <p>正文</p>
      </Modal>
    );
    const overlay = screen.getByRole('dialog').parentElement!;
    overlay.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    expect(onClose).toHaveBeenCalledTimes(1);

    onClose.mockClear();
    screen.getByText('正文').dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('danger 变体标题为危险色，内容区限高滚动', () => {
    render(
      <Modal open onClose={() => {}} title="危险" danger>
        <p>正文</p>
      </Modal>
    );
    expect(screen.getByText('危险')).toHaveClass('text-status-danger');
    expect(screen.getByText('正文').parentElement).toHaveClass('overflow-y-auto');
    expect(screen.getByRole('dialog')).toHaveClass('max-h-[85dvh]');
  });
});
