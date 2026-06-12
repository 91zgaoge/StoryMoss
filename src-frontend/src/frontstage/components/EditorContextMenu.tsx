import React, { useEffect, useRef, useState } from 'react';
import { Scissors, Copy, Clipboard, CheckSquare } from 'lucide-react';
import { cn } from '@/utils/cn';

interface EditorContextMenuProps {
  visible: boolean;
  x: number;
  y: number;
  onClose: () => void;
  editor: any;
  hasSelection: boolean;
}

export const EditorContextMenu: React.FC<EditorContextMenuProps> = ({
  visible,
  x,
  y,
  onClose,
  editor,
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

  const items = [
    { id: 'cut', label: '剪切', icon: Scissors, onClick: handleCut, disabled: !hasSelection },
    { id: 'copy', label: '复制', icon: Copy, onClick: handleCopy, disabled: !hasSelection },
    { id: 'paste', label: '粘贴', icon: Clipboard, onClick: handlePaste, disabled: false },
    {
      id: 'select-all',
      label: '全选',
      icon: CheckSquare,
      onClick: handleSelectAll,
      disabled: false,
    },
  ];

  return (
    <div
      ref={menuRef}
      onMouseDown={e => {
        e.preventDefault();
        e.stopPropagation();
      }}
      className="editor-context-menu"
      style={{ left: pos.x, top: pos.y }}
    >
      {items.map((item, index) => {
        const Icon = item.icon;
        return (
          <React.Fragment key={item.id}>
            {index > 0 && <div className="editor-context-menu-divider" />}
            <button
              onClick={!item.disabled ? item.onClick : undefined}
              disabled={item.disabled}
              className={cn(
                'editor-context-menu-item',
                item.disabled && 'editor-context-menu-item-disabled'
              )}
            >
              <Icon className="editor-context-menu-icon" />
              <span className="editor-context-menu-label">{item.label}</span>
            </button>
          </React.Fragment>
        );
      })}
    </div>
  );
};
