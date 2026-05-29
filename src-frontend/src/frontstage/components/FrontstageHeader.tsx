import React from 'react';
import { cn } from '@/utils/cn';
import ColorThemeDot from './ColorThemeDot';
import { IngestHealthIndicator } from './IngestHealthIndicator';
import type { Scene } from '@/types/v3';

interface Chapter {
  id: string;
  story_id: string;
  title?: string;
  chapter_number: number;
  content?: string;
  scene_id?: string;
}

interface Story {
  id: string;
  title: string;
  description?: string;
}

interface FrontstageHeaderProps {
  currentStory: Story | null;
  currentChapter: Chapter | null;
  wordCount: number;
  totalWordCount: number;
  fontSize: number;
  isSaved: boolean;
  isZenMode: boolean;
  wensiMode: 'off' | 'passive' | 'active';
  orchestratorStatus: { message: string } | null;
  bootstrapProgress: {
    stepName: string;
    stepNumber: number;
    totalSteps: number;
    status: string;
    message: string;
  } | null;
  onOpenBackstage: () => void;
  onCycleWensiMode: () => void;
  onToggleZenMode: () => void;
}

const FrontstageHeader: React.FC<FrontstageHeaderProps> = ({
  currentStory,
  currentChapter,
  wordCount,
  totalWordCount,
  fontSize,
  isSaved,
  isZenMode,
  wensiMode,
  orchestratorStatus,
  bootstrapProgress,
  onOpenBackstage,
  onCycleWensiMode,
  onToggleZenMode,
}) => {
  const wensiTooltip =
    wensiMode === 'active'
      ? '文思活跃 — Ctrl+Enter 续写'
      : wensiMode === 'passive'
        ? '文思被动 — 仅萤火提示'
        : '文思关闭';

  return (
    <header className="frontstage-header">
      <div className="frontstage-header-left">
        <span className="frontstage-story-name" onClick={onOpenBackstage} title="点击回幕后工作室">
          {currentStory?.title || '草苔'}
        </span>
        <div className="frontstage-status-bar">
          <span className="status-item">
            {currentChapter?.title ||
              (currentChapter ? `第${currentChapter.chapter_number}章` : '')}
          </span>
          <span className="status-separator">·</span>
          <span className="status-item" title="当前章节字数 / 全文字数">
            {wordCount} 字 / {totalWordCount} 字
          </span>
          <span className="status-separator">·</span>
          <span className="status-item" title="字体大小">
            {fontSize}px
          </span>
          {!isSaved && (
            <>
              <span className="status-separator">·</span>
              <span className="status-item saving">保存中...</span>
            </>
          )}
          {orchestratorStatus && (
            <>
              <span className="status-separator">·</span>
              <span className="status-item saving" title="AI 编排器状态">
                {orchestratorStatus.message}
              </span>
            </>
          )}
          {bootstrapProgress && (
            <>
              <span className="status-separator">·</span>
              <span
                className={cn(
                  'status-item',
                  bootstrapProgress.status === 'failed'
                    ? 'error'
                    : bootstrapProgress.status === 'completed'
                      ? 'saved'
                      : 'saving'
                )}
                title={
                  bootstrapProgress.status === 'failed'
                    ? `失败: ${bootstrapProgress.message}`
                    : '小说初始化进度'
                }
              >
                {bootstrapProgress.stepName}
                {bootstrapProgress.status === 'failed' ? ' ❌' : ''}({bootstrapProgress.stepNumber}/
                {bootstrapProgress.totalSteps})
              </span>
            </>
          )}
        </div>
      </div>

      {!isZenMode && (
        <div className="frontstage-header-right">
          <IngestHealthIndicator storyId={currentStory?.id || null} />
          <ColorThemeDot isZenMode={isZenMode} />
          <button
            className={`wensi-mode-toggle wensi-${wensiMode}`}
            onClick={onCycleWensiMode}
            title={wensiTooltip}
          >
            <span className="wensi-icon">
              {wensiMode === 'active' ? '热' : wensiMode === 'passive' ? '温' : '·'}
            </span>
          </button>
          <button className="zen-mode-btn" onClick={onToggleZenMode} title="F11 禅模式">
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <path d="M9 3v18" />
            </svg>
          </button>
        </div>
      )}
    </header>
  );
};

export default React.memo(FrontstageHeader);
