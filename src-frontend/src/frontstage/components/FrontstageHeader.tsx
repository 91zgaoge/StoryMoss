import React, { useCallback, useEffect, useRef, useState } from 'react';
import { cn } from '@/utils/cn';
import { Flame, Sparkles, ZapOff, Maximize, Settings, ChevronDown } from 'lucide-react';
import ColorThemeDot from './ColorThemeDot';
import { IngestHealthIndicator } from './IngestHealthIndicator';
import DebtIndicator from './DebtIndicator';
import EditableChapterTitle from './EditableChapterTitle';
import { displayChapterTitle } from '../utils/displayChapterTitle';
import { useAgencyAgentActivity } from '../useAgencyAgentActivity';

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
  /** 当前故事全部章节（顶栏章节下拉列表数据源） */
  chapters?: Chapter[];
  /** 下拉框选中章节时回调（切换幕前正文为该章节） */
  onSelectChapter?: (chapter: Chapter) => void;
  /** 下拉框打开时回调（用于拉取全量章节列表，分页场景本地可能只是部分页） */
  onOpenChapterList?: () => void;
  wordCount: number;
  totalWordCount: number;
  fontSize: number;
  isSaved: boolean;
  /** v0.33.x: 保存重试耗尽后的可见错误（非 null 时顶栏显示可点击的重试入口） */
  saveError?: string | null;
  /** v0.33.x: 点击「保存失败，点击重试」时触发重新 flush */
  onRetrySave?: () => void;
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

const FrontstageHeader: React.FC<FrontstageHeaderProps> = ({
  currentStory,
  displayTitle,
  canRename,
  currentChapter,
  displayChapterTitleText,
  canRenameChapter = false,
  chapters,
  onSelectChapter,
  onOpenChapterList,
  wordCount,
  totalWordCount,
  fontSize,
  isSaved,
  saveError,
  onRetrySave,
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
  const titleBeforeEditRef = useRef('');

  // v0.30.17: 幕前顶部显示创世三 Agent（主创/管理/编辑审计）的动作与进度。
  const { lines: agentLines, hasActivity: hasAgentActivity } = useAgencyAgentActivity();

  // 章节切换下拉：单击章节名展开章节列表，选中即切换幕前正文；
  // 双击仍保留改名（用延迟单击区分单/双击）。
  const [chapterMenuOpen, setChapterMenuOpen] = useState(false);
  const chapterSwitcherRef = useRef<HTMLDivElement>(null);
  const chapterClickTimerRef = useRef<number | null>(null);

  const closeChapterMenu = useCallback(() => setChapterMenuOpen(false), []);

  const toggleChapterMenu = useCallback(() => {
    setChapterMenuOpen(prev => {
      const next = !prev;
      if (next) onOpenChapterList?.();
      return next;
    });
  }, [onOpenChapterList]);

  const handleChapterTitleClick = useCallback(() => {
    if (chapterClickTimerRef.current !== null) {
      window.clearTimeout(chapterClickTimerRef.current);
    }
    chapterClickTimerRef.current = window.setTimeout(() => {
      chapterClickTimerRef.current = null;
      toggleChapterMenu();
    }, 250);
  }, [toggleChapterMenu]);

  const cancelChapterTitleClick = useCallback(() => {
    if (chapterClickTimerRef.current !== null) {
      window.clearTimeout(chapterClickTimerRef.current);
      chapterClickTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    if (!chapterMenuOpen) return;
    const handleMouseDown = (e: MouseEvent) => {
      if (!chapterSwitcherRef.current?.contains(e.target as Node)) {
        setChapterMenuOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setChapterMenuOpen(false);
    };
    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [chapterMenuOpen]);

  useEffect(
    () => () => {
      if (chapterClickTimerRef.current !== null) {
        window.clearTimeout(chapterClickTimerRef.current);
      }
    },
    []
  );

  const sortedChapters = (chapters ?? [])
    .slice()
    .sort((a, b) => a.chapter_number - b.chapter_number);

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

  // v0.26.53: 故事名不再单击回幕后（与双击改名冲突）；回幕后走右侧设置按钮。
  const handleTitleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
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
    <header
      className={[
        'frontstage-header',
        'fixed top-0 left-0 right-0 z-40',
        'flex items-center justify-between px-6 py-3',
        'bg-paper-100/90 backdrop-blur-sm border-b border-paper-300',
      ].join(' ')}
    >
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
            className={cn('frontstage-story-name', canRename && 'frontstage-story-name-renamable')}
            onDoubleClick={handleTitleDoubleClick}
            title={canRename ? '双击改名' : undefined}
          >
            {displayTitle}
          </span>
        )}
        <div className="frontstage-status-bar">
          {currentChapter && (
            <div
              className="chapter-switcher"
              ref={chapterSwitcherRef}
              onDoubleClickCapture={cancelChapterTitleClick}
            >
              <span
                className="chapter-switcher-label"
                onClick={e => {
                  // 改名输入框内的点击不触发下拉
                  if ((e.target as HTMLElement).tagName === 'INPUT') return;
                  handleChapterTitleClick();
                }}
                title="单击切换章节，双击改名"
              >
                <EditableChapterTitle
                  displayTitle={displayChapterTitleText ?? displayChapterTitle(currentChapter)}
                  canRename={canRenameChapter}
                  onRename={onRenameChapter}
                  variant="status"
                />
              </span>
              <ChevronDown
                className={cn('chapter-switcher-chevron', chapterMenuOpen && 'open')}
                onClick={toggleChapterMenu}
                aria-label="展开章节列表"
              />
              {chapterMenuOpen && (
                <div className="chapter-switcher-menu" role="listbox" aria-label="章节列表">
                  {sortedChapters.length === 0 ? (
                    <div className="chapter-switcher-empty">暂无章节</div>
                  ) : (
                    sortedChapters.map(ch => (
                      <button
                        key={ch.id}
                        type="button"
                        role="option"
                        aria-selected={ch.id === currentChapter.id}
                        className={cn(
                          'chapter-switcher-item',
                          ch.id === currentChapter.id && 'active'
                        )}
                        onClick={() => {
                          closeChapterMenu();
                          if (ch.id !== currentChapter.id) onSelectChapter?.(ch);
                        }}
                      >
                        {displayChapterTitle(ch)}
                      </button>
                    ))
                  )}
                </div>
              )}
            </div>
          )}
          <span className="status-separator">·</span>
          <span className="status-item tabular-nums" title="当前章节字数 / 全文字数">
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
          {saveError ? (
            <>
              <span className="status-separator">·</span>
              <span
                className="status-item error cursor-pointer"
                title={`保存失败：${saveError}（点击重试）`}
                onClick={onRetrySave}
              >
                保存失败，点击重试
              </span>
            </>
          ) : (
            !isSaved && (
              <>
                <span className="status-separator">·</span>
                <span className="status-item saving">保存中...</span>
              </>
            )
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
          {hasAgentActivity &&
            agentLines.map((line, idx) => (
              <React.Fragment key={idx}>
                <span className="status-separator">·</span>
                <span
                  className={cn('status-item', line.done ? 'saved' : 'saving')}
                  title="创世多 Agent 进度（主创 / 管理 / 编辑审计）"
                >
                  {line.text}
                </span>
              </React.Fragment>
            ))}
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

      <div className="frontstage-header-right">
        {!isZenMode && (
          <>
            <DebtIndicator
              chapterId={currentChapter?.id || null}
              storyId={currentStory?.id || null}
            />
            <IngestHealthIndicator storyId={currentStory?.id || null} />
            <ColorThemeDot isZenMode={isZenMode} />
          </>
        )}
        {/* 故事名不再单击回幕后；设置按钮为回幕后入口（禅模式也保留） */}
        <button
          className="settings-btn"
          onClick={onOpenBackstage}
          title="打开设置 / 幕后工作室"
          aria-label="打开设置 / 幕后工作室"
        >
          <Settings className="w-3.5 h-3.5" />
        </button>
        {!isZenMode && (
          <>
            <button
              className={cn('wensi-mode-toggle', `wensi-${wensiMode}`)}
              onClick={onCycleWensiMode}
              title={wensiTooltip}
              aria-label={wensiTooltip}
            >
              <span className="wensi-icon">
                {wensiMode === 'active' ? (
                  // v0.73.12: 活跃态用跳动火焰，与被动态的静态星星形成对比
                  <Flame className="w-3.5 h-3.5 wensi-flame" />
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
          </>
        )}
      </div>
    </header>
  );
};

export default React.memo(FrontstageHeader);
