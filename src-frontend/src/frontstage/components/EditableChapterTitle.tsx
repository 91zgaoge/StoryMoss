import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Loader2, Pencil, Sparkles } from 'lucide-react';
import { cn } from '@/utils/cn';

export interface EditableChapterTitleProps {
  displayTitle: string;
  canRename: boolean;
  onRename?: (title: string) => Promise<void> | void;
  /** 视觉变体：编辑器上方大标题 / 顶栏状态小字 */
  variant?: 'heading' | 'status';
  /** 大标题旁「AI 自动取名」入口；传入才渲染按钮 */
  onAutoName?: () => Promise<void> | void;
  /** 自动取名请求进行中（按钮转圈并禁用） */
  autoNaming?: boolean;
  className?: string;
  isZenMode?: boolean;
}

/**
 * 章节标题内联编辑（双击编辑、Enter/blur 提交、Esc/空 blur 取消；无单击导航）。
 */
const EditableChapterTitle: React.FC<EditableChapterTitleProps> = ({
  displayTitle,
  canRename,
  onRename,
  variant = 'heading',
  onAutoName,
  autoNaming = false,
  className,
  isZenMode = false,
}) => {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const titleBeforeEditRef = useRef('');

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
    if (!onRename) {
      cancelEdit();
      return;
    }
    setSaving(true);
    try {
      await onRename(next);
      setEditing(false);
      setDraft('');
    } catch {
      // 失败时保持编辑态，便于重试
    } finally {
      setSaving(false);
    }
  }, [draft, onRename, saving, cancelEdit]);

  const startEditing = useCallback(() => {
    if (!canRename || !onRename) return;
    titleBeforeEditRef.current = displayTitle;
    setDraft(displayTitle);
    setEditing(true);
  }, [canRename, onRename, displayTitle]);

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      startEditing();
    },
    [startEditing]
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

  if (!displayTitle && !editing) return null;

  if (editing) {
    return (
      <input
        ref={inputRef}
        className={cn(
          variant === 'heading' ? 'chapter-title-input' : 'frontstage-chapter-name-input',
          className
        )}
        value={draft}
        disabled={saving}
        aria-label="编辑章节名称"
        onChange={e => setDraft(e.target.value)}
        onBlur={() => {
          void commitEdit();
        }}
        onKeyDown={handleInputKeyDown}
      />
    );
  }

  const Tag = variant === 'heading' ? 'h1' : 'span';
  const titleEl = (
    <Tag
      className={cn(
        variant === 'heading' ? 'chapter-title' : 'status-item frontstage-chapter-name',
        variant === 'heading' && isZenMode && 'zen',
        canRename && 'chapter-title-renamable',
        className
      )}
      onDoubleClick={handleDoubleClick}
      title={canRename ? '双击改名，或点击右侧铅笔' : undefined}
    >
      {displayTitle}
    </Tag>
  );

  if (variant !== 'heading') return titleEl;

  const canEdit = canRename && !!onRename;
  if (!canEdit && !onAutoName) return titleEl;

  return (
    <div className="chapter-title-row">
      {titleEl}
      {canEdit && (
        <button
          type="button"
          className="chapter-edit-btn"
          aria-label="重命名章节"
          title="重命名章节（也可双击标题）"
          onClick={event => {
            event.preventDefault();
            event.stopPropagation();
            startEditing();
          }}
        >
          <Pencil className="h-3.5 w-3.5" />
        </button>
      )}
      {onAutoName && (
        <button
          type="button"
          className="chapter-auto-name-btn"
          aria-label="AI 自动取名"
          title="AI 自动取名（会覆盖当前标题）"
          disabled={autoNaming}
          onClick={event => {
            event.preventDefault();
            event.stopPropagation();
            void onAutoName();
          }}
        >
          {autoNaming ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Sparkles className="h-3.5 w-3.5" />
          )}
        </button>
      )}
    </div>
  );
};

export default EditableChapterTitle;
