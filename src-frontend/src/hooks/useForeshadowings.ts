import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';

export interface Foreshadowing {
  id: string;
  story_id: string;
  content: string;
  setup_scene_id?: string;
  payoff_scene_id?: string;
  status: 'setup' | 'payoff' | 'abandoned';
  importance: number;
  is_auto_generated?: boolean;
  created_at: string;
  resolved_at?: string;
}

export interface PayoffLedgerItem {
  id: string;
  ledger_key: string;
  title: string;
  summary: string;
  scope_type: 'story' | 'arc' | 'scene';
  current_status: 'setup' | 'hinted' | 'pending_payoff' | 'paid_off' | 'failed' | 'overdue';
  target_start_scene?: number;
  target_end_scene?: number;
  first_seen_scene?: number;
  last_touched_scene?: number;
  confidence: number;
  risk_signals: string[];
  importance: number;
  created_at: string;
  resolved_at?: string;
}

export interface PayoffRecommendation {
  foreshadowing_id: string;
  ledger_key: string;
  title: string;
  recommended_scene: number;
  urgency: 'low' | 'medium' | 'high' | 'critical';
  reason: string;
  importance: number;
}

export interface CreateForeshadowingInput {
  story_id: string;
  content: string;
  setup_scene_id?: string;
  importance: number;
}

export interface UpdateForeshadowingStatusInput {
  id: string;
  status: 'payoff' | 'abandoned';
  payoff_scene_id?: string;
}

const FORESHADOWINGS_KEY = 'foreshadowings';
const PAYOFF_LEDGER_KEY = 'payoff_ledger';
const OVERDUE_KEY = 'overdue_payoffs';
const RECOMMENDATIONS_KEY = 'payoff_recommendations';

export function useForeshadowings(storyId: string | null) {
  return useQuery({
    queryKey: [FORESHADOWINGS_KEY, storyId],
    queryFn: async () => {
      if (!storyId) return [];
      return loggedInvoke<Foreshadowing[]>('get_story_foreshadowings', { story_id: storyId });
    },
    enabled: !!storyId,
  });
}

export function useCreateForeshadowing() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: CreateForeshadowingInput) => {
      return loggedInvoke<string>('create_foreshadowing', {
        story_id: input.story_id,
        content: input.content,
        setup_scene_id: input.setup_scene_id,
        importance: input.importance,
      });
    },
    onSuccess: (_, input) => {
      queryClient.invalidateQueries({ queryKey: [FORESHADOWINGS_KEY, input.story_id] });
      queryClient.invalidateQueries({ queryKey: [PAYOFF_LEDGER_KEY, input.story_id] });
      queryClient.invalidateQueries({ queryKey: [OVERDUE_KEY, input.story_id] });
      queryClient.invalidateQueries({ queryKey: [RECOMMENDATIONS_KEY, input.story_id] });
    },
  });
}

export function useUpdateForeshadowingStatus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: UpdateForeshadowingStatusInput) => {
      return loggedInvoke<void>('update_foreshadowing_status', {
        id: input.id,
        status: input.status,
        payoff_scene_id: input.payoff_scene_id,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [FORESHADOWINGS_KEY] });
      queryClient.invalidateQueries({ queryKey: [PAYOFF_LEDGER_KEY] });
      queryClient.invalidateQueries({ queryKey: [OVERDUE_KEY] });
      queryClient.invalidateQueries({ queryKey: [RECOMMENDATIONS_KEY] });
    },
  });
}

// ==================== Payoff Ledger Hooks ====================

export function usePayoffLedger(storyId: string | null) {
  return useQuery({
    queryKey: [PAYOFF_LEDGER_KEY, storyId],
    queryFn: async () => {
      if (!storyId) return [];
      return loggedInvoke<PayoffLedgerItem[]>('get_payoff_ledger', { story_id: storyId });
    },
    enabled: !!storyId,
  });
}

export function useDetectOverduePayoffs(storyId: string | null, currentSceneNumber: number | null) {
  return useQuery({
    queryKey: [OVERDUE_KEY, storyId, currentSceneNumber],
    queryFn: async () => {
      if (!storyId || currentSceneNumber == null) return [];
      return loggedInvoke<PayoffLedgerItem[]>('detect_overdue_payoffs', {
        story_id: storyId,
        current_scene_number: currentSceneNumber,
      });
    },
    enabled: !!storyId && currentSceneNumber != null,
  });
}

export function useRecommendPayoffTiming(
  storyId: string | null,
  currentSceneNumber: number | null
) {
  return useQuery({
    queryKey: [RECOMMENDATIONS_KEY, storyId, currentSceneNumber],
    queryFn: async () => {
      if (!storyId || currentSceneNumber == null) return [];
      return loggedInvoke<PayoffRecommendation[]>('recommend_payoff_timing', {
        story_id: storyId,
        current_scene_number: currentSceneNumber,
      });
    },
    enabled: !!storyId && currentSceneNumber != null,
  });
}

export function useUpdatePayoffLedgerFields() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (params: {
      foreshadowing_id: string;
      story_id: string;
      target_start_scene?: number;
      target_end_scene?: number;
      risk_signals?: string[];
      scope_type?: string;
      ledger_key?: string;
    }) => {
      return loggedInvoke<void>('update_payoff_ledger_fields', {
        foreshadowing_id: params.foreshadowing_id,
        target_start_scene: params.target_start_scene,
        target_end_scene: params.target_end_scene,
        risk_signals: params.risk_signals,
        scope_type: params.scope_type,
        ledger_key: params.ledger_key,
      });
    },
    onSuccess: (_, params) => {
      queryClient.invalidateQueries({ queryKey: [PAYOFF_LEDGER_KEY, params.story_id] });
      queryClient.invalidateQueries({ queryKey: [OVERDUE_KEY, params.story_id] });
      queryClient.invalidateQueries({ queryKey: [RECOMMENDATIONS_KEY, params.story_id] });
    },
  });
}
