import React, { useEffect, useState } from 'react';
import {
  Wrench,
  Search,
  CheckCircle,
  RotateCcw,
  Loader2,
  ChevronDown,
  ChevronUp,
  FileText,
  AlertCircle,
  GitMerge,
  Clock,
  BookOpen,
} from 'lucide-react';
import { usePipeline } from '@/hooks/usePipeline';
import type { Draft, Revision, PipelineReview } from '@/types/pipeline';
import { cn } from '@/utils/cn';

interface PipelinePanelProps {
  storyId: string;
  chapterNumber: number;
  chapterTitle?: string;
  currentContent?: string;
  onContentChange?: (content: string) => void;
}

export const PipelinePanel: React.FC<PipelinePanelProps> = ({
  storyId,
  chapterNumber,
  chapterTitle,
  currentContent,
  onContentChange,
}) => {
  const {
    state,
    runRefine,
    runReview,
    runFinalize,
    repairFinalize,
    mergeRevision,
    loadPipelineState,
    refreshDrafts,
  } = usePipeline(storyId, chapterNumber);

  const [selectedDraft, setSelectedDraft] = useState<Draft | null>(null);
  const [showDrafts, setShowDrafts] = useState(false);
  const [showReviews, setShowReviews] = useState(false);
  const [refinePrompt, setRefinePrompt] = useState('');
  const [reviewFocus, setReviewFocus] = useState('');
  const [activeTab, setActiveTab] = useState<'actions' | 'drafts' | 'reviews'>('actions');

  useEffect(() => {
    if (storyId) {
      loadPipelineState();
    }
  }, [storyId, chapterNumber, loadPipelineState]);

  const isBusy = state.phase !== 'idle' && state.phase !== 'completed' && state.phase !== 'failed';

  const handleRefine = async () => {
    if (!selectedDraft && !state.currentDraft) return;
    const draft = selectedDraft || state.currentDraft;
    if (!draft) return;
    try {
      await runRefine(draft.id, refinePrompt || undefined);
      setRefinePrompt('');
    } catch {
      // error handled in hook
    }
  };

  const handleReview = async () => {
    if (!selectedDraft && !state.currentDraft) return;
    const draft = selectedDraft || state.currentDraft;
    if (!draft) return;
    try {
      await runReview(draft.id, reviewFocus || undefined);
      setReviewFocus('');
    } catch {
      // error handled in hook
    }
  };

  const notifyContentChange = async () => {
    if (!onContentChange) return;
    const { activeDraft } = await refreshDrafts();
    const content = activeDraft?.content || selectedDraft?.content || state.currentDraft?.content;
    if (content) {
      onContentChange(content);
    }
  };

  const handleFinalize = async () => {
    if (!selectedDraft && !state.currentDraft) return;
    const draft = selectedDraft || state.currentDraft;
    if (!draft) return;
    try {
      await runFinalize(draft.id, chapterTitle);
      await notifyContentChange();
    } catch {
      // error handled in hook
    }
  };

  const handleRepair = async () => {
    try {
      await repairFinalize();
    } catch {
      // error handled in hook
    }
  };

  const handleMerge = async (revisionId: string) => {
    try {
      await mergeRevision(revisionId);
      await notifyContentChange();
    } catch {
      // error handled in hook
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'finalized':
        return 'text-green-400 bg-green-400/10';
      case 'reviewed':
        return 'text-blue-400 bg-blue-400/10';
      case 'refined':
        return 'text-purple-400 bg-purple-400/10';
      case 'draft':
        return 'text-amber-400 bg-amber-400/10';
      default:
        return 'text-gray-400 bg-gray-400/10';
    }
  };

  return (
    <div
      className="flex flex-col h-full bg-[#1a1a2e] border-l border-white/5"
      data-testid="pipeline-panel"
    >
      {/* Header */}
      <div className="px-4 py-3 border-b border-white/5">
        <h3 className="text-sm font-semibold text-white/90 flex items-center gap-2">
          <GitMerge className="w-4 h-4 text-purple-400" />
          创作管线
        </h3>
        <p className="text-xs text-white/40 mt-0.5">
          场景 #{chapterNumber} {chapterTitle || ''}
          <span className="ml-1 text-white/25">（管线序号）</span>
        </p>
        <p className="text-[11px] text-amber-400/80 mt-1.5" data-testid="pipeline-finalize-note">
          Finalize 按章节号落盘
        </p>
      </div>

      {/* Status Bar */}
      {isBusy && (
        <div className="px-4 py-2 bg-purple-500/10 border-b border-white/5">
          <div className="flex items-center gap-2">
            <Loader2 className="w-3.5 h-3.5 text-purple-400 animate-spin" />
            <span className="text-xs text-purple-300">{state.message}</span>
          </div>
          <div className="mt-1.5 h-1 bg-white/5 rounded-full overflow-hidden">
            <div
              className="h-full bg-purple-400 rounded-full transition-all duration-300"
              style={{ width: `${state.progress}%` }}
            />
          </div>
        </div>
      )}

      {state.error && (
        <div className="px-4 py-2 bg-red-500/10 border-b border-white/5">
          <div className="flex items-center gap-2">
            <AlertCircle className="w-3.5 h-3.5 text-red-400" />
            <span className="text-xs text-red-300">{state.error}</span>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="flex border-b border-white/5">
        {(['actions', 'drafts', 'reviews'] as const).map(tab => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={cn(
              'flex-1 px-3 py-2 text-xs font-medium transition-colors',
              activeTab === tab
                ? 'text-purple-300 bg-purple-500/10 border-b-2 border-purple-400'
                : 'text-white/40 hover:text-white/60 hover:bg-white/5'
            )}
          >
            {tab === 'actions' && '操作'}
            {tab === 'drafts' && `草稿 (${state.drafts.length})`}
            {tab === 'reviews' && `审稿 (${state.reviews.length})`}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {activeTab === 'actions' && (
          <div className="p-4 space-y-4">
            {/* Current Draft Info */}
            {state.currentDraft && (
              <div className="p-3 rounded-lg bg-white/5 border border-white/5">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-white/60">当前草稿</span>
                  <span
                    className={cn(
                      'text-[10px] px-1.5 py-0.5 rounded',
                      getStatusColor(state.currentDraft.status)
                    )}
                  >
                    {state.currentDraft.status}
                  </span>
                </div>
                <p className="text-xs text-white/80 mt-1">
                  v{state.currentDraft.version} · {state.currentDraft.word_count}字
                </p>
              </div>
            )}

            {/* Action Buttons */}
            <div className="space-y-2">
              <button
                onClick={handleRefine}
                disabled={isBusy || !state.currentDraft}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-all',
                  isBusy || !state.currentDraft
                    ? 'bg-white/5 text-white/30 cursor-not-allowed'
                    : 'bg-purple-500/20 text-purple-300 hover:bg-purple-500/30'
                )}
              >
                <Wrench className="w-3.5 h-3.5" />
                AI 修稿
              </button>

              {state.currentDraft?.status === 'draft' && (
                <div className="px-1">
                  <input
                    type="text"
                    value={refinePrompt}
                    onChange={e => setRefinePrompt(e.target.value)}
                    placeholder="修稿指令（可选）..."
                    className="w-full px-2 py-1.5 text-xs bg-white/5 border border-white/10 rounded text-white/80 placeholder:text-white/30 focus:outline-none focus:border-purple-400/50"
                  />
                </div>
              )}

              <button
                onClick={handleReview}
                disabled={isBusy || !state.currentDraft}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-all',
                  isBusy || !state.currentDraft
                    ? 'bg-white/5 text-white/30 cursor-not-allowed'
                    : 'bg-blue-500/20 text-blue-300 hover:bg-blue-500/30'
                )}
              >
                <Search className="w-3.5 h-3.5" />
                AI 审稿
              </button>

              <button
                onClick={handleFinalize}
                disabled={isBusy || !state.currentDraft}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-all',
                  isBusy || !state.currentDraft
                    ? 'bg-white/5 text-white/30 cursor-not-allowed'
                    : 'bg-green-500/20 text-green-300 hover:bg-green-500/30'
                )}
              >
                <CheckCircle className="w-3.5 h-3.5" />
                定稿
              </button>

              {state.postProcessRun?.status === 'failed' && (
                <button
                  onClick={handleRepair}
                  disabled={isBusy}
                  className={cn(
                    'w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-all',
                    isBusy
                      ? 'bg-white/5 text-white/30 cursor-not-allowed'
                      : 'bg-amber-500/20 text-amber-300 hover:bg-amber-500/30'
                  )}
                >
                  <RotateCcw className="w-3.5 h-3.5" />
                  修复后处理
                </button>
              )}
            </div>

            {/* Post Process Steps */}
            {state.postProcessSteps.length > 0 && (
              <div className="mt-4">
                <h4 className="text-xs font-medium text-white/50 mb-2">后处理进度</h4>
                <div className="space-y-1.5">
                  {state.postProcessSteps.map(step => (
                    <div
                      key={step.id}
                      className="flex items-center gap-2 px-2 py-1.5 rounded bg-white/5"
                    >
                      {step.status === 'running' && (
                        <Loader2 className="w-3 h-3 text-purple-400 animate-spin" />
                      )}
                      {step.status === 'success' && (
                        <CheckCircle className="w-3 h-3 text-green-400" />
                      )}
                      {step.status === 'failed' && <AlertCircle className="w-3 h-3 text-red-400" />}
                      {step.status === 'pending' && <Clock className="w-3 h-3 text-white/30" />}
                      <span className="text-[11px] text-white/60 flex-1">{step.step_label}</span>
                      {step.critical && <span className="text-[10px] text-red-400/60">关键</span>}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {activeTab === 'drafts' && (
          <div className="p-3 space-y-2">
            {state.drafts.length === 0 && (
              <div className="text-center py-8 text-white/30 text-xs">
                <FileText className="w-8 h-8 mx-auto mb-2 opacity-30" />
                暂无草稿
              </div>
            )}
            {state.drafts.map(draft => (
              <button
                key={draft.id}
                onClick={() => setSelectedDraft(draft)}
                className={cn(
                  'w-full text-left p-3 rounded-lg border transition-all',
                  selectedDraft?.id === draft.id
                    ? 'bg-purple-500/10 border-purple-400/30'
                    : 'bg-white/5 border-white/5 hover:bg-white/10'
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-white/80">v{draft.version}</span>
                  <span
                    className={cn(
                      'text-[10px] px-1.5 py-0.5 rounded',
                      getStatusColor(draft.status)
                    )}
                  >
                    {draft.status}
                  </span>
                </div>
                <p className="text-[11px] text-white/40 mt-1">
                  {draft.word_count}字 · {new Date(draft.created_at).toLocaleDateString()}
                </p>
                {draft.content && (
                  <p className="text-[11px] text-white/30 mt-1 line-clamp-2">
                    {draft.content.slice(0, 80)}...
                  </p>
                )}
              </button>
            ))}
          </div>
        )}

        {activeTab === 'reviews' && (
          <div className="p-3 space-y-3">
            {state.reviews.length === 0 && state.latestReview === null && (
              <div className="text-center py-8 text-white/30 text-xs">
                <BookOpen className="w-8 h-8 mx-auto mb-2 opacity-30" />
                暂无审稿报告
              </div>
            )}

            {state.latestReview && (
              <div className="p-3 rounded-lg bg-blue-500/10 border border-blue-400/20">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-blue-300">最新审稿</span>
                  <span className="text-lg font-bold text-blue-400">
                    {state.latestReview.overall_score}
                  </span>
                </div>
                <div className="mt-2 space-y-1.5">
                  {state.latestReview.dimensions.map(dim => (
                    <div key={dim.name} className="flex items-center gap-2">
                      <span className="text-[11px] text-white/50 w-16">{dim.name}</span>
                      <div className="flex-1 h-1.5 bg-white/10 rounded-full overflow-hidden">
                        <div
                          className="h-full bg-blue-400 rounded-full"
                          style={{ width: `${dim.score}%` }}
                        />
                      </div>
                      <span className="text-[11px] text-white/60 w-8 text-right">{dim.score}</span>
                    </div>
                  ))}
                </div>
                {state.latestReview.issues.length > 0 && (
                  <div className="mt-3 space-y-1.5">
                    <span className="text-[11px] text-white/40">
                      发现问题 ({state.latestReview.issues.length})
                    </span>
                    {state.latestReview.issues.slice(0, 3).map((issue, i) => (
                      <div key={i} className="px-2 py-1.5 rounded bg-white/5 text-[11px]">
                        <span
                          className={cn(
                            'text-[10px] px-1 py-0.5 rounded mr-1.5',
                            issue.severity === 'critical'
                              ? 'bg-red-400/20 text-red-400'
                              : issue.severity === 'high'
                                ? 'bg-orange-400/20 text-orange-400'
                                : 'bg-yellow-400/20 text-yellow-400'
                          )}
                        >
                          {issue.severity}
                        </span>
                        <span className="text-white/60">{issue.description}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {state.reviews.map(review => (
              <div key={review.id} className="p-2.5 rounded-lg bg-white/5 border border-white/5">
                <div className="flex items-center justify-between">
                  <span className="text-[11px] text-white/50">审稿 #{review.review_index}</span>
                  {review.overall_score !== undefined && (
                    <span className="text-xs font-medium text-white/70">
                      {review.overall_score}分
                    </span>
                  )}
                </div>
                {review.content && (
                  <p className="text-[11px] text-white/40 mt-1 line-clamp-3">
                    {review.content.slice(0, 120)}...
                  </p>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default PipelinePanel;
