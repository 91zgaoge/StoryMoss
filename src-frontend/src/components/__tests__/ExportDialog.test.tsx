import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ExportDialog } from '../ExportDialog';

const mutate = vi.fn();

vi.mock('@/hooks/useExport', () => ({
  useExport: () => ({ mutate, isPending: false }),
  useExportTemplates: () => ({ data: [] }),
}));

vi.mock('@/services/tauri', () => ({
  antiAiReview: vi.fn().mockResolvedValue(null),
  getStoryChapters: vi.fn().mockResolvedValue([]),
  logFeatureUsage: vi.fn(),
}));

describe('ExportDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('走共享 Modal：标题、格式列表与关闭入口都在', () => {
    const onClose = vi.fn();
    render(<ExportDialog storyId="story-1" storyTitle="测试故事" isOpen onClose={onClose} />);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('导出故事')).toBeInTheDocument();
    expect(screen.getByText('测试故事')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Markdown/ })).toBeInTheDocument();

    const user = userEvent.setup();
    return user.click(screen.getByLabelText('关闭')).then(() => expect(onClose).toHaveBeenCalled());
  });

  it('导出按钮提交当前格式与选项', async () => {
    const user = userEvent.setup();
    render(<ExportDialog storyId="story-1" storyTitle="测试故事" isOpen onClose={vi.fn()} />);

    await user.click(screen.getByRole('button', { name: '导出' }));
    expect(mutate).toHaveBeenCalledWith(
      expect.objectContaining({
        story_id: 'story-1',
        format: 'markdown',
        include_metadata: true,
        include_outline: true,
        include_characters: true,
      }),
      expect.any(Object)
    );
  });

  it('Anti-AI 体检步骤仍在 Modal 内切换', async () => {
    const user = userEvent.setup();
    render(<ExportDialog storyId="story-1" storyTitle="测试故事" isOpen onClose={vi.fn()} />);

    await user.click(screen.getByRole('button', { name: /出版前 Anti-AI 体检/ }));
    expect(screen.getByText('出版前体检')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '运行审查' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '跳过，直接导出' })).toBeInTheDocument();
  });

  it('isOpen 为 false 时不渲染', () => {
    render(
      <ExportDialog storyId="story-1" storyTitle="测试故事" isOpen={false} onClose={vi.fn()} />
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
