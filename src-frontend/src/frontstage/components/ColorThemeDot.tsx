/**
 * ColorThemeDot - 幕前色调主题切换器
 *
 * 嵌入在顶部 header 中，"开启文思"按钮左侧
 * 平时：12px 半透明小圆点
 * 悬停：展开 4 色选择面板（向下）
 * Zen 模式：隐藏
 */

import React, { useState, useCallback, useEffect, useRef } from 'react';
import { cn } from '@/utils/cn';
import {
  colorThemes,
  colorThemeList,
  type ColorThemeId,
  loadColorTheme,
  saveColorTheme,
  applyColorTheme,
} from '@/frontstage/config/colorThemes';

interface ColorThemeDotProps {
  isZenMode?: boolean;
}

const ColorThemeDot: React.FC<ColorThemeDotProps> = ({ isZenMode = false }) => {
  const [currentThemeId, setCurrentThemeId] = useState<ColorThemeId>(loadColorTheme);
  const [isHovered, setIsHovered] = useState(false);
  const [panelOpen, setPanelOpen] = useState(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 初始化时应用主题
  useEffect(() => {
    applyColorTheme(currentThemeId);
  }, []);

  const handleSelect = useCallback((themeId: ColorThemeId) => {
    setCurrentThemeId(themeId);
    saveColorTheme(themeId);
    applyColorTheme(themeId);
    setPanelOpen(false);
    setIsHovered(false);
  }, []);

  const handleMouseEnter = useCallback(() => {
    if (hideTimer.current) {
      clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    setIsHovered(true);
    setPanelOpen(true);
  }, []);

  const handleMouseLeave = useCallback(() => {
    hideTimer.current = setTimeout(() => {
      setPanelOpen(false);
      setIsHovered(false);
    }, 200);
  }, []);

  // Zen 模式隐藏
  if (isZenMode) return null;

  const currentTheme = colorThemes[currentThemeId];

  return (
    <div
      className="color-theme-dot-wrapper"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* 选择面板 - 向下展开 */}
      <div className={cn('color-theme-panel', panelOpen && 'open')}>
        <div className="color-theme-panel-title">色调</div>
        <div className="color-theme-options">
          {colorThemeList.map(theme => (
            <button
              key={theme.id}
              className={cn('color-theme-option', currentThemeId === theme.id && 'active')}
              onClick={() => handleSelect(theme.id)}
              title={theme.description}
            >
              <span className="color-theme-swatch" style={{ backgroundColor: theme.terracotta }} />
              <span className="color-theme-label">{theme.name}</span>
            </button>
          ))}
        </div>
      </div>

      {/* 状态点 */}
      <div
        className={cn('color-theme-dot', isHovered && 'hovered')}
        style={{ backgroundColor: currentTheme.terracotta }}
        title="切换色调主题"
      />
    </div>
  );
};

export default ColorThemeDot;
