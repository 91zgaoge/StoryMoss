/**
 * useWritingStyle - 写作风格管理 Hook
 *
 * 管理编辑器写作风格的状态，并持久化到 localStorage
 */

import { useState, useEffect, useCallback } from 'react';
import { createLogger } from '@/utils/logger';
import {
  WritingStyleId,
  WritingStyle,
  writingStyles,
  defaultStyle,
} from '@/frontstage/config/writingStyles';

const writingStyleLogger = createLogger('hooks:useWritingStyle');

const STORAGE_KEY = 'storyforge-writing-style';

export function useWritingStyle() {
  const [currentStyle, setCurrentStyle] = useState<WritingStyle>(defaultStyle);
  const [isLoaded, setIsLoaded] = useState(false);

  // 从 localStorage 加载保存的风格
  useEffect(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const styleId = saved as WritingStyleId;
        if (writingStyles[styleId]) {
          setCurrentStyle(writingStyles[styleId]);
        }
      }
    } catch (error) {
      writingStyleLogger.error('Failed to load writing style', { error });
    }
    setIsLoaded(true);
  }, []);

  // 切换风格
  const setStyle = useCallback((styleId: WritingStyleId) => {
    const style = writingStyles[styleId];
    if (style) {
      setCurrentStyle(style);
      try {
        localStorage.setItem(STORAGE_KEY, styleId);
      } catch (error) {
        writingStyleLogger.error('Failed to save writing style', { error });
      }
    }
  }, []);

  // 生成 CSS 变量对象
  const getStyleVariables = useCallback(() => {
    return {
      '--fs-font-family': currentStyle.fontFamily,
      '--fs-font-size': `${currentStyle.fontSize}px`,
      '--fs-line-height': currentStyle.lineHeight,
      '--fs-letter-spacing': currentStyle.letterSpacing,
      '--fs-paragraph-spacing': currentStyle.paragraphSpacing,
      '--fs-paper-color': currentStyle.paperColor,
      '--fs-ink-color': currentStyle.inkColor,
      '--fs-accent-color': currentStyle.accentColor,
    } as React.CSSProperties;
  }, [currentStyle]);

  // 应用风格到元素的类名
  const getStyleClassName = useCallback(() => {
    return `writing-style-${currentStyle.id}`;
  }, [currentStyle]);

  return {
    currentStyle,
    setStyle,
    isLoaded,
    getStyleVariables,
    getStyleClassName,
    availableStyles: Object.values(writingStyles),
  };
}
