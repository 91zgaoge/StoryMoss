import React, { useCallback, useEffect, useRef, useState } from 'react';
import { cn } from '@/utils/cn';
import { Flame, Sparkles, ZapOff, Maximize, Settings } from 'lucide-react';
import ColorThemeDot from './ColorThemeDot';
import { IngestHealthIndicator } from './IngestHealthIndicator';
import DebtIndicator from './DebtIndicator';
import EditableChapterTitle from './EditableChapterTitle';
import { displayChapterTitle } from '../utils/displayChapterTitle';

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
  /** 由 displayStoryTitle 计算的展示名 */
  displayTitle: string;
  /** 是否允许双击改名（通常有 currentStory 时为 true） */
  canRename: boolean;
  currentChapter: Chapter | null;
  /** 由 displayChapterTitle 计算的章节展示名 */
  displayChapterTitleText?: string;
  /** 是否允许双击改章节名 */
  canRenameChapter?: boolean;
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
  onRenameStory?: (title: string) => Promise<void> | void;
  onRenameChapter?: (title: string) => Promise<void> | void;
}

const CLICK_GUARD_MS = 350;

const FrontstageHeader: React.FC<FrontstageHeaderProps> = ({
  currentStory,
  displayTitle,
  canRename,
  currentChapter,
  displayChapterTitleText,
  canRenameChapter = false,
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
  onRenameStory,
  onRenameChapter,
}) => {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const clickGuardUntilRef = useRef(0);
  const titleBeforeEditRef = useRef('');

  const wensiTooltip =
    wensiMode === 'active'
      ? '文思活跃：按 Ctrl+Enter 触发 AI 续写'
      : wensiMode === 'passive'
        ? '文思被动：AI 仅显示萤火提示，不主动续写'
        : '文思已关闭';

  useEffect(() => {
    if (!editing) return;
    const el = inputRef.current;
    if (!el) return;
    el.focus();
    el.select();
  }, [editing]);

  const cancelEdit = useCallback(() => {
    setEditing(false);
    setDraft('');
    setSaving(false);
  }, []);

  const commitEdit = useCallback(async () => {
    if (saving) return;
    const next = draft.trim();
    if (!next) {
      cancelEdit();
      return;
    }
    if (next === titleBeforeEditRef.current) {
      cancelEdit();
      return;
    }
    if (!onRenameStory) {
      cancelEdit();
      return;
    }
    setSaving(true);
    try {
      await onRenameStory(next);
      setEditing(false);
      setDraft('');
    } catch {
      // 失败时保持编辑态，便于重试
    } finally {
      setSaving(false);
    }
  }, [draft, onRenameStory, saving, cancelEdit]);

  const handleTitleClick = useCallback(() => {
    if (Date.now() < clickGuardUntilRef.current) return;
    if (editing) return;
    onOpenBackstage();
  }, [editing, onOpenBackstage]);

  const handleTitleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      clickGuardUntilRef.current = Date.now() + CLICK_GUARD_MS;
      if (!canRename || !onRenameStory) return;
      titleBeforeEditRef.current = displayTitle;
      setDraft(displayTitle);
      setEditing(true);
    },
    [canRename, onRenameStory, displayTitle]
  );

  const handleInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        void commitEdit();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancelEdit();
      }
    },
    [commitEdit, cancelEdit]
  );

  return (
    <header className="frontstage-header">
      <div className="frontstage-header-left">
        {editing ? (
          <input
            ref={inputRef}
            className="frontstage-story-name-input"
            value={draft}
            disabled={saving}
            aria-label="编辑故事名称"
            onChange={e => setDraft(e.target.value)}
            onBlur={() => {
              void commitEdit();
            }}
            onKeyDown={handleInputKeyDown}
          />
        ) : (
          <span
            className="frontstage-story-name"
            onClick={handleTitleClick}
            onDoubleClick={handleTitleDoubleClick}
            title={
              canRename
                ? '单击回幕后工作室 · 双击改名'
                : currentStory
                  ? '点击回幕后工作室'
                  : '点击回幕后工作室'
            }
          >
            {displayTitle}
          </span>
        )}
        <div className="frontstage-status-bar">
          {currentChapter && (
            <EditableChapterTitle
              displayTitle={displayChapterTitleText ?? displayChapterTitle(currentChapter)}
              canRename={canRenameChapter}
              onRename={onRenameChapter}
              variant="status"
            />
          )}
          <span className="status-separator">·</span>
          <span className="status-item" title="当前章节字数 / 全文字数">
            {wordCount} 字 / {totalWordCount} 字
          </span>
          <span className="status-separator">·</span>
          <span
            className={cn(
              'status-item',
              onOpenFontSettings && 'cursor-pointer hover:text-cinema-gold'
            )}
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
