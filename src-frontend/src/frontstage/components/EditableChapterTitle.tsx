import React, { useCallback, useEffect, useRef, useState } from 'react';
import { cn } from '@/utils/cn';

export interface EditableChapterTitleProps {
  displayTitle: string;
  canRename: boolean;
  onRename?: (title: string) => Promise<void> | void;
  /** 视觉变体：编辑器上方大标题 / 顶栏状态小字 */
  variant?: 'heading' | 'status';
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

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (!canRename || !onRename) return;
      titleBeforeEditRef.current = displayTitle;
      setDraft(displayTitle);
      setEditing(true);
    },
    [canRename, onRename, displayTitle]
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
  return (
    <Tag
      className={cn(
        variant === 'heading' ? 'chapter-title' : 'status-item frontstage-chapter-name',
        variant === 'heading' && isZenMode && 'zen',
        canRename && 'chapter-title-renamable',
        className
      )}
      onDoubleClick={handleDoubleClick}
      title={canRename ? '双击改名' : undefined}
    >
      {displayTitle}
    </Tag>
  );
};

export default EditableChapterTitle;
