import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SceneEditor } from '../SceneEditor';
import type { Scene } from '@/types';

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn(),
}));

vi.mock('@/hooks/useMemoryCompression', () => ({
  useCompressScene: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

vi.mock('../scene-editor/SceneAuditPanel', () => ({
  SceneAuditPanel: () => <div data-testid="scene-audit-panel" />,
}));

vi.mock('../scene-editor/SceneAnnotationPanel', () => ({
  SceneAnnotationPanel: () => <div data-testid="scene-annotation-panel" />,
}));

vi.mock('../pipeline/PipelinePanel', () => ({
  PipelinePanel: (props: { sceneId?: string; onContentChange?: (content: string) => void }) => (
    <div data-testid="pipeline-panel">
      <span>Finalize scene={props.sceneId}</span>
      <button type="button" onClick={() => props.onContentChange?.('merged-content')}>
        mock-finalize
      </button>
    </div>
  ),
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

const baseScene: Scene = {
  id: 'scene-1',
  story_id: 'story-1',
  sequence_number: 2,
  title: '第二场',
  characters_present: [],
  character_conflicts: [],
  content: 'original',
  execution_stage: 'drafting',
  draft_content: 'draft',
  created_at: '2026-07-09T00:00:00Z',
  updated_at: '2026-07-09T00:00:00Z',
};

function renderEditor(scene: Scene = baseScene) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');

  const result = render(
    <QueryClientProvider client={queryClient}>
      <SceneEditor scene={scene} characters={[]} onSave={vi.fn()} onCancel={vi.fn()} />
    </QueryClientProvider>
  );

  return { ...result, invalidateSpy, queryClient };
}

describe('SceneEditor pipeline rail (P1a)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows collapsible 管线 rail on drafting/review/final tabs', async () => {
    renderEditor();

    expect(screen.getByTestId('scene-editor-pipeline-rail')).toBeInTheDocument();
    expect(screen.getByTestId('pipeline-panel')).toBeInTheDocument();
    expect(screen.getByText('Finalize scene=scene-1')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: '规划' }));
    expect(screen.queryByTestId('scene-editor-pipeline-rail')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: '审校' }));
    expect(screen.getByTestId('scene-editor-pipeline-rail')).toBeInTheDocument();
  });

  it('refreshes editor content and invalidates scenes query on pipeline content change', async () => {
    const { invalidateSpy } = renderEditor();

    await userEvent.click(screen.getByRole('button', { name: 'mock-finalize' }));

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['scenes', 'story-1'] });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['scenes', 'detail', 'scene-1'] });
  });
});
