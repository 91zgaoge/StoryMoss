import { useCallback, useEffect, useState } from 'react';
import { useAppStore } from '@/stores/appStore';
import { defaultStyle } from '@/frontstage/config/writingStyles';
import { createLogger } from '@/utils/logger';
import type { EditorConfig } from '@/types/editor';

const editorConfigLogger = createLogger('hooks:contracts:useEditorConfig');

export const STORAGE_KEY = 'storyforge-editor-config';

/** 从 localStorage 加载编辑器配置 */
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
    editorConfigLogger.error('Failed to load editor config');
  }
  return {
    styleId: 'default',
    fontFamily: defaultStyle.fontFamily,
    fontSize: defaultStyle.fontSize,
    lineHeight: defaultStyle.lineHeight,
    customFonts: [],
  };
}

/** 保存编辑器配置到 localStorage 并同步到 appStore */
export function saveEditorConfig(config: EditorConfig) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
    // W2-F2: 替代 editor-config-changed DOM CustomEvent，改用 Zustand store
    useAppStore.getState().setEditorConfig(config);
  } catch {
    editorConfigLogger.error('Failed to save editor config');
  }
}

/**
 * 编辑器配置 Hook
 *
 * 封装配置加载与持久化，订阅跨窗口 storage 事件与 appStore 变更。
 */
export function useEditorConfig() {
  const [config, setConfig] = useState<EditorConfig>(loadEditorConfig);
  const storeConfig = useAppStore(state => state.editorConfig);

  useEffect(() => {
    if (storeConfig) {
      setConfig(storeConfig);
    }
  }, [storeConfig]);

  useEffect(() => {
    const handleStorageChange = () => {
      setConfig(loadEditorConfig());
    };
    window.addEventListener('storage', handleStorageChange);
    return () => {
      window.removeEventListener('storage', handleStorageChange);
    };
  }, []);

  const saveConfig = useCallback((updates: Partial<EditorConfig>) => {
    const newConfig = { ...loadEditorConfig(), ...updates };
    saveEditorConfig(newConfig);
    setConfig(newConfig);
  }, []);

  return { config, setConfig: saveConfig, reload: () => setConfig(loadEditorConfig()) };
}
