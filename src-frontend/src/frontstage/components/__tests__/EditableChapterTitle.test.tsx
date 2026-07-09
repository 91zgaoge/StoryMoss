import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import EditableChapterTitle from '../EditableChapterTitle';

describe('EditableChapterTitle', () => {
  const onRename = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('显示 displayTitle', () => {
    render(
      <EditableChapterTitle displayTitle="第一章" canRename onRename={onRename} variant="heading" />
    );
    expect(screen.getByText('第一章')).toBeInTheDocument();
  });

  it('双击进入编辑；清空后 blur 不调用 onRename', async () => {
    const user = userEvent.setup();
    render(
      <EditableChapterTitle displayTitle="第一章" canRename onRename={onRename} variant="heading" />
    );
    await user.dblClick(screen.getByText('第一章'));
    const input = screen.getByLabelText('编辑章节名称');
    await user.clear(input);
    await user.tab();
    await waitFor(() => {
      expect(onRename).not.toHaveBeenCalled();
    });
    expect(screen.getByText('第一章')).toBeInTheDocument();
  });

  it('双击改名后 Enter 提交', async () => {
    const user = userEvent.setup();
    render(
      <EditableChapterTitle displayTitle="第一章" canRename onRename={onRename} variant="heading" />
    );
    await user.dblClick(screen.getByText('第一章'));
    const input = screen.getByLabelText('编辑章节名称');
    await user.clear(input);
    await user.type(input, '开端{Enter}');
    await waitFor(() => {
      expect(onRename).toHaveBeenCalledWith('开端');
    });
  });

  it('canRename=false 时双击不进入编辑', async () => {
    const user = userEvent.setup();
    render(
      <EditableChapterTitle
        displayTitle="第1章"
        canRename={false}
        onRename={onRename}
        variant="heading"
      />
    );
    await user.dblClick(screen.getByText('第1章'));
    expect(screen.queryByLabelText('编辑章节名称')).not.toBeInTheDocument();
  });

  it('Esc 取消编辑', async () => {
    const user = userEvent.setup();
    render(
      <EditableChapterTitle displayTitle="第一章" canRename onRename={onRename} variant="heading" />
    );
    await user.dblClick(screen.getByText('第一章'));
    const input = screen.getByLabelText('编辑章节名称');
    await user.type(input, 'x');
    await user.keyboard('{Escape}');
    expect(onRename).not.toHaveBeenCalled();
    expect(screen.getByText('第一章')).toBeInTheDocument();
  });
});
