/**
 * EditorSettings - 编辑器设置组件
 *
 * 包含：
 * - 写作风格选择
 * - 字体家族设置
 * - 字体大小设置
 * - 行高设置
 * - 自定义字体选项
 */

import { useState, useEffect } from 'react';
import { Type, Palette, Check, ChevronDown, Plus, Trash2, Settings2 } from 'lucide-react';
import { cn } from '@/utils/cn';
import { createLogger } from '@/utils/logger';
import { useAppStore } from '@/stores/appStore';
import { WritingStyleId, styleList, defaultStyle } from '@/frontstage/config/writingStyles';
import { getCurrentEditorColors } from '@/frontstage/config/colorThemes';
import type { EditorConfig, CustomFont } from '@/types/editor';

const editorSettingsLogger = createLogger('ui:EditorSettings');

// 预设字体列表
const PRESET_FONTS: CustomFont[] = [
  {
    id: 'lxgw',
    name: '霞鹜文楷',
    family: "'LXGW WenKai', 'Noto Serif SC', 'PingFang SC', serif",
    source: 'google',
  },
  {
    id: 'noto-serif',
    name: '思源宋体',
    family: "'Noto Serif SC', 'Source Han Serif CN', 'LXGW WenKai', serif",
    source: 'google',
  },
  {
    id: 'system-serif',
    name: '系统宋体',
    family: "Georgia, 'STSong', 'SimSun', serif",
    source: 'system',
  },
  {
    id: 'system-sans',
    name: '系统黑体',
    family: "'SF Pro Display', 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif",
    source: 'system',
  },
];

const STORAGE_KEY = 'storyforge-editor-config';

// 加载配置
export function loadEditorConfig(): EditorConfig {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      return {
        styleId: parsed.styleId || 'default',
        fontFamily: parsed.fontFamily || defaultStyle.fontFamily,
        fontSize: parsed.fontSize || defaultStyle.fontSize,
        lineHeight: parsed.lineHeight || defaultStyle.lineHeight,
        customFonts: parsed.customFonts || [],
      };
    }
  } catch {
    editorSettingsLogger.error('Failed to load editor config');
  }
  return {
    styleId: 'default',
    fontFamily: defaultStyle.fontFamily,
    fontSize: defaultStyle.fontSize,
    lineHeight: defaultStyle.lineHeight,
    customFonts: [],
  };
}

// 保存配置
export function saveEditorConfig(config: EditorConfig) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
    // W2-F2: 替代 editor-config-changed DOM CustomEvent，改用 Zustand store
    useAppStore.getState().setEditorConfig(config);
  } catch {
    editorSettingsLogger.error('Failed to save editor config');
  }
}

interface EditorSettingsProps {
  onChange?: (config: EditorConfig) => void;
}

export function EditorSettings({ onChange }: EditorSettingsProps) {
  const [config, setConfig] = useState<EditorConfig>(loadEditorConfig());
  const [showStylePicker, setShowStylePicker] = useState(false);
  const [showFontSettings, setShowFontSettings] = useState(false);
  const [newFontName, setNewFontName] = useState('');
  const [newFontFamily, setNewFontFamily] = useState('');

  const currentStyle = styleList.find(s => s.id === config.styleId) || defaultStyle;
  const themeColors = getCurrentEditorColors();
  const allFonts = [...PRESET_FONTS, ...config.customFonts];

  const updateConfig = (updates: Partial<EditorConfig>) => {
    const newConfig = { ...config, ...updates };
    setConfig(newConfig);
    saveEditorConfig(newConfig);
    onChange?.(newConfig);
  };

  const handleStyleChange = (styleId: WritingStyleId) => {
    const style = styleList.find(s => s.id === styleId);
    if (style) {
      updateConfig({
        styleId,
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        lineHeight: style.lineHeight,
      });
    }
    setShowStylePicker(false);
  };

  const addCustomFont = () => {
    if (newFontName.trim() && newFontFamily.trim()) {
      const newFont: CustomFont = {
        id: `custom-${Date.now()}`,
        name: newFontName.trim(),
        family: newFontFamily.trim(),
        source: 'custom',
      };
      updateConfig({
        customFonts: [...config.customFonts, newFont],
      });
      setNewFontName('');
      setNewFontFamily('');
    }
  };

  const removeCustomFont = (id: string) => {
    updateConfig({
      customFonts: config.customFonts.filter(f => f.id !== id),
    });
  };

  return (
    <div className="space-y-6">
      {/* 写作风格 */}
      <div className="space-y-3">
        <label className="text-sm font-medium text-gray-300 flex items-center gap-2">
          <Palette className="w-4 h-4 text-cinema-gold" />
          写作风格
        </label>

        <div className="relative">
          <button
            onClick={() => setShowStylePicker(!showStylePicker)}
            className="w-full flex items-center justify-between px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-xl text-white hover:border-cinema-gold transition-colors"
          >
            <div className="flex items-center gap-3">
              <div
                className="w-8 h-8 rounded-lg"
                style={{
                  backgroundColor: themeColors.paperColor,
                  border: `2px solid ${themeColors.accentColor}`,
                }}
              />
              <div className="text-left">
                <div className="font-medium">{currentStyle.name}</div>
                <div className="text-xs text-gray-400">{currentStyle.description}</div>
              </div>
            </div>
            <ChevronDown
              className={cn(
                'w-5 h-5 text-gray-400 transition-transform',
                showStylePicker && 'rotate-180'
              )}
            />
          </button>

          {showStylePicker && (
            <div className="absolute top-full left-0 right-0 mt-2 bg-cinema-800 border border-cinema-700 rounded-xl shadow-xl z-50 overflow-hidden">
              <div className="p-2 space-y-1 max-h-80 overflow-y-auto">
                {styleList.map(style => (
                  <button
                    key={style.id}
                    onClick={() => handleStyleChange(style.id)}
                    className={cn(
                      'w-full flex items-center gap-3 px-3 py-3 rounded-lg text-left transition-colors',
                      config.styleId === style.id
                        ? 'bg-cinema-gold/20 border border-cinema-gold/50'
                        : 'hover:bg-cinema-700/50'
                    )}
                  >
                    <div
                      className="w-6 h-6 rounded border-2 flex-shrink-0"
                      style={{
                        backgroundColor: themeColors.paperColor,
                        borderColor: themeColors.accentColor,
                      }}
                    >
                      {config.styleId === style.id && (
                        <Check
                          className="w-4 h-4 m-0.5"
                          style={{ color: themeColors.accentColor }}
                        />
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-white text-sm">{style.name}</span>
                        {style.author && (
                          <span className="text-xs text-gray-500">· {style.author}</span>
                        )}
                      </div>
                      <p className="text-xs text-gray-400 truncate">{style.preview}</p>
                    </div>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* 字体设置 */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="text-sm font-medium text-gray-300 flex items-center gap-2">
            <Type className="w-4 h-4 text-cinema-gold" />
            字体设置
          </label>
          <button
            onClick={() => setShowFontSettings(!showFontSettings)}
            className="text-xs text-cinema-gold hover:text-cinema-gold-light flex items-center gap-1"
          >
            <Settings2 className="w-3 h-3" />
            {showFontSettings ? '收起' : '展开'}
          </button>
        </div>

        {/* 字体家族 */}
        <div>
          <label className="text-xs text-gray-500 mb-2 block">字体</label>
          <select
            value={config.fontFamily}
            onChange={e => updateConfig({ fontFamily: e.target.value })}
            className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
          >
            <optgroup label="预设字体">
              {PRESET_FONTS.map(font => (
                <option key={font.id} value={font.family}>
                  {font.name}
                </option>
              ))}
            </optgroup>
            {config.customFonts.length > 0 && (
              <optgroup label="自定义字体">
                {config.customFonts.map(font => (
                  <option key={font.id} value={font.family}>
                    {font.name}
                  </option>
                ))}
              </optgroup>
            )}
          </select>
        </div>

        {/* 字号和行高 */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="text-xs text-gray-500 mb-2 block">字号 {config.fontSize}px</label>
            <input
              type="range"
              min={12}
              max={32}
              step={1}
              value={config.fontSize}
              onChange={e => updateConfig({ fontSize: Number(e.target.value) })}
              className="w-full accent-cinema-gold"
            />
          </div>
          <div>
            <label className="text-xs text-gray-500 mb-2 block">
              行高 {config.lineHeight.toFixed(1)}
            </label>
            <input
              type="range"
              min={1.2}
              max={3}
              step={0.1}
              value={config.lineHeight}
              onChange={e => updateConfig({ lineHeight: Number(e.target.value) })}
              className="w-full accent-cinema-gold"
            />
          </div>
        </div>

        {/* 高级字体设置 */}
        {showFontSettings && (
          <div className="pt-4 border-t border-cinema-700 space-y-4">
            <h4 className="text-sm font-medium text-gray-300">自定义字体</h4>

            {/* 添加新字体 */}
            <div className="space-y-2">
              <input
                type="text"
                placeholder="字体名称（如：我的字体）"
                value={newFontName}
                onChange={e => setNewFontName(e.target.value)}
                className="w-full px-3 py-2 bg-cinema-900 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
              />
              <input
                type="text"
                placeholder="CSS font-family（如：'My Font', serif）"
                value={newFontFamily}
                onChange={e => setNewFontFamily(e.target.value)}
                className="w-full px-3 py-2 bg-cinema-900 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
              />
              <button
                onClick={addCustomFont}
                disabled={!newFontName.trim() || !newFontFamily.trim()}
                className="w-full px-3 py-2 bg-cinema-gold/20 text-cinema-gold rounded-lg text-sm hover:bg-cinema-gold/30 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
              >
                <Plus className="w-4 h-4" />
                添加字体
              </button>
            </div>

            {/* 自定义字体列表 */}
            {config.customFonts.length > 0 && (
              <div className="space-y-2">
                <label className="text-xs text-gray-500">已添加的字体</label>
                {config.customFonts.map(font => (
                  <div
                    key={font.id}
                    className="flex items-center justify-between px-3 py-2 bg-cinema-900 rounded-lg"
                  >
                    <div>
                      <div className="text-sm text-white">{font.name}</div>
                      <div className="text-xs text-gray-500 font-mono">{font.family}</div>
                    </div>
                    <button
                      onClick={() => removeCustomFont(font.id)}
                      className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-colors"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                ))}
              </div>
            )}

            <div className="text-xs text-gray-500 bg-cinema-900/50 p-3 rounded-lg">
              <p>提示：自定义字体需要系统中已安装该字体，或通过 @import 引入 Web Font。</p>
            </div>
          </div>
        )}
      </div>

      {/* 预览 */}
      <div className="space-y-2">
        <label className="text-xs text-gray-500">预览</label>
        <div
          className="p-4 rounded-xl border border-cinema-700"
          style={{
            fontFamily: config.fontFamily,
            fontSize: `${config.fontSize}px`,
            lineHeight: config.lineHeight,
            backgroundColor: themeColors.paperColor,
            color: themeColors.inkColor,
          }}
        >
          <p>文字是心灵的窗户，每一笔都流淌着思想的温度。</p>
          <p className="mt-2 opacity-70">The quick brown fox jumps over the lazy dog.</p>
        </div>
      </div>
    </div>
  );
}
