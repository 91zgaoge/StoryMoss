import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StyleDnaModal } from '../StyleDnaModal';
import type { Story, StyleDNA } from '@/types/index';

const setStoryStyleDna = vi.fn().mockResolvedValue(undefined);
const setStoryStyleBlend = vi.fn().mockResolvedValue({});
const getStoryStyleBlend = vi.fn().mockResolvedValue(null);
const setCurrentStory = vi.fn();

vi.mock('@/services/tauri', () => ({
  setStoryStyleDna: (...args: unknown[]) => setStoryStyleDna(...args),
  setStoryStyleBlend: (...args: unknown[]) => setStoryStyleBlend(...args),
  getStoryStyleBlend: (...args: unknown[]) => getStoryStyleBlend(...args),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: {
    getState: () => ({
      currentStory: { id: 'story-1', title: '测试故事' },
      setCurrentStory,
    }),
  },
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

const story: Story = {
  id: 'story-1',
  title: '测试故事',
  style_dna_id: 'dna-1',
  created_at: '2026-01-01T00:00:00+08:00',
  updated_at: '2026-01-01T00:00:00+08:00',
};

const styleDnas: StyleDNA[] = [
  { id: 'dna-1', name: '海明威', is_builtin: true, is_user_created: false },
  { id: 'dna-2', name: '张爱玲', is_builtin: true, is_user_created: false },
];

function renderModal(onClose = vi.fn()) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <StyleDnaModal story={story} styleDnas={styleDnas} onClose={onClose} onOpenSample={vi.fn()} />
    </QueryClientProvider>
  );
  return { onClose };
}

describe('StyleDnaModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getStoryStyleBlend.mockResolvedValue(null);
    setStoryStyleDna.mockResolvedValue(undefined);
    setStoryStyleBlend.mockResolvedValue({});
  });

  it('单一风格先选中、再点确定才保存', async () => {
    const user = userEvent.setup();
    const { onClose } = renderModal();

    // 当前已保存的风格被标记为选中
    expect(screen.getByTestId('style-dna-option-dna-1')).toHaveAttribute('aria-pressed', 'true');
    expect(setStoryStyleDna).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('style-dna-option-dna-2'));
    expect(screen.getByTestId('style-dna-option-dna-2')).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByTestId('style-dna-option-dna-1')).toHaveAttribute('aria-pressed', 'false');
    expect(setStoryStyleDna).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('style-dna-confirm'));
    await waitFor(() => expect(setStoryStyleDna).toHaveBeenCalledWith('story-1', 'dna-2'));
    expect(setCurrentStory).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'story-1', style_dna_id: 'dna-2' })
    );
    expect(onClose).toHaveBeenCalled();
  });

  it('关闭按钮直接退出，不落库', async () => {
    const user = userEvent.setup();
    const { onClose } = renderModal();

    await user.click(screen.getByTestId('style-dna-close'));
    expect(onClose).toHaveBeenCalled();
    expect(setStoryStyleDna).not.toHaveBeenCalled();
  });

  it('弹窗限高且内容区可滚动（保存按钮不会被挤出视口）', () => {
    renderModal();
    const dialog = screen.getByTestId('style-dna-modal');
    const card = dialog.firstElementChild as HTMLElement;
    expect(card.className).toContain('max-h-[calc(100dvh-2rem)]');
    expect(card.className).toContain('overflow-hidden');
    expect(screen.getByTestId('style-dna-confirm')).toBeInTheDocument();
    expect(screen.getByTestId('style-dna-cancel')).toBeInTheDocument();
  });

  it('风格混合页签展示固定底部的保存/取消，并可保存配置', async () => {
    const user = userEvent.setup();
    const { onClose } = renderModal();

    await user.click(screen.getByTestId('style-dna-tab-blend'));
    await waitFor(() => expect(getStoryStyleBlend).toHaveBeenCalledWith('story-1'));

    const saveButton = screen.getByRole('button', { name: '保存配置' });
    expect(saveButton.closest('.sticky')).not.toBeNull();

    // 添加一个风格后权重自动归一为 100%，配置合法
    await user.click(screen.getByRole('button', { name: /张爱玲/ }));
    await user.click(saveButton);

    await waitFor(() => expect(setStoryStyleBlend).toHaveBeenCalled());
    expect(onClose).toHaveBeenCalled();
  });

  it('已有混合配置异步加载后回填到面板，而不是停在空白默认值', async () => {
    getStoryStyleBlend.mockResolvedValueOnce({
      blend: {
        name: '混合甲',
        components: [{ dna_id: 'dna-1', dna_name: '海明威', weight: 1, role: 'dominant' }],
        drift_check_enabled: false,
      },
    });
    const user = userEvent.setup();
    renderModal();

    await user.click(screen.getByTestId('style-dna-tab-blend'));
    await waitFor(() => expect(screen.getByDisplayValue('混合甲')).toBeInTheDocument());
    expect(screen.getByText('主导')).toBeInTheDocument();
  });
});
