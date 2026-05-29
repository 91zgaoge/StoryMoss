/**
 * CharacterPeekCard - 角色微型悬浮卡片 (v6.0.1)
 *
 * 在编辑器中 hover 角色名 600ms 后显示的微型卡片
 * 120px 宽，只读，显示核心信息
 */

import { useEffect, useRef, useState } from 'react';
import { cn } from '@/utils/cn';
import type { CharacterQuickView } from '@/services/tauri';

interface CharacterPeekCardProps {
  character: CharacterQuickView | null;
  position: { x: number; y: number };
  visible: boolean;
}

export function CharacterPeekCard({ character, position, visible }: CharacterPeekCardProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  const [adjustedPosition, setAdjustedPosition] = useState(position);

  useEffect(() => {
    if (!visible || !cardRef.current) return;

    const card = cardRef.current;
    const rect = card.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let { x, y } = position;

    // 优先显示在元素上方，空间不足则下方
    if (y - rect.height - 8 < 16) {
      y = y + 20; // 下方
    } else {
      y = y - rect.height - 8; // 上方
    }

    // 水平边界
    if (x + rect.width > viewportWidth - 16) {
      x = viewportWidth - rect.width - 16;
    }
    if (x < 16) {
      x = 16;
    }

    setAdjustedPosition({ x, y });
  }, [position, visible]);

  if (!visible || !character) return null;

  return (
    <div
      ref={cardRef}
      className={cn(
        'fixed z-50 rounded-lg shadow-xl border',
        'bg-[var(--parchment)] border-[var(--warm-sand)]',
        'animate-in fade-in zoom-in-95 duration-150'
      )}
      style={{
        left: adjustedPosition.x,
        top: adjustedPosition.y,
        width: '140px',
      }}
    >
      <div className="p-2.5">
        <p className="text-sm font-semibold text-[var(--charcoal)] truncate">{character.name}</p>

        {character.status_tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1.5">
            {character.status_tags.slice(0, 3).map(tag => (
              <span
                key={tag}
                className="text-[10px] px-1 py-0.5 rounded bg-[var(--terracotta)]/10 text-[var(--terracotta)]"
              >
                {tag}
              </span>
            ))}
          </div>
        )}

        {character.appearance_summary && (
          <p className="text-[10px] text-[var(--stone-gray)] mt-1.5 line-clamp-2 leading-relaxed">
            {character.appearance_summary}
          </p>
        )}

        <p className="text-[10px] text-[var(--stone-gray)]/70 mt-1.5 text-right">
          上次出场: 第{character.last_seen_chapter}场
        </p>
      </div>
    </div>
  );
}
