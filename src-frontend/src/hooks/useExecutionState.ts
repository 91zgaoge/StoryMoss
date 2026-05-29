import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import { useScenes } from './useScenes';
import { useForeshadowings } from './useForeshadowings';
import { useChapters } from './useChapters';
import type { Scene } from '@/types';

export type NarrativePhase =
  | 'Setup'
  | 'Rising'
  | 'Climax'
  | 'Resolution'
  | 'Finale'
  | 'ConflictActive'
  | 'Falling';

export interface ExecutionState {
  scenes: Scene[];
  sceneCount: number;
  totalWordCount: number;
  narrativePhase: NarrativePhase;
  overduePayoffs: number;
  lastScene: Scene | null;
  chaptersCount: number;
  avgConfidence: number;
}

export interface PrimaryAction {
  label: string;
  action: string;
  variant: 'primary' | 'danger' | 'warning';
}

function mapBackendPhase(phase: string): NarrativePhase {
  switch (phase) {
    case 'Setup':
      return 'Setup';
    case 'Rising':
      return 'Rising';
    case 'Climax':
      return 'Climax';
    case 'Resolution':
      return 'Resolution';
    case 'Finale':
      return 'Finale';
    case 'ConflictActive':
      return 'ConflictActive';
    case 'Falling':
      return 'Falling';
    default:
      return 'Setup';
  }
}

function computeTotalWordCount(scenes: Scene[]): number {
  return scenes.reduce((sum, s) => sum + (s.content?.length || 0), 0);
}

function computeAvgConfidence(scenes: Scene[]): number {
  const scored = scenes.filter(s => s.confidence_score !== undefined);
  if (scored.length === 0) return 0;
  return scored.reduce((sum, s) => sum + (s.confidence_score || 0), 0) / scored.length;
}

export function useExecutionState(storyId: string | null): {
  state: ExecutionState;
  isLoading: boolean;
} {
  const { data: scenes = [], isLoading: scenesLoading } = useScenes(storyId);
  const { data: foreshadowings = [], isLoading: foreshadowingsLoading } =
    useForeshadowings(storyId);
  const { data: chapters = [], isLoading: chaptersLoading } = useChapters(storyId);

  const { data: canonicalState, isLoading: canonicalLoading } = useQuery({
    queryKey: ['canonical_state', storyId],
    queryFn: async () => {
      if (!storyId) return null;
      return loggedInvoke<{
        narrative_phase: string;
        story_context: { overdue_payoffs: unknown[] };
      }>('get_canonical_state', { story_id: storyId });
    },
    enabled: !!storyId,
    staleTime: 30000,
  });

  const state = useMemo<ExecutionState>(() => {
    const sceneCount = scenes.length;
    const totalWordCount = computeTotalWordCount(scenes);
    const narrativePhase = canonicalState
      ? mapBackendPhase(canonicalState.narrative_phase)
      : 'Setup';
    const overduePayoffs = canonicalState
      ? canonicalState.story_context.overdue_payoffs.length
      : foreshadowings.filter(f => f.status === 'setup').length;
    const lastScene =
      sceneCount > 0
        ? scenes.reduce(
            (latest, s) => (s.sequence_number > latest.sequence_number ? s : latest),
            scenes[0]
          )
        : null;
    const chaptersCount = chapters.length;
    const avgConfidence = computeAvgConfidence(scenes);

    return {
      scenes,
      sceneCount,
      totalWordCount,
      narrativePhase,
      overduePayoffs,
      lastScene,
      chaptersCount,
      avgConfidence,
    };
  }, [scenes, foreshadowings, chapters, canonicalState]);

  return {
    state,
    isLoading: scenesLoading || foreshadowingsLoading || chaptersLoading || canonicalLoading,
  };
}

export function resolvePrimaryAction(state: ExecutionState): PrimaryAction {
  if (state.overduePayoffs > 0) {
    return {
      label: `处理逾期伏笔 (${state.overduePayoffs})`,
      action: 'open_payoff_ledger',
      variant: 'danger',
    };
  }
  if (state.scenes.length === 0) {
    return {
      label: '创建首个场景',
      action: 'create_first_scene',
      variant: 'primary',
    };
  }
  if (state.lastScene) {
    const confidence = state.lastScene.confidence_score;
    if (confidence !== undefined && confidence < 0.5) {
      return {
        label: '修复场景问题',
        action: 'open_scene_editor',
        variant: 'warning',
      };
    }
    if (state.lastScene.content && state.lastScene.content.length < 200) {
      return {
        label: '续写当前场景',
        action: 'continue_writing',
        variant: 'primary',
      };
    }
  }
  return {
    label: '续写下一章',
    action: 'continue_next_chapter',
    variant: 'primary',
  };
}

export function getPhaseLabel(phase: NarrativePhase): string {
  const labels: Record<NarrativePhase, string> = {
    Setup: '铺垫阶段',
    Rising: '上升动作',
    Climax: '高潮阶段',
    Resolution: '收尾阶段',
    Finale: '终章',
    ConflictActive: '冲突激化',
    Falling: '回落阶段',
  };
  return labels[phase];
}

export function getPhaseColor(phase: NarrativePhase): string {
  const colors: Record<NarrativePhase, string> = {
    Setup: 'text-blue-400 bg-blue-500/20',
    Rising: 'text-amber-400 bg-amber-500/20',
    Climax: 'text-red-400 bg-red-500/20',
    Resolution: 'text-green-400 bg-green-500/20',
    Finale: 'text-purple-400 bg-purple-500/20',
    ConflictActive: 'text-orange-400 bg-orange-500/20',
    Falling: 'text-cyan-400 bg-cyan-500/20',
  };
  return colors[phase];
}
