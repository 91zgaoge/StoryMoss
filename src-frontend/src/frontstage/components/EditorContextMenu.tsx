import React, { useEffect, useRef, useState } from 'react';
import {
  GitBranch,
  Quote,
  Check,
  Loader2,
  Scissors,
  Copy,
  Clipboard,
  CheckSquare,
} from 'lucide-react';
import { cn } from '@/utils/cn';

interface EditorContextMenuProps {
  visible: boolean;
  x: number;
  y: number;
  onClose: () => void;
  editor: any;
  isRevisionMode: boolean;
  onToggleRevision: () => void;
  onGenerateCommentary: () => void;
  isGeneratingCommentary: boolean;
  hasSelection: boolean;
}

export const EditorContextMenu: React.FC<EditorContextMenuProps> = ({
  visible,
  x,
  y,
  onClose,
  editor,
  isRevisionMode,
  onToggleRevision,
  onGenerateCommentary,
  isGeneratingCommentary,
  hasSelection,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  useEffect(() => {
    if (!visible || !menuRef.current) return;
    const rect = menuRef.current.getBoundingClientRect();
    const winW = window.innerWidth;
    const winH = window.innerHeight;
    let nextX = x;
    let nextY = y;
    if (x + rect.width > winW) nextX = winW - rect.width - 8;
    if (y + rect.height > winH) nextY = winH - rect.height - 8;
    if (nextX < 8) nextX = 8;
    if (nextY < 8) nextY = 8;
    setPos({ x: nextX, y: nextY });
  }, [visible, x, y]);

  useEffect(() => {
    if (!visible) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('keydown', handleKey);
    };
  }, [visible, onClose]);

  if (!visible) return null;

  const handleCut = () => {
    editor?.commands.focus();
    document.execCommand('cut');
    onClose();
  };

  const handleCopy = () => {
    document.execCommand('copy');
    onClose();
  };

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      editor?.commands.focus();
      editor?.commands.insertContent(text);
    } catch {
      // ignore
    }
    onClose();
  };

  const handleSelectAll = () => {
    editor?.commands.selectAll();
    onClose();
  };

  const MenuItem = ({
    onClick,
    disabled,
    children,
  }: {
    onClick?: () => void;
    disabled?: boolean;
    children: React.ReactNode;
  }) => (
    <button
      onClick={!disabled ? onClick : undefined}
      disabled={disabled}
      className={cn(
        'w-full text-left px-3 py-2 rounded-lg text-sm flex items-center gap-2 transition-colors',
        disabled
          ? 'text-[var(--stone-gray)]/60 cursor-not-allowed'
          : 'hover:bg-[var(--warm-sand)] active:scale-[0.98] text-[var(--charcoal)]'
      )}
    >
      {children}
    </button>
  );

  const Divider = () => <div className="h-px bg-[var(--charcoal)]/10 my-1" />;

  return (
    <div
      ref={menuRef}
      onMouseDown={e => {
        e.preventDefault();
        e.stopPropagation();
      }}
      className="fixed z-[9999] bg-[var(--ivory)] border border-[var(--warm-sand)] rounded-xl shadow-xl p-1.5 min-w-[160px] animate-fade-in text-[var(--charcoal)]"
      style={{ left: pos.x, top: pos.y }}
    >
      <div className="grid grid-cols-3 gap-0.5">
        <button
          onClick={handleCut}
          className="flex flex-col items-center justify-center py-2 rounded-lg hover:bg-[var(--warm-sand)] active:scale-[0.98] text-[var(--charcoal)]"
        >
          <Scissors className="w-4 h-4 mb-1" />
          <span className="text-[10px]">剪切</span>
        </button>
        <button
          onClick={handleCopy}
          className="flex flex-col items-center justify-center py-2 rounded-lg hover:bg-[var(--warm-sand)] active:scale-[0.98] text-[var(--charcoal)]"
        >
          <Copy className="w-4 h-4 mb-1" />
          <span className="text-[10px]">复制</span>
        </button>
        <button
          onClick={handlePaste}
          className="flex flex-col items-center justify-center py-2 rounded-lg hover:bg-[var(--warm-sand)] active:scale-[0.98] text-[var(--charcoal)]"
        >
          <Clipboard className="w-4 h-4 mb-1" />
          <span className="text-[10px]">粘贴</span>
        </button>
      </div>

      <Divider />

      <MenuItem onClick={onToggleRevision}>
        <GitBranch
          className={cn(
            'w-4 h-4',
            isRevisionMode ? 'text-[var(--terracotta)]' : 'text-[var(--stone-gray)]'
          )}
        />
        <span className="flex-1">修订模式</span>
        {isRevisionMode && <Check className="w-4 h-4 text-[var(--terracotta)]" />}
      </MenuItem>

      <Divider />

      <MenuItem onClick={onGenerateCommentary} disabled={isGeneratingCommentary}>
        {isGeneratingCommentary ? (
          <Loader2 className="w-4 h-4 animate-spin text-[var(--stone-gray)]" />
        ) : (
          <Quote className="w-4 h-4 text-[var(--stone-gray)]" />
        )}
        <span className="flex-1">生成古典评点</span>
      </MenuItem>

      <MenuItem onClick={handleSelectAll}>
        <CheckSquare className="w-4 h-4 text-[var(--stone-gray)]" />
        <span className="flex-1">全选</span>
      </MenuItem>
    </div>
  );
};
