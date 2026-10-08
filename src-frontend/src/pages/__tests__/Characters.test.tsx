import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Characters } from '../Characters';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

const deleteMutate = vi.fn();
const setLifeStatusMutate = vi.fn();

vi.mock('@/services/api/wizard', () => ({
  generateCharacterProfiles: vi.fn(),
}));

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn(),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: any) => any) =>
    selector({
      currentStory: { id: 'story-1', title: 'Test Story' },
    }),
}));

vi.mock('@/hooks/useCharacters', () => ({
  useCharacters: () => ({
    data: [
      { id: 'char-1', name: 'Alice', is_auto_generated: false, life_status: 'alive' },
      // v0.64.7：第 2 章写成死亡的角色
      {
        id: 'char-2',
        name: 'Bob',
        is_auto_generated: false,
        life_status: 'dead',
        death_chapter: 2,
      },
    ],
    isLoading: false,
  }),
  useCreateCharacter: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteCharacter: () => ({ mutate: vi.fn(), isPending: false }),
  useSetCharacterLifeStatus: () => ({ mutate: setLifeStatusMutate, isPending: false }),
}));

vi.mock('@/hooks/useCharacterRelationships', () => ({
  useCharacterRelationships: () => ({
    data: [
      {
        id: 'rel-1',
        story_id: 'story-1',
        source_character_id: 'char-1',
        target_character_id: 'char-2',
        target_character_name: 'Bob',
        relationship_type: '朋友',
        description: '好朋友',
        emotional_bond: '信任',
        emotional_intensity: 0.8,
        reverse_emotional_bond: '依赖',
        reverse_emotional_intensity: 0.6,
        created_at: new Date().toISOString(),
      },
      {
        id: 'rel-2',
        story_id: 'story-1',
        source_character_id: 'char-2',
        target_character_id: 'char-1',
        target_character_name: 'Alice',
        relationship_type: '对手',
        description: '竞争对手',
        emotional_bond: '嫉妒',
        emotional_intensity: 0.7,
        reverse_emotional_bond: '欣赏',
        reverse_emotional_intensity: 0.4,
        created_at: new Date().toISOString(),
      },
    ],
    isLoading: false,
  }),
  useCreateCharacterRelationship: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteCharacterRelationship: () => ({ mutate: deleteMutate, isPending: false }),
  useUpdateCharacterRelationship: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('@/hooks/useWorldBuilding', () => ({
  useWorldBuilding: () => ({ data: null }),
}));

vi.mock('@/components/CharacterStatePanel', () => ({
  CharacterStatePanel: () => <div data-testid="character-state-panel" />,
}));

vi.mock('@/components/CharacterEditModal', () => ({
  CharacterEditModal: () => <div data-testid="character-edit-modal" />,
}));

vi.mock('@/components/CharacterRelationshipForm', () => ({
  CharacterRelationshipForm: () => <div data-testid="relationship-form" />,
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

describe('Characters', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  it('renders relationship delete button and triggers mutation on confirm', async () => {
    render(<Characters />, { wrapper });

    await userEvent.click(screen.getByRole('button', { name: '关系' }));

    const deleteBtn = (await screen.findAllByTestId('delete-relationship-rel-1'))[0];
    expect(deleteBtn).toBeInTheDocument();

    await userEvent.click(deleteBtn);

    expect(window.confirm).toHaveBeenCalledWith('确定要删除这个关系吗？');
    expect(deleteMutate).toHaveBeenCalledWith({ relationshipId: 'rel-1', storyId: 'story-1' });
  });

  it('does not trigger mutation when delete is cancelled', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<Characters />, { wrapper });

    await userEvent.click(screen.getByRole('button', { name: '关系' }));

    const deleteBtn = (await screen.findAllByTestId('delete-relationship-rel-1'))[0];
    await userEvent.click(deleteBtn);

    expect(deleteMutate).not.toHaveBeenCalled();
  });

  it('展示行方向标签按 isOutgoing 生成并含实际角色名', async () => {
    render(<Characters />, { wrapper });

    await userEvent.click(screen.getByRole('button', { name: '关系' }));

    // 出向卡片（Alice 视角看 rel-1）：source→target 即 当前角色 → Bob
    expect(await screen.findAllByText(/当前角色 → Bob: 信任/)).not.toHaveLength(0);
    expect(screen.getAllByText(/Bob → 当前角色: 依赖/)).not.toHaveLength(0);

    // 入向卡片（Alice 视角看 rel-2）：对方名按 source 解析为 Bob，方向交换
    expect(screen.getAllByText(/Bob → 当前角色: 嫉妒/)).not.toHaveLength(0);
    expect(screen.getAllByText(/当前角色 → Bob: 欣赏/)).not.toHaveLength(0);
    expect(screen.getAllByText(/来自 Bob/)).not.toHaveLength(0);
  });

  it('编辑表单的方向标签与 placeholder 随 isOutgoing 适配', async () => {
    render(<Characters />, { wrapper });

    await userEvent.click(screen.getByRole('button', { name: '关系' }));

    const editButtons = await screen.findAllByTitle('编辑关系');
    await userEvent.click(editButtons[0]);

    expect(screen.getByText(/当前角色 → Bob的情感/)).toBeInTheDocument();
    expect(screen.getByText(/Bob → 当前角色的情感/)).toBeInTheDocument();
    expect(screen.getByPlaceholderText('如：信任/憎恨（留空则保持不变）')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('如：崇拜/冷漠（留空则保持不变）')).toBeInTheDocument();
  });

  // v0.64.7：正文写死的角色在人物页有「已死（第N章）」徽标，
  // 作者可一键改回存活（假死/诈死情节）
  it('已死角色显示徽标，存活角色不显示', async () => {
    render(<Characters />, { wrapper });

    const badge = await screen.findByTestId('dead-badge-char-2');
    expect(badge).toBeInTheDocument();
    expect(badge.textContent).toContain('已死');
    expect(badge.textContent).toContain('第2章');
    expect(screen.queryByTestId('dead-badge-char-1')).not.toBeInTheDocument();
  });

  it('点击「改回存活」经确认后提交 alive=true', async () => {
    render(<Characters />, { wrapper });

    await userEvent.click(await screen.findByTestId('toggle-life-char-2'));

    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('改回存活'));
    expect(setLifeStatusMutate).toHaveBeenCalledWith({
      storyId: 'story-1',
      name: 'Bob',
      alive: true,
    });
  });

  it('存活角色可标记身故（alive=false），取消确认则不提交', async () => {
    render(<Characters />, { wrapper });

    await userEvent.click(await screen.findByTestId('toggle-life-char-1'));
    expect(setLifeStatusMutate).toHaveBeenCalledWith({
      storyId: 'story-1',
      name: 'Alice',
      alive: false,
    });

    setLifeStatusMutate.mockClear();
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    await userEvent.click(await screen.findByTestId('toggle-life-char-2'));
    expect(setLifeStatusMutate).not.toHaveBeenCalled();
  });
});
