/**
 * AiHintOverlay - AI 提示浮现动效组件
 *
 * 设计理念：
 * - 灰色小字如文思泉涌般浮现
 * - 渐变出现、停留、消失的动画
 * - 不干扰正文阅读，但提供创作灵感
 */

import { useEffect, useState } from 'react';
import type { AiHint } from '../types';

interface AiHintOverlayProps {
  hints: AiHint[];
}

interface PositionedHint extends AiHint {
  top: number;
  left: number;
  opacity: number;
  scale: number;
}

export function AiHintOverlay({ hints }: AiHintOverlayProps) {
  const [positionedHints, setPositionedHints] = useState<PositionedHint[]>([]);

  // Calculate positions and animate hints
  useEffect(() => {
    const calculatePositions = () => {
      const editor = document.querySelector('.reader-writer-editor') as HTMLElement;
      if (!editor) return;

      const editorRect = editor.getBoundingClientRect();
      const lineHeight = parseInt(getComputedStyle(editor).lineHeight) || 28;
      const charWidth = 14; // Approximate character width

      const positioned = hints.map((hint, index) => {
        // Calculate position based on line and column
        const top = editorRect.top + (hint.position.line - 1) * lineHeight + 40; // +40 for padding
        const left = editorRect.left + hint.position.column * charWidth + 20;

        // Stagger animations for multiple hints
        const delay = index * 200;

        return {
          ...hint,
          top: top - editorRect.top + 60, // Relative to container
          left: left - editorRect.left,
          opacity: 0,
          scale: 0.9,
        };
      });

      setPositionedHints(positioned);

      // Animate in
      requestAnimationFrame(() => {
        setPositionedHints(prev =>
          prev.map(h => ({
            ...h,
            opacity: 0.6,
            scale: 1,
          }))
        );
      });
    };

    calculatePositions();
  }, [hints]);

  if (hints.length === 0) {
    return null;
  }

  return (
    <div className="ai-hint-overlay">
      {positionedHints.map(hint => (
        <div
          key={hint.id}
          className={`ai-hint-bubble ${hint.isPreview ? 'preview' : ''}`}
          style={{
            top: `${hint.top}px`,
            left: `${hint.left}px`,
            opacity: hint.opacity,
            transform: `scale(${hint.scale})`,
            transition: 'all 0.5s cubic-bezier(0.4, 0, 0.2, 1)',
          }}
        >
          <div className="ai-hint-content">
            <span className="ai-hint-icon">✦</span>
            <span className="ai-hint-text">{hint.text}</span>
          </div>
          <div className="ai-hint-glow"></div>
        </div>
      ))}

      {/* Ambient hints that float around */}
      <div className="ambient-hints">
        <FloatingHint text="情节可以更紧凑..." delay={0} />
        <FloatingHint text="此处可增加细节描写" delay={2000} />
        <FloatingHint text="人物情绪转折自然" delay={4000} />
      </div>
    </div>
  );
}

// Floating ambient hint component
function FloatingHint({ text, delay }: { text: string; delay: number }) {
  const [visible, setVisible] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });

  useEffect(() => {
    // Random position on right side
    const randomY = 20 + Math.random() * 60; // 20% to 80% of viewport height
    setPosition({ x: 85, y: randomY });

    const timer = setTimeout(() => {
      setVisible(true);

      // Hide after 6 seconds
      setTimeout(() => {
        setVisible(false);
      }, 6000);
    }, delay);

    return () => clearTimeout(timer);
  }, [delay]);

  return (
    <div
      className={`floating-hint ${visible ? 'visible' : ''}`}
      style={{
        top: `${position.y}%`,
        right: '20px',
        transitionDelay: `${delay}ms`,
      }}
    >
      <span className="floating-hint-pulse"></span>
      <span className="floating-hint-text">{text}</span>
    </div>
  );
}
