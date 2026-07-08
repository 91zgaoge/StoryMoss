import React from 'react';
import { cn } from '@/utils/cn';
import { Flame, Sparkles, ZapOff, Maximize, Settings } from 'lucide-react';
import ColorThemeDot from './ColorThemeDot';
import { IngestHealthIndicator } from './IngestHealthIndicator';
import DebtIndicator from './DebtIndicator';
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
  dbPoolStatus: { in_use: number; max_size: number; idle: number } | null;
  onOpenBackstage: () => void;
  onOpenFontSettings?: () => void;
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
  dbPoolStatus,
  onOpenBackstage,
  onOpenFontSettings,
  onCycleWensiMode,
  onToggleZenMode,
}) => {
  const wensiTooltip =
    wensiMode === 'active'
      ? '文思活跃：按 Ctrl+Enter 触发 AI 续写'
      : wensiMode === 'passive'
        ? '文思被动：AI 仅显示萤火提示，不主动续写'
        : '文思已关闭';

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
          <span
            className={cn('status-item', onOpenFontSettings && 'cursor-pointer hover:text-cinema-gold')}
            title="字体大小（点击打开字体设置）"
            onClick={onOpenFontSettings}
          >
            {fontSize}px
          </span>
          {!isSaved && (
            <>
              <span className="status-separator">·</span>
              <span className="status-item saving">保存中...</span>
            </>
          )}
          {dbPoolStatus &&
            (() => {
              const utilPct =
                dbPoolStatus.max_size > 0 ? (dbPoolStatus.in_use / dbPoolStatus.max_size) * 100 : 0;
              const isCritical = dbPoolStatus.in_use >= dbPoolStatus.max_size || utilPct >= 95;
              const isWarning = utilPct >= 80;
              if (!isWarning) return null;
              return (
                <>
                  <span className="status-separator">·</span>
                  <span
                    className={cn('status-item', isCritical ? 'error' : 'saving')}
                    title={`数据库连接池：使用 ${dbPoolStatus.in_use}/${dbPoolStatus.max_size}（${Math.round(utilPct)}%），空闲 ${dbPoolStatus.idle}`}
                  >
                    DB {dbPoolStatus.in_use}/{dbPoolStatus.max_size}
                    {isCritical ? ' ⚠' : ''}
                  </span>
                </>
              );
            })()}
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
          <DebtIndicator
            chapterId={currentChapter?.id || null}
            storyId={currentStory?.id || null}
          />
          <IngestHealthIndicator storyId={currentStory?.id || null} />
          <ColorThemeDot isZenMode={isZenMode} />
          <button
            className="settings-btn"
            onClick={onOpenBackstage}
            title="打开设置 / 幕后工作室"
            aria-label="打开设置 / 幕后工作室"
          >
            <Settings className="w-3.5 h-3.5" />
          </button>
          <button
            className={cn('wensi-mode-toggle', `wensi-${wensiMode}`)}
            onClick={onCycleWensiMode}
            title={wensiTooltip}
            aria-label={wensiTooltip}
          >
            <span className="wensi-icon">
              {wensiMode === 'active' ? (
                <Flame className="w-3.5 h-3.5" />
              ) : wensiMode === 'passive' ? (
                <Sparkles className="w-3.5 h-3.5" />
              ) : (
                <ZapOff className="w-3.5 h-3.5" />
              )}
            </span>
          </button>
          <button
            className="zen-mode-btn"
            onClick={onToggleZenMode}
            title="进入全屏禅写模式（F11）"
            aria-label="进入全屏禅写模式（F11）"
          >
            <Maximize className="w-3.5 h-3.5" />
          </button>
        </div>
      )}
    </header>
  );
};

export default React.memo(FrontstageHeader);
