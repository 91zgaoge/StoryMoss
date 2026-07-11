import { useCallback, useEffect, useState } from 'react';
import { emit, listen } from '@tauri-apps/api/event';
import { useAppStore } from '@/stores/appStore';
import { defaultStyle } from '@/frontstage/config/writingStyles';
import { createLogger } from '@/utils/logger';
import type { EditorConfig } from '@/types/editor';

const editorConfigLogger = createLogger('hooks:contracts:useEditorConfig');

export const STORAGE_KEY = 'storymoss-editor-config';

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
    useAppStore.getState().setEditorConfig(config);
    void emit('editor-config-changed', config);
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
    const applyConfig = (cfg: EditorConfig) => {
      setConfig(cfg);
      useAppStore.getState().setEditorConfig(cfg);
    };

    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY || e.key === null) {
        applyConfig(loadEditorConfig());
      }
    };
    window.addEventListener('storage', handleStorageChange);

    let unlisten: (() => void) | undefined;
    void listen<EditorConfig>('editor-config-changed', event => {
      applyConfig(event.payload);
    })
      .then(fn => {
        unlisten = fn;
      })
      .catch(() => {
        /* non-Tauri / test env */
      });

    return () => {
      window.removeEventListener('storage', handleStorageChange);
      unlisten?.();
    };
  }, []);

  const saveConfig = useCallback((updates: Partial<EditorConfig>) => {
    const newConfig = { ...loadEditorConfig(), ...updates };
    saveEditorConfig(newConfig);
    setConfig(newConfig);
  }, []);

  return { config, setConfig: saveConfig, reload: () => setConfig(loadEditorConfig()) };
}
