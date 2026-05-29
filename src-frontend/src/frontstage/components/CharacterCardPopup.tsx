/**
 * CharacterCardPopup - 角色卡片弹窗组件
 *
 * 在编辑器中点击/悬停角色名时显示的详情卡片
 */

import { useEffect, useRef, useState } from 'react';
import { X, User, BookOpen, Target, Brain } from 'lucide-react';
import { cn } from '@/utils/cn';
import type { Character } from '@/types/index';

interface CharacterCardPopupProps {
  character: Character;
  position: { x: number; y: number };
  visible: boolean;
  onClose: () => void;
  anchorEl?: HTMLElement | null;
}

export function CharacterCardPopup({
  character,
  position,
  visible,
  onClose,
  anchorEl,
}: CharacterCardPopupProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  const [adjustedPosition, setAdjustedPosition] = useState(position);

  // 调整位置避免超出视口
  useEffect(() => {
    if (!visible || !cardRef.current) return;

    const card = cardRef.current;
    const rect = card.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let { x, y } = position;

    // 水平边界检测
    if (x + rect.width > viewportWidth - 16) {
      x = viewportWidth - rect.width - 16;
    }
    if (x < 16) {
      x = 16;
    }

    // 垂直边界检测 - 优先显示在下方，空间不足则显示在上方
    if (y + rect.height > viewportHeight - 16) {
      y = Math.max(16, y - rect.height - (anchorEl?.offsetHeight || 0) - 8);
    }

    setAdjustedPosition({ x, y });
  }, [position, visible, anchorEl]);

  // 点击外部关闭
  useEffect(() => {
    if (!visible) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (cardRef.current && !cardRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [visible, onClose]);

  // ESC 键关闭
  useEffect(() => {
    if (!visible) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [visible, onClose]);

  if (!visible) return null;

  return (
    <div
      ref={cardRef}
      className={cn(
        'fixed z-50 w-80 rounded-xl shadow-2xl border',
        'bg-[var(--parchment)] border-[var(--warm-sand)]',
        'animate-in fade-in slide-in-from-top-2 duration-200'
      )}
      style={{
        left: adjustedPosition.x,
        top: adjustedPosition.y,
      }}
    >
      {/* 头部 */}
      <div className="flex items-center justify-between p-4 border-b border-[var(--warm-sand)]">
        <div className="flex items-center gap-3">
          <div className="w-12 h-12 rounded-full bg-[var(--terracotta)]/10 flex items-center justify-center text-[var(--terracotta)]">
            <User className="w-6 h-6" />
          </div>
          <div>
            <h3 className="font-display text-lg font-semibold text-[var(--charcoal)]">
              {character.name}
            </h3>
            <p className="text-xs text-[var(--stone-gray)]">主要角色</p>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-1.5 rounded-lg hover:bg-[var(--warm-sand)] text-[var(--stone-gray)] transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* 内容 */}
      <div className="p-4 space-y-4 max-h-80 overflow-y-auto">
        {/* 背景故事 */}
        {character.background && (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-[var(--terracotta)]">
              <BookOpen className="w-4 h-4" />
              <span className="text-xs font-medium uppercase tracking-wider">背景故事</span>
            </div>
            <p className="text-sm text-[var(--charcoal)] leading-relaxed">{character.background}</p>
          </div>
        )}

        {/* 性格特点 */}
        {character.personality && (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-[var(--terracotta)]">
              <Brain className="w-4 h-4" />
              <span className="text-xs font-medium uppercase tracking-wider">性格特点</span>
            </div>
            <p className="text-sm text-[var(--charcoal)] leading-relaxed">
              {character.personality}
            </p>
          </div>
        )}

        {/* 目标动机 */}
        {character.goals && (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-[var(--terracotta)]">
              <Target className="w-4 h-4" />
              <span className="text-xs font-medium uppercase tracking-wider">目标与动机</span>
            </div>
            <p className="text-sm text-[var(--charcoal)] leading-relaxed">{character.goals}</p>
          </div>
        )}

        {/* 如果没有详细信息 */}
        {!character.background && !character.personality && !character.goals && (
          <p className="text-sm text-[var(--stone-gray)] text-center py-4">
            暂无详细信息，请在角色管理中编辑
          </p>
        )}
      </div>

      {/* 底部 */}
      <div className="px-4 py-3 border-t border-[var(--warm-sand)] bg-[var(--parchment-dark)]/50 rounded-b-xl">
        <p className="text-xs text-[var(--stone-gray)] text-center">按 ESC 或点击外部关闭</p>
      </div>
    </div>
  );
}
