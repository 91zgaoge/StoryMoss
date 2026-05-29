/**
 * AiSuggestionBubble - AI 提示意见气泡组件
 *
 * 设计理念：
 * - 如萤火虫般在右侧留白区域随机位置浮现
 * - 灰色小字（Olive Gray oklch(52% 0.01 85)）
 * - 渐变出现 → 停留 → 渐变消失的动效
 * - 不干扰正文阅读，但提供创作灵感
 */

import React, { useState, useEffect, useCallback } from 'react';

export type SuggestionType = 'plot' | 'character' | 'environment' | 'pacing' | 'emotion';

interface Suggestion {
  id: string;
  text: string;
  type: SuggestionType;
  position: { top: number; right: number };
}

interface AiSuggestionBubbleProps {
  /** 是否启用提示 */
  enabled?: boolean;
  /** 提示出现间隔（毫秒） */
  interval?: number;
  /** 提示停留时间（毫秒） */
  duration?: number;
}

const SUGGESTION_TEMPLATES: Record<SuggestionType, string[]> = {
  plot: [
    '情节在此处可稍作转折...',
    '悬念铺垫得恰到好处',
    '冲突升级的时机到了',
    '此处可埋下伏笔',
    '反转即将来临',
    '情节节奏张弛有度',
  ],
  character: [
    '人物心理描写可更深入...',
    '角色动机需要更多铺垫',
    '人物关系可以更加复杂',
    '内心独白能增强代入感',
    '角色的成长弧线清晰',
    '人物对话符合性格',
  ],
  environment: [
    '环境描写可以渲染气氛...',
    '场景转换略显突兀',
    '此处可添加感官细节',
    '天气描写暗示心境',
    '空间布局交代清晰',
    '氛围营造恰到好处',
  ],
  pacing: [
    '节奏可以稍微放缓...',
    '此处适合加快节奏',
    '紧张感逐渐累积',
    '节奏张弛有度',
    '过渡自然流畅',
    '节奏变化恰到好处',
  ],
  emotion: [
    '情感递进自然流畅',
    '此处情感可以更加饱满',
    '情绪转折略显突兀',
    '情感共鸣强烈',
    '情绪渲染到位',
    '情感层次丰富',
  ],
};

const TYPE_LABELS: Record<SuggestionType, string> = {
  plot: '情节',
  character: '人物',
  environment: '环境',
  pacing: '节奏',
  emotion: '情感',
};

const TYPE_ICONS: Record<SuggestionType, string> = {
  plot: '↻',
  character: '👤',
  environment: '📍',
  pacing: '⚡',
  emotion: '♥',
};

export const AiSuggestionBubble: React.FC<AiSuggestionBubbleProps> = ({
  enabled = true,
  interval = 12000,
  duration = 8000,
}) => {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [visibleIds, setVisibleIds] = useState<Set<string>>(new Set());

  const generateRandomPosition = useCallback((): { top: number; right: number } => {
    // 在右侧 10%-40% 的区域内随机
    const top = 15 + Math.random() * 60; // 15% - 75%
    const right = 5 + Math.random() * 25; // 5% - 30%
    return { top, right };
  }, []);

  const generateSuggestion = useCallback((): Suggestion => {
    const types = Object.keys(SUGGESTION_TEMPLATES) as SuggestionType[];
    const type = types[Math.floor(Math.random() * types.length)];
    const texts = SUGGESTION_TEMPLATES[type];
    const text = texts[Math.floor(Math.random() * texts.length)];

    return {
      id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      text,
      type,
      position: generateRandomPosition(),
    };
  }, [generateRandomPosition]);

  useEffect(() => {
    if (!enabled) {
      setSuggestions([]);
      setVisibleIds(new Set());
      return;
    }

    // 初始延迟，给用户准备时间
    const initialDelay = setTimeout(() => {
      // 立即显示第一个提示
      const first = generateSuggestion();
      setSuggestions([first]);

      // 渐显动画
      requestAnimationFrame(() => {
        setVisibleIds(prev => new Set([...prev, first.id]));
      });

      // 设置消失定时器
      setTimeout(() => {
        setVisibleIds(prev => {
          const next = new Set(prev);
          next.delete(first.id);
          return next;
        });

        // 完全移除
        setTimeout(() => {
          setSuggestions(prev => prev.filter(s => s.id !== first.id));
        }, 600);
      }, duration);
    }, 2000);

    // 定期生成新提示
    const intervalId = setInterval(() => {
      // 限制同时显示的提示数量
      setSuggestions(prev => {
        if (prev.length >= 3) {
          // 移除最旧的一个
          const [oldest, ...rest] = prev;
          setVisibleIds(ids => {
            const next = new Set(ids);
            next.delete(oldest.id);
            return next;
          });
          return rest;
        }
        return prev;
      });

      // 随机决定是否生成新提示（70% 概率）
      if (Math.random() > 0.3) {
        const newSuggestion = generateSuggestion();

        setSuggestions(prev => [...prev, newSuggestion]);

        // 渐显
        requestAnimationFrame(() => {
          setVisibleIds(prev => new Set([...prev, newSuggestion.id]));
        });

        // 设置消失定时器
        setTimeout(() => {
          setVisibleIds(prev => {
            const next = new Set(prev);
            next.delete(newSuggestion.id);
            return next;
          });

          setTimeout(() => {
            setSuggestions(prev => prev.filter(s => s.id !== newSuggestion.id));
          }, 600);
        }, duration);
      }
    }, interval);

    return () => {
      clearTimeout(initialDelay);
      clearInterval(intervalId);
    };
  }, [enabled, interval, duration, generateSuggestion]);

  if (!enabled || suggestions.length === 0) {
    return null;
  }

  return (
    <div className="ai-suggestion-container">
      {suggestions.map(suggestion => (
        <div
          key={suggestion.id}
          className={`ai-suggestion-bubble ${visibleIds.has(suggestion.id) ? 'visible' : ''}`}
          style={{
            top: `${suggestion.position.top}%`,
            right: `${suggestion.position.right}%`,
          }}
        >
          {/* 呼吸光点 */}
          <div className="ai-suggestion-pulse">
            <span className="ai-suggestion-icon">{TYPE_ICONS[suggestion.type]}</span>
          </div>

          {/* 内容区域 */}
          <div className="ai-suggestion-content">
            <div className="ai-suggestion-type">{TYPE_LABELS[suggestion.type]}</div>
            <div className="ai-suggestion-text">{suggestion.text}</div>
          </div>

          {/* 装饰性光晕 */}
          <div className="ai-suggestion-glow" />
        </div>
      ))}
    </div>
  );
};

/**
 * 浮动环境提示组件
 * 更轻量级的提示，在屏幕边缘浮动
 */
export const FloatingAmbientHint: React.FC<{
  enabled?: boolean;
}> = ({ enabled = true }) => {
  const [hints, setHints] = useState<
    Array<{
      id: string;
      text: string;
      y: number;
      visible: boolean;
    }>
  >([]);

  useEffect(() => {
    if (!enabled) return;

    const ambientTexts = [
      '情节可以更紧凑...',
      '此处可增加细节描写',
      '人物情绪转折自然',
      '文思泉涌，继续书写...',
      '此处留白恰到好处',
      '对话节奏把握得当',
      '氛围渲染甚佳',
      '人物形象逐渐丰满',
    ];

    const timers: ReturnType<typeof setTimeout>[] = [];

    // 创建浮动提示
    const createFloatingHint = (delay: number) => {
      const timer = setTimeout(() => {
        const id = Date.now().toString();
        const y = 20 + Math.random() * 60;
        const text = ambientTexts[Math.floor(Math.random() * ambientTexts.length)];

        setHints(prev => [...prev, { id, text, y, visible: false }]);

        // 渐显
        requestAnimationFrame(() => {
          setHints(prev => prev.map(h => (h.id === id ? { ...h, visible: true } : h)));
        });

        // 6秒后消失
        setTimeout(() => {
          setHints(prev => prev.map(h => (h.id === id ? { ...h, visible: false } : h)));

          setTimeout(() => {
            setHints(prev => prev.filter(h => h.id !== id));
          }, 500);
        }, 6000);

        // 递归创建下一个
        createFloatingHint(8000 + Math.random() * 4000);
      }, delay);

      timers.push(timer);
    };

    // 开始创建
    createFloatingHint(3000);
    createFloatingHint(8000);

    return () => {
      timers.forEach(clearTimeout);
    };
  }, [enabled]);

  if (!enabled) return null;

  return (
    <div className="floating-hints-container">
      {hints.map(hint => (
        <div
          key={hint.id}
          className={`floating-hint ${hint.visible ? 'visible' : ''}`}
          style={{ top: `${hint.y}%` }}
        >
          <span className="floating-hint-pulse-dot" />
          <span className="floating-hint-text">{hint.text}</span>
        </div>
      ))}
    </div>
  );
};

export default AiSuggestionBubble;
