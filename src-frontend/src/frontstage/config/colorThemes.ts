/**
 * 幕前冷暖撞色色调主题系统
 *
 * 4 组色调：暖赭(默认)、冷青、琥珀、靛紫
 * 平时以右下角状态点呈现，悬停展开选择面板
 */

import { emit } from '@tauri-apps/api/event';
import { createLogger } from '@/utils/logger';

const colorThemeLogger = createLogger('hooks:colorThemes');

export type ColorThemeId = 'warm' | 'cool' | 'amber' | 'indigo';

export interface ColorTheme {
  id: ColorThemeId;
  name: string;
  description: string;
  /** 核心背景色 --parchment */
  parchment: string;
  /** 次级表面 --parchment-dark */
  parchmentDark: string;
  /** hover/边框 --warm-sand */
  warmSand: string;
  /**  subtle 边框 --border-cream */
  borderCream: string;
  /** 主强调色 --terracotta */
  terracotta: string;
  /** 强调 hover --terracotta-light */
  terracottaLight: string;
  /** 强调 active --terracotta-dark */
  terracottaDark: string;
  /** 主文字 --charcoal */
  charcoal: string;
  /** 次级文字 --charcoal-light */
  charcoalLight: string;
  /** 柔和文字 --olive-gray */
  oliveGray: string;
  /** placeholder --stone-gray */
  stoneGray: string;
  /** 标题/强文字 --ink */
  ink: string;
  /** 输入框背景 --ivory */
  ivory: string;
  /** 装饰高亮 --gold */
  gold: string;
}

export const COLOR_THEME_STORAGE_KEY = 'storymoss-color-theme';
const STORAGE_KEY = COLOR_THEME_STORAGE_KEY;

function deriveTheme(
  id: ColorThemeId,
  name: string,
  description: string,
  parchment: string,
  terracotta: string,
  charcoal: string
): ColorTheme {
  // OKLCH 解析辅助: oklch(L% C H)
  const parse = (s: string) => {
    const m = s.match(/oklch\(([\d.]+)%\s+([\d.]+)\s+(\d+(?:\.\d+)?)\)/);
    if (!m) return { l: 0, c: 0, h: 0 };
    return { l: parseFloat(m[1]), c: parseFloat(m[2]), h: parseFloat(m[3]) };
  };
  const fmt = (l: number, c: number, h: number) =>
    `oklch(${l.toFixed(1)}% ${c.toFixed(3)} ${h.toFixed(0)})`;

  const p = parse(parchment);
  const t = parse(terracotta);
  const ch = parse(charcoal);

  return {
    id,
    name,
    description,
    parchment,
    parchmentDark: fmt(p.l - 3, p.c * 1.2, p.h),
    warmSand: fmt(p.l - 5.5, p.c * 1.5, p.h),
    borderCream: fmt(p.l - 2.5, p.c * 1.1, p.h),
    terracotta,
    terracottaLight: fmt(Math.min(t.l + 6, 78), t.c * 1.08, t.h),
    terracottaDark: fmt(t.l - 6, t.c * 0.92, t.h),
    charcoal,
    charcoalLight: fmt(Math.min(ch.l + 10, 65), ch.c * 0.8, ch.h),
    oliveGray: fmt(Math.min(ch.l + 14, 68), ch.c * 0.65, ch.h),
    stoneGray: fmt(Math.min(ch.l + 20, 72), ch.c * 0.5, ch.h),
    ink: fmt(Math.max(ch.l - 13, 12), ch.c * 1.1, ch.h),
    ivory: fmt(Math.min(p.l + 1.5, 99.5), p.c * 0.6, p.h),
    gold: fmt(Math.min(t.l + 14, 80), Math.max(t.c - 0.03, 0.05), (t.h + 40) % 360),
  };
}

export const colorThemes: Record<ColorThemeId, ColorTheme> = {
  warm: deriveTheme(
    'warm',
    '暖赭',
    '暖色纸张 + 赭红强调，默认温馨氛围',
    'oklch(96.5% 0.008 95)',
    'oklch(58% 0.13 45)',
    'oklch(38% 0.015 85)'
  ),
  cool: deriveTheme(
    'cool',
    '冷青',
    '冷白灰蓝 + 青色强调，清新理性',
    'oklch(97% 0.012 220)',
    'oklch(52% 0.14 200)',
    'oklch(32% 0.02 220)'
  ),
  amber: deriveTheme(
    'amber',
    '琥珀',
    '暖米黄 + 琥珀橙强调，温润古典',
    'oklch(96% 0.015 85)',
    'oklch(60% 0.16 55)',
    'oklch(35% 0.018 80)'
  ),
  indigo: deriveTheme(
    'indigo',
    '靛紫',
    '冷灰白 + 靛蓝强调，沉静深邃',
    'oklch(98% 0.008 280)',
    'oklch(50% 0.18 270)',
    'oklch(32% 0.02 270)'
  ),
};

export const defaultColorTheme: ColorTheme = colorThemes.warm;
export const colorThemeList = Object.values(colorThemes);

/** 加载保存的色调主题 */
export function loadColorTheme(): ColorThemeId {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && saved in colorThemes) return saved as ColorThemeId;
  } catch {
    colorThemeLogger.error('Failed to load color theme');
  }
  return 'warm';
}

/** 保存色调主题 */
export function saveColorTheme(themeId: ColorThemeId) {
  try {
    localStorage.setItem(STORAGE_KEY, themeId);
    void emit('color-theme-changed', themeId);
  } catch {
    colorThemeLogger.error('Failed to save color theme');
  }
}

/** 将主题应用到 document.documentElement CSS 变量 */
export function applyColorTheme(themeId: ColorThemeId) {
  const theme = colorThemes[themeId] || defaultColorTheme;
  const root = document.documentElement;

  const vars: Record<string, string> = {
    '--parchment': theme.parchment,
    '--parchment-dark': theme.parchmentDark,
    '--warm-sand': theme.warmSand,
    '--border-cream': theme.borderCream,
    '--terracotta': theme.terracotta,
    '--terracotta-light': theme.terracottaLight,
    '--terracotta-dark': theme.terracottaDark,
    '--charcoal': theme.charcoal,
    '--charcoal-light': theme.charcoalLight,
    '--olive-gray': theme.oliveGray,
    '--stone-gray': theme.stoneGray,
    '--ink': theme.ink,
    '--ivory': theme.ivory,
    '--gold': theme.gold,
    '--text-on-accent': 'oklch(100% 0 0)',
  };

  Object.entries(vars).forEach(([key, value]) => {
    root.style.setProperty(key, value);
  });
}

/** 获取当前主题的编辑器配色 */
export function getCurrentEditorColors(themeId?: ColorThemeId) {
  const id = themeId || loadColorTheme();
  const theme = colorThemes[id] || defaultColorTheme;
  return {
    paperColor: theme.parchment,
    inkColor: theme.ink,
    accentColor: theme.terracotta,
  };
}
