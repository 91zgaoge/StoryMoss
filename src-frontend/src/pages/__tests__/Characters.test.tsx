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
    data: [{ id: 'char-1', name: 'Alice', is_auto_generated: false }],
    isLoading: false,
  }),
  useCreateCharacter: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteCharacter: () => ({ mutate: vi.fn(), isPending: false }),
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

    const deleteBtn = await screen.findByTestId('delete-relationship-rel-1');
    expect(deleteBtn).toBeInTheDocument();

    await userEvent.click(deleteBtn);

    expect(window.confirm).toHaveBeenCalledWith('确定要删除这个关系吗？');
    expect(deleteMutate).toHaveBeenCalledWith({ relationshipId: 'rel-1', storyId: 'story-1' });
  });

  it('does not trigger mutation when delete is cancelled', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<Characters />, { wrapper });

    await userEvent.click(screen.getByRole('button', { name: '关系' }));

    const deleteBtn = await screen.findByTestId('delete-relationship-rel-1');
    await userEvent.click(deleteBtn);

    expect(deleteMutate).not.toHaveBeenCalled();
  });
});
