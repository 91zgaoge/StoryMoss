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

  it('传入 onAutoName 时显示 AI 取名按钮并触发回调', async () => {
    const user = userEvent.setup();
    const onAutoName = vi.fn().mockResolvedValue(undefined);
    render(
      <EditableChapterTitle
        displayTitle="临江夜雨"
        canRename
        onRename={onRename}
        onAutoName={onAutoName}
        variant="heading"
      />
    );
    const button = screen.getByLabelText('AI 自动取名');
    await user.click(button);
    expect(onAutoName).toHaveBeenCalledTimes(1);
  });

  it('自动取名进行中禁用按钮', () => {
    render(
      <EditableChapterTitle
        displayTitle="临江夜雨"
        canRename
        onRename={onRename}
        onAutoName={vi.fn()}
        autoNaming
        variant="heading"
      />
    );
    expect(screen.getByLabelText('AI 自动取名')).toBeDisabled();
  });

  it('status 变体不显示 AI 取名按钮', () => {
    render(
      <EditableChapterTitle
        displayTitle="临江夜雨"
        canRename
        onRename={onRename}
        onAutoName={vi.fn()}
        variant="status"
      />
    );
    expect(screen.queryByLabelText('AI 自动取名')).not.toBeInTheDocument();
  });
});
