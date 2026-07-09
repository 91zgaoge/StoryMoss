import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import {
  loadEditorConfig,
  saveEditorConfig,
  useEditorConfig,
  STORAGE_KEY,
} from '../useEditorConfig';

const mockEmit = vi.fn();
const editorConfigListeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock('@tauri-apps/api/event', () => ({
  emit: (...args: unknown[]) => mockEmit(...args),
  listen: async (channel: string, cb: (event: { payload: unknown }) => void) => {
    editorConfigListeners.set(channel, cb);
    return () => {
      editorConfigListeners.delete(channel);
    };
  },
}));

const mockSetEditorConfig = vi.fn();

vi.mock('@/stores/appStore', () => ({
  useAppStore: Object.assign(
    (selector?: (state: { editorConfig: unknown; setEditorConfig: unknown }) => unknown) => {
      const state = {
        editorConfig: null,
        setEditorConfig: mockSetEditorConfig,
      };
      return selector ? selector(state) : state;
    },
    {
      getState: () => ({
        editorConfig: null,
        setEditorConfig: mockSetEditorConfig,
      }),
    }
  ),
}));

vi.mock('@/frontstage/config/writingStyles', () => ({
  defaultStyle: {
    fontFamily: "'Default Font', serif",
    fontSize: 18,
    lineHeight: 1.8,
  },
}));

vi.mock('@/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
  }),
}));

describe('useEditorConfig contract', () => {
  beforeEach(() => {
    localStorage.clear();
    mockSetEditorConfig.mockClear();
    mockEmit.mockClear();
    editorConfigListeners.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('loadEditorConfig', () => {
    it('当 localStorage 为空时返回默认配置', () => {
      const config = loadEditorConfig();
      expect(config.styleId).toBe('default');
      expect(config.fontSize).toBe(18);
      expect(config.lineHeight).toBe(1.8);
      expect(config.customFonts).toEqual([]);
    });

    it('从 localStorage 解析已保存配置', () => {
      const saved = {
        styleId: 'romantic',
        fontFamily: "'Custom Font', serif",
        fontSize: 20,
        lineHeight: 2,
        customFonts: [{ id: 'f1', name: 'My Font', family: 'MyFont', source: 'custom' as const }],
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(saved));

      const config = loadEditorConfig();
      expect(config).toEqual(saved);
    });

    it('localStorage 数据损坏时返回默认配置', () => {
      localStorage.setItem(STORAGE_KEY, 'not-json');
      const config = loadEditorConfig();
      expect(config.styleId).toBe('default');
      expect(config.fontSize).toBe(18);
    });
  });

  describe('saveEditorConfig', () => {
    it('持久化配置到 localStorage 并同步到 appStore 且广播 Tauri 事件', () => {
      const config = {
        styleId: 'minimal' as const,
        fontFamily: "'Sci-Fi Font', sans-serif",
        fontSize: 22,
        lineHeight: 1.6,
        customFonts: [],
      };

      saveEditorConfig(config);

      const saved = JSON.parse(localStorage.getItem(STORAGE_KEY)!);
      expect(saved).toEqual(config);
      expect(mockSetEditorConfig).toHaveBeenCalledWith(config);
      expect(mockEmit).toHaveBeenCalledWith('editor-config-changed', config);
    });
  });

  describe('useEditorConfig hook', () => {
    it('初始化时加载 localStorage 配置', () => {
      const saved = {
        styleId: 'classical',
        fontFamily: "'Mystery Font', serif",
        fontSize: 21,
        lineHeight: 1.9,
        customFonts: [],
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(saved));

      const { result } = renderHook(() => useEditorConfig());
      expect(result.current.config).toEqual(saved);
    });

    it('setConfig 会合并更新并持久化', () => {
      const { result } = renderHook(() => useEditorConfig());

      act(() => {
        result.current.setConfig({ fontSize: 24 });
      });

      expect(result.current.config.fontSize).toBe(24);
      const saved = JSON.parse(localStorage.getItem(STORAGE_KEY)!);
      expect(saved.fontSize).toBe(24);
      expect(mockSetEditorConfig).toHaveBeenCalled();
    });

    it('响应 storage 事件重新加载配置', async () => {
      const { result } = renderHook(() => useEditorConfig());
      expect(result.current.config.fontSize).toBe(18);

      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          styleId: 'default' as const,
          fontFamily: "'Default Font', serif",
          fontSize: 30,
          lineHeight: 1.8,
          customFonts: [],
        })
      );

      act(() => {
        window.dispatchEvent(new StorageEvent('storage', { key: STORAGE_KEY }));
      });

      await waitFor(() => {
        expect(result.current.config.fontSize).toBe(30);
      });
    });

    it('响应 editor-config-changed Tauri 事件重新加载配置', async () => {
      const { result } = renderHook(() => useEditorConfig());
      await waitFor(() => {
        expect(editorConfigListeners.has('editor-config-changed')).toBe(true);
      });

      const remoteConfig = {
        styleId: 'default' as const,
        fontFamily: "'Remote Font', serif",
        fontSize: 26,
        lineHeight: 1.7,
        customFonts: [],
      };

      act(() => {
        editorConfigListeners.get('editor-config-changed')?.({ payload: remoteConfig });
      });

      await waitFor(() => {
        expect(result.current.config.fontSize).toBe(26);
        expect(result.current.config.fontFamily).toBe("'Remote Font', serif");
      });
    });
  });
});
