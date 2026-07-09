import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Scenes } from '../Scenes';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({
      currentStory: { id: 'story-1', title: 'Test Story' },
      pendingSceneId: null,
      setPendingSceneId: vi.fn(),
      setCurrentView: vi.fn(),
    }),
}));

vi.mock('@/hooks/useScenes', () => ({
  useScenes: () => ({
    data: [
      {
        id: 'scene-1',
        story_id: 'story-1',
        sequence_number: 1,
        title: '开场',
        characters_present: [],
        character_conflicts: [],
        content: 'hello',
        foreshadowing_ids: [],
        created_at: '2026-07-09T00:00:00Z',
        updated_at: '2026-07-09T00:00:00Z',
      },
    ],
    isLoading: false,
  }),
  useCreateScene: () => ({ mutate: vi.fn() }),
  useUpdateScene: () => ({ mutate: vi.fn() }),
  useDeleteScene: () => ({ mutate: vi.fn() }),
  useReorderScenes: () => ({ mutate: vi.fn() }),
}));

vi.mock('@/hooks/useCharacters', () => ({
  useCharacters: () => ({ data: [], isLoading: false }),
}));

vi.mock('@/hooks/useSceneVersions', () => ({
  useCreateSceneVersion: () => ({ mutate: vi.fn() }),
}));

vi.mock('@/components/StoryTimeline', () => ({
  StoryTimeline: ({
    onEditScene,
    scenes,
  }: {
    onEditScene: (scene: { id: string }) => void;
    scenes: { id: string }[];
  }) => (
    <button type="button" data-testid="edit-scene" onClick={() => onEditScene(scenes[0])}>
      edit
    </button>
  ),
}));

vi.mock('@/components/SceneEditor', () => ({
  SceneEditor: () => (
    <div data-testid="scene-editor">
      <div data-testid="scene-editor-pipeline-rail">管线</div>
    </div>
  ),
}));

vi.mock('@/components/ExecutionPanel', () => ({
  ExecutionPanel: () => <div data-testid="execution-panel">execution</div>,
}));

vi.mock('@/components/VersionTimeline', () => ({
  VersionTimeline: () => <div data-testid="version-timeline" />,
}));

vi.mock('@/components/DiffViewer', () => ({
  DiffViewer: () => <div data-testid="diff-viewer" />,
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

describe('Scenes edit-mode layout (P1a)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('does not render a standalone pipeline column while editing', async () => {
    render(<Scenes />, { wrapper });

    expect(screen.getByTestId('scenes-execution-column')).toBeInTheDocument();
    expect(screen.queryByTestId('standalone-pipeline-column')).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId('edit-scene'));

    expect(screen.getByTestId('scene-editor')).toBeInTheDocument();
    expect(screen.getByTestId('scene-editor-pipeline-rail')).toBeInTheDocument();
    expect(screen.queryByTestId('scenes-execution-column')).not.toBeInTheDocument();
    expect(screen.queryByTestId('standalone-pipeline-column')).not.toBeInTheDocument();
  });
});
