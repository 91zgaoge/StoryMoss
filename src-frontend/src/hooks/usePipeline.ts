import { useState, useCallback, useRef } from 'react';
import {
  runRefine,
  runReview,
  runFinalize,
  repairFinalize,
  getPipelineActiveDraft,
  mergeRevision,
  getDraftRevisionHistory,
  getDraftReviewHistory,
  getPostProcessStatus,
  getStoryChapterDrafts,
  getLatestPipelineReview,
} from '@/services/tauri';
import type {
  Draft,
  Revision,
  PipelineReview,
  PostProcessRun,
  PostProcessStep,
  RefineResult,
  ReviewResult,
  PipelineResult,
} from '@/types/pipeline';

export type PipelinePhase =
  | 'idle'
  | 'loading'
  | 'refining'
  | 'reviewing'
  | 'finalizing'
  | 'repairing'
  | 'completed'
  | 'failed';

export interface PipelineState {
  phase: PipelinePhase;
  progress: number;
  message: string;
  currentDraft?: Draft;
  drafts: Draft[];
  revisions: Revision[];
  reviews: PipelineReview[];
  latestReview?: ReviewResult | null;
  postProcessRun?: PostProcessRun;
  postProcessSteps: PostProcessStep[];
  error?: string;
}

function parseReviewResult(review: PipelineReview | null): ReviewResult | null {
  if (!review) return null;
  try {
    return {
      review_id: review.id,
      overall_score: review.overall_score || 0,
      dimensions: review.dimensions ? JSON.parse(review.dimensions) : [],
      issues: review.issues ? JSON.parse(review.issues) : [],
      summary: review.content || '',
    };
  } catch {
    return null;
  }
}

export function usePipeline(storyId: string, chapterNumber: number) {
  const [state, setState] = useState<PipelineState>({
    phase: 'idle',
    progress: 0,
    message: '',
    drafts: [],
    revisions: [],
    reviews: [],
    postProcessSteps: [],
  });

  const abortControllerRef = useRef<AbortController | null>(null);

  const setPhase = useCallback((phase: PipelinePhase, message: string, progress: number = 0) => {
    setState(prev => ({ ...prev, phase, message, progress, error: undefined }));
  }, []);

  const setError = useCallback((error: string) => {
    setState(prev => ({ ...prev, phase: 'failed', error }));
  }, []);

  const refreshDrafts = useCallback(async () => {
    try {
      const drafts = await getStoryChapterDrafts(storyId, chapterNumber);
      const activeDraft = await getPipelineActiveDraft(storyId, chapterNumber);
      setState(prev => ({ ...prev, drafts, currentDraft: activeDraft || undefined }));
      return { drafts, activeDraft };
    } catch (e) {
      console.warn('[usePipeline] refreshDrafts failed:', e);
      return { drafts: [] as Draft[], activeDraft: null };
    }
  }, [storyId, chapterNumber]);

  const refreshRevisions = useCallback(async (draftId: string) => {
    try {
      const revisions = await getDraftRevisionHistory(draftId);
      setState(prev => ({ ...prev, revisions }));
      return revisions;
    } catch (e) {
      console.warn('[usePipeline] refreshRevisions failed:', e);
      return [];
    }
  }, []);

  const refreshReviews = useCallback(async (draftId: string) => {
    try {
      const reviews = await getDraftReviewHistory(draftId);
      const rawLatest = await getLatestPipelineReview(draftId);
      const latestReview = parseReviewResult(rawLatest);
      setState(prev => ({ ...prev, reviews, latestReview: latestReview || undefined }));
      return { reviews, latestReview };
    } catch (e) {
      console.warn('[usePipeline] refreshReviews failed:', e);
      return { reviews: [] as PipelineReview[], latestReview: null };
    }
  }, []);

  const runRefineAction = useCallback(
    async (draftId: string, userPrompt?: string) => {
      setPhase('refining', '正在执行 AI 修稿...', 10);
      try {
        const result: RefineResult = await runRefine(storyId, draftId, userPrompt);
        setPhase('completed', `修稿完成：${result.change_summary || '已生成修订版本'}`, 100);
        await refreshRevisions(draftId);
        await refreshDrafts();
        return result;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(`修稿失败: ${msg}`);
        throw e;
      }
    },
    [storyId, setPhase, setError, refreshRevisions, refreshDrafts]
  );

  const runReviewAction = useCallback(
    async (draftId: string, reviewFocus?: string) => {
      setPhase('reviewing', '正在执行 AI 审稿...', 10);
      try {
        const result: ReviewResult = await runReview(storyId, draftId, reviewFocus);
        setState(prev => ({
          ...prev,
          phase: 'completed',
          message: `审稿完成：综合评分 ${result.overall_score}分`,
          progress: 100,
          latestReview: result,
        }));
        await refreshReviews(draftId);
        return result;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(`审稿失败: ${msg}`);
        throw e;
      }
    },
    [storyId, setPhase, setError, refreshReviews]
  );

  const runFinalizeAction = useCallback(
    async (draftId: string, chapterTitle?: string) => {
      setPhase('finalizing', '正在定稿并执行后处理...', 10);
      try {
        const result: PipelineResult = await runFinalize(
          storyId,
          draftId,
          chapterNumber,
          chapterTitle
        );
        setPhase('completed', '定稿完成，后处理已启动', 100);

        if (result.post_process_run_id) {
          await pollPostProcessStatus(result.post_process_run_id);
        }

        await refreshDrafts();
        return result;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(`定稿失败: ${msg}`);
        throw e;
      }
    },
    [storyId, chapterNumber, setPhase, setError, refreshDrafts]
  );

  const repairFinalizeAction = useCallback(async () => {
    setPhase('repairing', '正在修复定稿后处理...', 10);
    try {
      const result: PipelineResult = await repairFinalize(storyId, chapterNumber);
      setPhase('completed', '后处理修复完成', 100);

      if (result.post_process_run_id) {
        await pollPostProcessStatus(result.post_process_run_id);
      }

      await refreshDrafts();
      return result;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`修复失败: ${msg}`);
      throw e;
    }
  }, [storyId, chapterNumber, setPhase, setError, refreshDrafts]);

  const mergeRevisionAction = useCallback(
    async (revisionId: string) => {
      try {
        await mergeRevision(revisionId);
        await refreshDrafts();
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(`合并修稿失败: ${msg}`);
        throw e;
      }
    },
    [setError, refreshDrafts]
  );

  const pollPostProcessStatus = useCallback(async (runId: string, maxAttempts: number = 30) => {
    for (let i = 0; i < maxAttempts; i++) {
      try {
        const status = await getPostProcessStatus(runId);
        if (status) {
          setState(prev => ({
            ...prev,
            postProcessRun: status.run,
            postProcessSteps: status.steps,
          }));

          if (status.run.status === 'completed' || status.run.status === 'failed') {
            break;
          }
        }
      } catch (e) {
        console.warn('[usePipeline] pollPostProcessStatus failed:', e);
      }
      await new Promise(resolve => setTimeout(resolve, 2000));
    }
  }, []);

  const loadPipelineState = useCallback(async () => {
    setPhase('loading', '加载管线状态...', 0);
    try {
      const { drafts, activeDraft } = await refreshDrafts();
      if (activeDraft) {
        const [revisions, { reviews, latestReview }] = await Promise.all([
          refreshRevisions(activeDraft.id),
          refreshReviews(activeDraft.id),
        ]);

        setState(prev => ({
          ...prev,
          phase: 'idle',
          message: '管线状态已加载',
          progress: 100,
          currentDraft: activeDraft || undefined,
          drafts,
          revisions,
          reviews,
          latestReview: latestReview || undefined,
        }));
      } else {
        setPhase('idle', '暂无活跃草稿', 100);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`加载失败: ${msg}`);
    }
  }, [refreshDrafts, refreshRevisions, refreshReviews, setPhase, setError]);

  const reset = useCallback(() => {
    setState({
      phase: 'idle',
      progress: 0,
      message: '',
      drafts: [],
      revisions: [],
      reviews: [],
      postProcessSteps: [],
    });
  }, []);

  return {
    state,
    runRefine: runRefineAction,
    runReview: runReviewAction,
    runFinalize: runFinalizeAction,
    repairFinalize: repairFinalizeAction,
    mergeRevision: mergeRevisionAction,
    loadPipelineState,
    refreshDrafts,
    refreshRevisions,
    refreshReviews,
    reset,
  };
}
