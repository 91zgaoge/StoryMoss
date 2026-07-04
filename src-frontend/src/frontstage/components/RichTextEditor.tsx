/**
 * RichTextEditor - 富文本编辑器组件 (v4.0)
 *
 * 极简沉浸式写作编辑器
 * - 编辑器内 / 命令菜单
 * - 精简修订模式横幅
 * - 批注与评论统一入口
 */

import React, {
  Component,
  useEffect,
  useCallback,
  forwardRef,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import Underline from '@tiptap/extension-underline';
import Highlight from '@tiptap/extension-highlight';
import { Sparkles, X, Check } from 'lucide-react';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import type { Character } from '@/types/index';
import { CharacterCardPopup } from './CharacterCardPopup';
import { CharacterPeekCard } from './CharacterPeekCard';
import { getCharacterByName } from '@/services/tauri';
import type { CharacterQuickView } from '@/services/tauri';
import { loadEditorConfig, type EditorConfig } from '@/components/EditorSettings';
import { defaultStyle } from '@/frontstage/config/writingStyles';
import { getCurrentEditorColors } from '@/frontstage/config/colorThemes';
import { useSubscription } from '@/hooks/useSubscription';
import { createLogger } from '@/utils/logger';

const rtEditorLogger = createLogger('ui:frontstage:RichTextEditor');

// v0.26.3: 纯文本指纹比较函数，用于 setContent 去抖和幽灵文本重复检测
const textFingerprint = (s: string) =>
  s
    .replace(/<[^>]*>/g, '')
    .replace(/\s+/g, '')
    .replace(
      /[\u3002\uff01\uff1f.!?，、；：""''（）《》\[\]【】…—～·\u201c\u201d\u2018\u2019]/g,
      ''
    )
    .slice(0, 500);

// v0.26.3: 计算两个字符串的最长公共子串长度，用于判断幽灵文本是否已被编辑器包含（即使幽灵文本是正文的前缀/片段）
const longestCommonSubstringLength = (a: string, b: string) => {
  if (!a || !b) return 0;
  let max = 0;
  const dp = Array(b.length + 1).fill(0);
  for (let i = 1; i <= a.length; i++) {
    let prev = 0;
    for (let j = 1; j <= b.length; j++) {
      const temp = dp[j];
      if (a[i - 1] === b[j - 1]) {
        dp[j] = prev + 1;
        max = Math.max(max, dp[j]);
      } else {
        dp[j] = 0;
      }
      prev = temp;
    }
  }
  return max;
};
import { smartExecute, formatText } from '@/services/tauri';
import { AiSuggestionNode } from '../tiptap/AiSuggestionNode';
// Phase 4: SceneDividerNode 不再在幕前编辑器中渲染
// import { SceneDividerNode } from '@/frontstage/extensions/SceneDividerNode';
import { EditorContextMenu } from './EditorContextMenu';

interface RichTextEditorProps {
  content: string;
  onChange: (content: string) => void;
  placeholder?: string;
  className?: string;
  characters?: Character[];
  /** 文思三态：关闭 / 被动提示 / 主动辅助 */
  wensiMode?: 'off' | 'passive' | 'active';
  generatedText?: string;
  isGenerating?: boolean;
  onAcceptGeneration?: () => void;
  onRejectGeneration?: () => void;
  fontSize?: number;
  onFontSizeChange?: (size: number) => void;
  isZenMode?: boolean;
  onZenModeChange?: (zen: boolean) => void;
  storyId?: string;
  chapterId?: string;
  chapterNumber?: number;
  /** 请求 AI 生成（供 Ctrl+Enter / 自动续写 等明确续写调用） */
  onRequestGeneration?: (instruction?: string) => void;
  /** 智能生成入口（供 / 输入框自由指令调用，走意图引擎解析） */
  onSmartGeneration?: (userInput: string) => void;
  /** Slash 命令回调（自动续写/审校/评点等） */
  onSlashCommand?: (commandId: string) => void;
  /** 智能文思 Ghost Text 建议 */
  smartGhostText?: string;
  /** v0.23.98: 父组件控制的幽灵文本隐藏截止时间戳（跨 remount 有效） */
  hideGhostUntil?: number;
  /** v0.24.00: 父组件传入的后端日志函数，用于在 RichTextEditor 内部记录关键诊断 */
  logToBackend?: (phase: string, message: string, details?: Record<string, unknown>) => void;
  /** 统一状态提示回调（替代黑色 toast） */
  onShowStatus?: (message: string) => void;
  /** 内联修改建议 */
  inlineSuggestion?: {
    instruction: string;
    targetText: string;
    category: string;
    targetParagraphIndex: number;
  } | null;
  onClearInlineSuggestion?: () => void;
  /** 订阅状态 */
  subscription?: {
    tier: string;
    isPro: boolean;
    isFree: boolean;
    hasAutoWriteQuota?: (chars: number) => Promise<boolean>;
    hasAutoReviseQuota?: (chars: number) => Promise<boolean>;
    getQuotaText?: () => string;
  };
}

export interface RichTextEditorRef {
  insertText: (text: string) => void;
  /** 在文档末尾追加文本（用于 AI 续写接受后始终追加到正文最后） */
  appendText: (text: string) => void;
  getText: () => string;
  getSelectedText: () => string;
  focus: () => void;
  setContent: (text: string) => void;
  /** 加载聚合场景内容 — 将多个 Scene 用 divider 拼接后写入编辑器 */
  loadAggregatedScenes: (
    scenes: Array<{
      id: string;
      sequence_number: number;
      title?: string | null;
      content?: string | null;
    }>
  ) => void;
}

// v0.24.9: 包裹 EditorContent 的局部错误边界，TipTap 渲染异常时只让编辑器区域降级，
// 而不是让整个 Frontstage 被顶层 ErrorBoundary 捕获白屏。同时把错误详情写回后端日志。
interface EditorContentBoundaryProps {
  children: React.ReactNode;
  editorProps?: {
    logToBackend?: (phase: string, message: string, details?: Record<string, unknown>) => void;
  };
  onError?: (error: Error, info: { componentStack: string }) => void;
}

class EditorContentBoundary extends Component<EditorContentBoundaryProps, { hasError: boolean }> {
  constructor(props: EditorContentBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): { hasError: boolean } {
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: { componentStack: string }) {
    rtEditorLogger.error('[RichTextEditor.EditorContentBoundary] TipTap render error', {
      error,
      errorInfo,
    });
    try {
      this.props.editorProps?.logToBackend?.(
        'frontstage:crash:tiptap_render',
        `${error.name || 'Error'}: ${error.message || String(error)}`,
        {
          stack: (error.stack || '').slice(0, 4000),
          componentStack: (errorInfo.componentStack || '').slice(0, 2000),
        }
      );
    } catch {
      // ignore
    }
    this.props.onError?.(error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex items-center justify-center h-full p-8 text-center">
          <div>
            <p className="text-gray-400 mb-4">编辑器渲染出现异常</p>
            <button
              type="button"
              onClick={() => this.setState({ hasError: false })}
              className="px-4 py-2 bg-cinema-gold text-cinema-950 rounded-lg hover:bg-cinema-gold-light transition-colors"
            >
              重试
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

const RichTextEditor = forwardRef<RichTextEditorRef, RichTextEditorProps>(
  (
    {
      content,
      onChange,
      placeholder = '开始写作...',
      className,
      characters = [],
      wensiMode = 'off',
      generatedText = '',
      isGenerating = false,
      onAcceptGeneration,
      onRejectGeneration,
      fontSize: externalFontSize,
      onFontSizeChange,
      isZenMode = false,
      onZenModeChange,
      storyId,
      chapterId,
      chapterNumber,
      onRequestGeneration,
      onSmartGeneration,
      onSlashCommand,
      smartGhostText,
      hideGhostUntil = 0,
      logToBackend,
      onShowStatus,
      inlineSuggestion,
      onClearInlineSuggestion,
      subscription,
    },
    ref
  ) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [editorConfig, setEditorConfig] = useState<EditorConfig>(loadEditorConfig());
    const [isAiThinking, setIsAiThinking] = useState(false);
    // v0.23.90: Tab 按下瞬间立即隐藏幽灵文本，避免 flushSync/异步状态更新延迟导致双份显示
    const [isHidingGhost, setIsHidingGhost] = useState(false);
    // v0.23.96: Tab 接受后 30s 内强制不渲染幽灵文本，作为 post-accept lock 的渲染层兜底
    // v0.23.98: 与父组件 hideGhostUntil 合并，确保 remount 后仍然强制隐藏
    const postAcceptHideUntilRef = useRef(hideGhostUntil);

    // v0.25.2: 密集诊断日志计数器，用于追踪渲染和状态变化
    const renderCountRef = useRef(0);
    const lastLogTsRef = useRef(0);
    const logRenderDiagnostics = useCallback(
      (label: string, extra?: Record<string, unknown>) => {
        const now = Date.now();
        // 限制日志频率，避免诊断本身影响性能
        if (now - lastLogTsRef.current < 50 && label !== 'render') return;
        lastLogTsRef.current = now;
        const ed = editorRef.current;
        logToBackend?.('frontstage:rich_editor_diag', `${label}`, {
          renderCount: renderCountRef.current,
          generatedTextLen: generatedText?.length ?? 0,
          isHidingGhost,
          isGenerating,
          hideGhostUntilRemaining: hideGhostUntil - now,
          postAcceptHideRemaining: postAcceptHideUntilRef.current - now,
          editorHtmlLen: ed?.getHTML()?.length ?? 0,
          editorTextLen: ed?.getText()?.length ?? 0,
          editorEmpty: ed?.isEmpty ?? null,
          hasFocus: ed?.isFocused ?? null,
          ...extra,
        });
      },
      [generatedText, isHidingGhost, isGenerating, hideGhostUntil, logToBackend]
    );

    // 选区状态（用于角色卡片弹窗）
    const [selectedRange, setSelectedRange] = useState<{
      from: number;
      to: number;
      text: string;
    } | null>(null);

    // ===== 编辑器内 Slash 指令输入框 =====
    const [showSlashInput, setShowSlashInput] = useState(false);
    const [slashInputText, setSlashInputText] = useState('');
    const [slashInputPos, setSlashInputPos] = useState({ x: 0, y: 0 });
    const slashInputRef = useRef<HTMLInputElement>(null);

    // 右键菜单状态
    const [contextMenu, setContextMenu] = useState<{ visible: boolean; x: number; y: number }>({
      visible: false,
      x: 0,
      y: 0,
    });

    // 角色卡片弹窗状态
    const [selectedCharacter, setSelectedCharacter] = useState<Character | null>(null);
    const [popupPosition, setPopupPosition] = useState({ x: 0, y: 0 });
    const [showPopup, setShowPopup] = useState(false);
    const [popupAnchor, setPopupAnchor] = useState<HTMLElement | null>(null);

    // 角色 hover peek 状态
    const [peekCharacter, setPeekCharacter] = useState<CharacterQuickView | null>(null);
    const [peekPosition, setPeekPosition] = useState({ x: 0, y: 0 });
    const [showPeek, setShowPeek] = useState(false);
    const peekTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const currentHoverNameRef = useRef<string | null>(null);

    // 同步状态到 ref，避免 useEditor 闭包问题
    const showSlashInputRef = useRef(showSlashInput);
    useEffect(() => {
      showSlashInputRef.current = showSlashInput;
    }, [showSlashInput]);

    // onChange 同步到 ref，避免 debounce 闭包过期
    const onChangeRef = useRef(onChange);
    onChangeRef.current = onChange;

    // HTML 序列化防抖：按键时先用 getText() 更新轻量状态，200ms 后再序列化完整 HTML
    const htmlDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const latestTextRef = useRef('');
    const editorRef = useRef<ReturnType<typeof useEditor> | null>(null);

    // v0.23.23: 标记外部内容同步（非用户编辑），防止 setContent 触发 onUpdate → 伪"保存中"
    const isExternalSyncRef = useRef(false);

    const editor = useEditor({
      extensions: [
        StarterKit.configure({
          heading: { levels: [1, 2, 3] },
          bulletList: { keepMarks: true, keepAttributes: false },
          orderedList: { keepMarks: true, keepAttributes: false },
        }),
        Placeholder.configure({ placeholder }),
        Underline,
        Highlight.configure({ multicolor: true }),
        AiSuggestionNode,
        // Phase 4: SceneDividerNode 从幕前编辑器移除——幕前为纯正文阅读/写作界面，
        // 场景边界标记仅保留在数据库层供后台使用。
      ],
      content,
      onUpdate: ({ editor }) => {
        // v0.23.23: 外部内容同步（editor.commands.setContent）触发的 onUpdate 不调 onChange
        if (isExternalSyncRef.current) {
          logRenderDiagnostics('onUpdate_skipped_external_sync', {
            htmlLen: editor.getHTML().length,
          });
          return;
        }
        // 轻量文本更新（字数/状态等低耗时场景）
        latestTextRef.current = editor.getText();
        logRenderDiagnostics('onUpdate_fired', {
          htmlLen: editor.getHTML().length,
          textLen: editor.getText().length,
        });

        if (htmlDebounceRef.current) {
          clearTimeout(htmlDebounceRef.current);
        }
        htmlDebounceRef.current = setTimeout(() => {
          htmlDebounceRef.current = null;
          logRenderDiagnostics('onChange_debounced_fired', {
            htmlLen: editor.getHTML().length,
          });
          onChangeRef.current(editor.getHTML());
        }, 200);
      },
      editorProps: {
        attributes: {
          class: 'prose prose-lg focus:outline-none',
        },
        handleDOMEvents: {
          mousedown: (view, event) => {
            if ((event as MouseEvent).button === 0) {
              setSelectedRange(null);
            }
            return false;
          },
        },
        handleKeyDown: (view, event) => {
          // Slash 指令输入框 — 首次输入 /
          if (
            event.key === '/' &&
            wensiMode !== 'off' &&
            !isZenMode &&
            !showSlashInputRef.current
          ) {
            // 删除刚输入的 / 字符
            const { from } = view.state.selection;
            const textBefore = view.state.doc.textBetween(Math.max(0, from - 1), from);
            if (textBefore === '/') {
              view.dispatch(view.state.tr.delete(from - 1, from));
            }
            // 计算浮动输入框位置
            const pos = view.state.selection.from;
            const coords = view.coordsAtPos(pos);
            const containerRect = containerRef.current?.getBoundingClientRect();
            if (containerRect) {
              setSlashInputPos({
                x: coords.left - containerRect.left,
                y: coords.bottom - containerRect.top + 4,
              });
            }
            setSlashInputText('');
            setShowSlashInput(true);
            // 聚焦输入框（下一轮渲染后）
            setTimeout(() => slashInputRef.current?.focus(), 0);
            return true;
          }

          return false;
        },
      },
    });

    // 将 editor 实例同步到 ref，供卸载时 flush 最终 HTML
    editorRef.current = editor;

    // 卸载时清除防抖并 flush 最终 HTML
    useEffect(() => {
      return () => {
        if (htmlDebounceRef.current) {
          clearTimeout(htmlDebounceRef.current);
          htmlDebounceRef.current = null;
        }
        if (editorRef.current) {
          try {
            onChangeRef.current(editorRef.current.getHTML());
          } catch {
            // 编辑器已销毁时忽略
          }
        }
      };
    }, []);

    // W2-F2: 监听配置变化（替代 editor-config-changed DOM CustomEvent）
    const storeEditorConfig = useAppStore(state => state.editorConfig);
    useEffect(() => {
      if (storeEditorConfig) {
        setEditorConfig(storeEditorConfig);
      }
    }, [storeEditorConfig]);

    // 跨窗口同步：监听 localStorage 变化
    useEffect(() => {
      const handleStorageChange = () => {
        setEditorConfig(loadEditorConfig());
      };
      window.addEventListener('storage', handleStorageChange);
      return () => {
        window.removeEventListener('storage', handleStorageChange);
      };
    }, []);

    // W4-F6: E2E 性能基准支持 — 暴露 editor 实例供 benchmark 测试使用
    useEffect(() => {
      if (!editor) return;
      const handleExpose = (e: Event) => {
        const customEvent = e as CustomEvent;
        if (customEvent.detail?.callback) {
          customEvent.detail.callback(editor);
        }
      };
      window.addEventListener('__expose_editor_for_benchmark__', handleExpose);
      return () => {
        window.removeEventListener('__expose_editor_for_benchmark__', handleExpose);
      };
    }, [editor]);

    // 编辑器区域右键菜单
    useEffect(() => {
      const editorEl = containerRef.current;
      if (!editorEl || !editor) return;

      const handleContextMenu = (e: MouseEvent) => {
        e.preventDefault();
        setContextMenu({ visible: true, x: e.clientX, y: e.clientY });
      };

      const handleMouseDown = (e: MouseEvent) => {
        if (e.button === 2) {
          e.preventDefault();
          setContextMenu({ visible: true, x: e.clientX, y: e.clientY });
        }
      };

      const handleDocumentMouseDown = (e: MouseEvent) => {
        if (e.button === 2) return;
        setContextMenu(prev => (prev.visible ? { ...prev, visible: false } : prev));
      };

      editorEl.addEventListener('contextmenu', handleContextMenu, true);
      editorEl.addEventListener('mousedown', handleMouseDown, true);
      document.addEventListener('mousedown', handleDocumentMouseDown);

      return () => {
        editorEl.removeEventListener('contextmenu', handleContextMenu, true);
        editorEl.removeEventListener('mousedown', handleMouseDown, true);
        document.removeEventListener('mousedown', handleDocumentMouseDown);
      };
    }, [editor]);

    // v0.23.89: 记录 generatedText 变化，便于无 devtools 时追踪幽灵文本生命周期
    useEffect(() => {
      // v0.23.98: 父组件 hideGhostUntil 可能 remount 后才到达，同步到本地 ref
      if (hideGhostUntil > postAcceptHideUntilRef.current) {
        postAcceptHideUntilRef.current = hideGhostUntil;
      }
      if (generatedText && generatedText.length > 10) {
        // v0.24.4: 新的生成内容到达时，解除永久隐藏，让幽灵文本正常渲染
        document.body.classList.remove('force-hide-ghost');
        if (Date.now() < postAcceptHideUntilRef.current) {
          rtEditorLogger.warn('[RichTextEditor] ghost text suppressed by post-accept hide', {
            len: generatedText.length,
            preview: generatedText.slice(0, 80),
            hideMsRemaining: postAcceptHideUntilRef.current - Date.now(),
          });
        } else {
          rtEditorLogger.warn('[RichTextEditor] ghost text rendered', {
            len: generatedText.length,
            preview: generatedText.slice(0, 80),
          });
        }
      }
      // v0.23.90: generatedText 真正清空后，解除本地隐藏锁定
      if (!generatedText && isHidingGhost) {
        logRenderDiagnostics('unlock_hiding_ghost');
        setIsHidingGhost(false);
      }
    }, [generatedText, isHidingGhost, hideGhostUntil, logRenderDiagnostics]);

    // 同步外部内容变化
    // W2-F1: 编辑器有焦点时不强制 setContent，避免保存/同步过程中丢焦点
    // v0.23.23: 用 isExternalSyncRef 标记外部同步，跳过 onUpdate 中的 onChange 回调，
    // 防止启动加载/章节切换时触发伪"保存中"和自动保存
    // v0.23.89: 更严格地防止 React error #185 无限循环：用文本指纹 + 同步次数熔断。
    const lastExternalContentRef = useRef(content);
    const syncAttemptsRef = useRef(0);
    const lastSyncAtRef = useRef(0);
    useEffect(() => {
      if (!editor || editor.isDestroyed) return;
      logRenderDiagnostics('setContent_effect_entry', {
        contentLen: content?.length ?? 0,
        lastExternalContentLen: lastExternalContentRef.current?.length ?? 0,
        isFocused: editor.isFocused,
        hideGhostUntilRemaining: hideGhostUntil - Date.now(),
      });
      if (editor.isFocused) {
        logRenderDiagnostics('setContent_effect_skipped_focused');
        return;
      }
      // v0.24.9: Tab 接受后 30s 内禁止外部 setContent，避免后台同步/保存回写与
      // 刚追加的 AI 正文冲突，导致内容重复或 TipTap 渲染异常。
      if (Date.now() < hideGhostUntil) {
        logRenderDiagnostics('setContent_effect_blocked_hideGhostUntil', {
          remainingMs: hideGhostUntil - Date.now(),
          content_preview: content.slice(0, 80),
        });
        return;
      }
      // 如果当前要同步的内容就是我们最近一次外部设置的内容，说明是 TipTap 规范化
      // 回写的 onUpdate，不需要再次 setContent，防止 React error #185 无限循环。
      if (content === lastExternalContentRef.current) {
        logRenderDiagnostics('setContent_effect_skipped_same_ref');
        return;
      }

      // 熔断：1 秒内同步超过 3 次则停止，避免极端情况下的无限循环
      const now = Date.now();
      if (now - lastSyncAtRef.current > 1000) {
        syncAttemptsRef.current = 0;
      }
      syncAttemptsRef.current += 1;
      lastSyncAtRef.current = now;
      if (syncAttemptsRef.current > 3) {
        rtEditorLogger.error('[RichTextEditor] 外部同步熔断：1 秒内超过 3 次', {
          content_preview: content.slice(0, 80),
          attempts: syncAttemptsRef.current,
        });
        return;
      }

      try {
        const editorHtml = editor.getHTML();

        if (textFingerprint(content) === textFingerprint(editorHtml)) {
          logRenderDiagnostics('setContent_effect_skipped_fingerprint_match', {
            contentLen: content.length,
            editorHtmlLen: editorHtml.length,
          });
          lastExternalContentRef.current = content;
          return;
        }
        logRenderDiagnostics('setContent_effect_executing', {
          content_preview: content.slice(0, 80),
          editor_preview: editorHtml.slice(0, 80),
          attempt: syncAttemptsRef.current,
        });
        isExternalSyncRef.current = true;
        editor.commands.setContent(content || '<p></p>');
        lastExternalContentRef.current = content;
        logRenderDiagnostics('setContent_effect_executed', {
          newEditorHtmlLen: editor.getHTML().length,
        });
        // 在下一个微任务中重置标记，确保 TipTap 同步触发的 onUpdate 能被跳过
        queueMicrotask(() => {
          isExternalSyncRef.current = false;
        });
      } catch (e) {
        rtEditorLogger.error('[RichTextEditor] 外部 setContent 失败', {
          error: e,
          content_length: content.length,
        });
      }
    }, [content, editor]);

    // 选区变化跟踪（用于角色卡片弹窗）
    useEffect(() => {
      if (!editor) return;

      const handleSelectionUpdate = () => {
        const { selection } = editor.state;
        if (selection.empty) {
          setSelectedRange(null);
          return;
        }
        const text = editor.state.doc.textBetween(selection.from, selection.to, '\n');
        if (!text.trim()) {
          setSelectedRange(null);
          return;
        }
        setSelectedRange({ from: selection.from, to: selection.to, text: text.trim() });
      };

      editor.on('selectionUpdate', handleSelectionUpdate);
      return () => {
        editor.off('selectionUpdate', handleSelectionUpdate);
      };
    }, [editor]);

    // 处理角色名点击
    useEffect(() => {
      if (!editor || !containerRef.current || characters.length === 0) return;

      const editorElement = containerRef.current?.querySelector('.ProseMirror');
      if (!editorElement) return;

      const extractWordAtPoint = (node: Node, offset: number): string | null => {
        if (node.nodeType !== Node.TEXT_NODE) return null;
        const text = node.textContent || '';

        let start = offset;
        let end = offset;

        while (start > 0) {
          const char = text[start - 1];
          if (/[\s\n\r.,;:!?，。；：！？""''（）【】]/.test(char)) break;
          start--;
        }

        while (end < text.length) {
          const char = text[end];
          if (/[\s\n\r.,;:!?，。；：！？""''（）【】]/.test(char)) break;
          end++;
        }

        return text.slice(start, end).trim();
      };

      const handleClick = (e: Event) => {
        const mouseEvent = e as MouseEvent;
        const target = mouseEvent.target as HTMLElement;
        const paragraph = target.tagName === 'P' ? target : target.closest('p');
        if (!paragraph) return;

        const selection = window.getSelection();
        if (!selection || selection.rangeCount === 0) return;

        const range = selection.getRangeAt(0);
        let word: string | null = null;

        if (selection.toString().trim()) {
          word = selection.toString().trim();
        } else {
          const node = range.startContainer;
          const offset = range.startOffset;
          word = extractWordAtPoint(node, offset);
        }

        if (word) {
          const character = characters.find(c => c.name === word);
          if (character) {
            if (!selection.toString().trim()) {
              try {
                const textNode = range.startContainer;
                const text = textNode.textContent || '';
                const index = text.indexOf(word);
                if (index >= 0 && textNode.nodeType === Node.TEXT_NODE) {
                  const newRange = document.createRange();
                  newRange.setStart(textNode, index);
                  newRange.setEnd(textNode, index + (word?.length || 0));
                  selection.removeAllRanges();
                  selection.addRange(newRange);
                }
              } catch {
                // ignore
              }
            }

            const rect = paragraph.getBoundingClientRect();
            setPopupPosition({ x: rect.left, y: rect.bottom + 8 });
            setPopupAnchor(paragraph as HTMLElement);
            setSelectedCharacter(character);
            setShowPopup(true);
          }
        }
      };

      (editorElement as HTMLElement).addEventListener('click', handleClick);

      // Hover peek: 检测鼠标悬停在角色名上 600ms 后显示微型卡片
      const handleMouseOver = async (e: MouseEvent) => {
        const target = e.target as HTMLElement;
        if (!target || target.closest('.character-peek-card')) return;

        const range = document.caretRangeFromPoint?.(e.clientX, e.clientY);
        if (!range) return;

        const node = range.startContainer;
        const offset = range.startOffset;
        const word = extractWordAtPoint(node, offset);
        if (!word) return;

        const matched = characters.find(c => c.name === word);
        if (!matched) return;

        if (currentHoverNameRef.current === matched.name) return;
        currentHoverNameRef.current = matched.name;

        // 清除之前的 timer
        if (peekTimerRef.current) {
          clearTimeout(peekTimerRef.current);
        }

        peekTimerRef.current = setTimeout(async () => {
          if (currentHoverNameRef.current !== matched.name) return;
          if (!storyId) return;

          try {
            const data = await getCharacterByName(storyId, matched.name);
            if (data && currentHoverNameRef.current === matched.name) {
              setPeekCharacter(data);
              setPeekPosition({ x: e.clientX, y: e.clientY });
              setShowPeek(true);
            }
          } catch (err) {
            // silent fail
          }
        }, 600);
      };

      const handleMouseOut = (e: MouseEvent) => {
        const related = e.relatedTarget as HTMLElement;
        if (related && related.closest('.character-peek-card')) return;
        currentHoverNameRef.current = null;
        if (peekTimerRef.current) {
          clearTimeout(peekTimerRef.current);
          peekTimerRef.current = null;
        }
        setShowPeek(false);
      };

      (editorElement as HTMLElement).addEventListener('mouseover', handleMouseOver);
      (editorElement as HTMLElement).addEventListener('mouseout', handleMouseOut);

      return () => {
        (editorElement as HTMLElement).removeEventListener('click', handleClick);
        (editorElement as HTMLElement).removeEventListener('mouseover', handleMouseOver);
        (editorElement as HTMLElement).removeEventListener('mouseout', handleMouseOut);
      };
    }, [editor, characters, storyId]);

    // ===== 内联修改建议处理 =====
    useEffect(() => {
      if (!inlineSuggestion || !editor || isAiThinking) return;

      const generateInlineSuggestion = async () => {
        setIsAiThinking(true);
        try {
          // W3-F3: Inline Suggestion 统一走 smart_execute（Orchestrator Full 模式）
          const result = await smartExecute({
            user_input: inlineSuggestion.instruction,
            current_content: editor.getHTML() || '',
            selected_text: inlineSuggestion.targetText,
          });

          if (result.final_content) {
            const paragraphs: { pos: number; nodeSize: number }[] = [];
            editor.state.doc.descendants((node, pos) => {
              if (node.type.name === 'paragraph') {
                paragraphs.push({ pos, nodeSize: node.nodeSize });
              }
            });

            let targetIndex = inlineSuggestion.targetParagraphIndex;
            if (targetIndex < 0 || targetIndex >= paragraphs.length) {
              targetIndex = paragraphs.length - 1;
            }

            editor.commands.insertAiSuggestion(
              {
                suggestionId: `inline-${Date.now()}-${Math.random().toString(36).substr(2, 5)}`,
                category: inlineSuggestion.category,
                priority: 'high',
                originalText: inlineSuggestion.targetText,
                targetParagraphIndex: targetIndex,
                storyId: storyId || '',
              },
              result.final_content
            );
          }
        } catch (err) {
          rtEditorLogger.error('Inline suggestion generation failed', { error: err });
          const msg = err instanceof Error ? err.message : String(err);
          onShowStatus?.(`文思生成失败：${msg}`);
        } finally {
          setIsAiThinking(false);
          onClearInlineSuggestion?.();
        }
      };

      generateInlineSuggestion();
    }, [inlineSuggestion, editor, storyId, chapterNumber, isAiThinking, onClearInlineSuggestion]);

    // 智能排版
    const handleFormatText = useCallback(async () => {
      if (!editor || isAiThinking) return;
      const text = editor.getText();
      if (!text.trim()) {
        onShowStatus?.('编辑器内容为空，无法排版');
        return;
      }
      setIsAiThinking(true);
      try {
        const formatted = await formatText(text);
        editor.commands.setContent(`<p>${formatted.replace(/\n/g, '</p><p>')}</p>`);
        onShowStatus?.('排版完成');
      } catch (error) {
        rtEditorLogger.error('Format text error', { error });
        const msg = error instanceof Error ? error.message : String(error);
        onShowStatus?.(`排版失败：${msg}`);
      } finally {
        setIsAiThinking(false);
      }
    }, [editor, isAiThinking]);

    // 处理 slash 输入框的提交 — 所有用户输入统一走 smart_execute（后端模型驱动编排）
    const handleSlashSubmit = useCallback(() => {
      const text = slashInputText.trim();
      if (!text) return;
      setShowSlashInput(false);
      setSlashInputText('');
      // W3-F3: 仅保留需要打开面板的高级命令，其余 AI 生成统一走 smart_execute
      if (text === '自动续写') {
        onSlashCommand?.('auto_write');
      } else if (text === '审校') {
        onSlashCommand?.('auto_revise');
      } else {
        // AI修稿 / AI审稿 / 定稿 / 其他指令 统一由后端意图识别路由
        onSmartGeneration?.(text);
      }
    }, [slashInputText, onSmartGeneration, onSlashCommand]);

    // 关闭 slash 输入框（取消）
    const handleSlashCancel = useCallback(() => {
      setShowSlashInput(false);
      setSlashInputText('');
    }, []);

    // 关闭 slash 输入框并插入 /
    const handleSlashInsertSlash = useCallback(() => {
      setShowSlashInput(false);
      setSlashInputText('');
      if (editor) {
        editor.commands.insertContent('/');
      }
    }, [editor]);

    const handleAcceptAndContinue = useCallback(() => {
      rtEditorLogger.warn('[RichTextEditor] Tab pressed, calling onAcceptGeneration', {
        generatedTextLen: generatedText?.length ?? 0,
        wensiMode,
      });
      // v0.23.90: 立即本地隐藏幽灵文本，不等待父组件状态刷新
      // v0.23.96: 同时设置 30s 渲染层强制隐藏，防止任何竞态导致幽灵文本复现
      // v0.23.99: 直接操作 DOM 给幽灵容器加 force-hide-ghost 类，浏览器立即生效，不受 React 批处理延迟影响
      // v0.24.00: 升级为 document 级 querySelectorAll，确保一定能命中幽灵容器；并记录诊断日志
      // v0.24.1: 提升到 body 级 ghost-hidden，任何时机渲染出的幽灵段落都会被全局 CSS 隐藏
      // v0.24.4: 改为永久隐藏（直到新一次生成开始或页面刷新），杜绝 30s 后 CSS
      // 类被移除、幽灵文本重新露出的可能性。
      setIsHidingGhost(true);
      postAcceptHideUntilRef.current = Date.now() + 30000;
      logRenderDiagnostics('accept_start', {
        generatedTextPreview: generatedText?.slice(0, 100) ?? '',
        editorTextLenBeforeAccept: editor?.getText()?.length ?? 0,
      });
      document.body.classList.add('force-hide-ghost');
      const ghostContainers = document.querySelectorAll('.editor-ghost-continuation');
      logToBackend?.('frontstage:force_hide_ghost', 'attempting to hide ghost containers', {
        found: ghostContainers.length,
        bodyHidden: document.body.classList.contains('force-hide-ghost'),
      });
      ghostContainers.forEach(el => {
        el.classList.add('force-hide-ghost');
      });
      onAcceptGeneration?.();
      if (wensiMode === 'active' && !isZenMode) {
        setTimeout(() => {
          onRequestGeneration?.('续写');
        }, 300);
      }
    }, [onAcceptGeneration, wensiMode, isZenMode, onRequestGeneration, generatedText]);

    // 键盘快捷键（全局，用于接受/拒绝 AI 生成）
    useEffect(() => {
      const handleKeyDown = (e: KeyboardEvent) => {
        if (isZenMode) return;

        if (e.key === 'Tab' && generatedText && handleAcceptAndContinue) {
          e.preventDefault();
          handleAcceptAndContinue();
          return;
        }

        if (e.key === 'Escape' && generatedText && onRejectGeneration) {
          e.preventDefault();
          onRejectGeneration();
          return;
        }
      };

      window.addEventListener('keydown', handleKeyDown);
      return () => window.removeEventListener('keydown', handleKeyDown);
    }, [generatedText, handleAcceptAndContinue, onRejectGeneration, isZenMode]);

    // 暴露方法给父组件
    useImperativeHandle(
      ref,
      () => ({
        insertText: (text: string) => {
          if (editor) {
            if (selectedRange) {
              editor
                .chain()
                .focus()
                .setTextSelection({ from: selectedRange.from, to: selectedRange.to })
                .insertContent(text)
                .run();
            } else {
              editor.chain().focus().insertContent(text).run();
            }
          }
        },
        appendText: (text: string) => {
          if (!editor || editor.isDestroyed) return;
          try {
            const beforeText = editor.getText();
            const endPos = editor.state.doc.content.size;
            editor
              .chain()
              .focus()
              .insertContentAt(endPos, text || '')
              .run();
            // v0.24.9: 追加后立即检查是否出现重复，便于定位“内容重复”根因
            queueMicrotask(() => {
              try {
                const afterText = editor.getText();
                const plain = text.replace(/<[^>]+>/g, '').replace(/\s+/g, '');
                const afterPlain = afterText.replace(/\s+/g, '');
                const occurrences = plain.length > 0 ? afterPlain.split(plain).length - 1 : 0;
                logToBackend?.('frontstage:append_text_check', 'post-append duplicate check', {
                  beforeLen: beforeText.length,
                  afterLen: afterText.length,
                  appendedLen: text.length,
                  occurrences,
                  hasDuplicate: occurrences >= 2,
                  beforePreview: beforeText.slice(0, 120),
                  afterPreview: afterText.slice(0, 120),
                  appendedPreview: text.replace(/<[^>]+>/g, '').slice(0, 120),
                });
              } catch {
                // ignore
              }
            });
          } catch (e) {
            rtEditorLogger.error('[RichTextEditor.appendText] 失败', {
              error: e,
              text_length: text?.length,
            });
          }
        },
        getText: () => {
          try {
            return editor?.getText() || '';
          } catch (e) {
            rtEditorLogger.error('[RichTextEditor.getText] 失败', { error: e });
            return '';
          }
        },
        getSelectedText: () => {
          if (!editor) return '';
          const { from, to } = editor.state.selection;
          if (from === to) return '';
          return editor.state.doc.textBetween(from, to, '\n');
        },
        focus: () => editor?.commands.focus(),
        setContent: (text: string) => {
          if (!editor || editor.isDestroyed) return;
          try {
            isExternalSyncRef.current = true;
            const safeText = text || '';
            editor.commands.setContent(`<p>${safeText.replace(/\n/g, '</p><p>')}</p>`);
            queueMicrotask(() => {
              isExternalSyncRef.current = false;
            });
          } catch (e) {
            rtEditorLogger.error('[RichTextEditor.setContent] 失败', {
              error: e,
              text_length: text?.length,
            });
          }
        },
        loadAggregatedScenes: scenes => {
          if (!editor || editor.isDestroyed) return;
          try {
            isExternalSyncRef.current = true;
            const fragments: string[] = [];
            for (const scene of scenes) {
              const dividerHtml = `<div data-scene-divider="true" data-scene-id="${scene.id}" data-scene-number="${scene.sequence_number}" data-scene-title="${scene.title || ''}"><span class="scene-divider-label">场景 ${scene.sequence_number}${scene.title ? ': ' + scene.title : ''}</span></div>`;
              fragments.push(dividerHtml);
              if (scene.content) {
                fragments.push(scene.content);
              }
            }
            const html = fragments.join('');
            editor.commands.setContent(html || '<p></p>');
            queueMicrotask(() => {
              isExternalSyncRef.current = false;
            });
          } catch (e) {
            rtEditorLogger.error('[RichTextEditor.loadAggregatedScenes] 失败', { error: e });
          }
        },
      }),
      [editor, selectedRange]
    );

    // AI 生成时自动滚动到编辑器底部，让幽灵文本和 Tab/Esc 提示可见
    useEffect(() => {
      if (generatedText || isGenerating) {
        requestAnimationFrame(() => {
          const scrollContainer = containerRef.current?.querySelector(
            '.overflow-auto'
          ) as HTMLElement | null;
          if (scrollContainer) {
            scrollContainer.scrollTo({
              top: scrollContainer.scrollHeight,
              behavior: 'smooth',
            });
          }
        });
      }
    }, [generatedText, isGenerating]);

    if (!editor) return null;

    // v0.25.2: 每次渲染记录关键状态，用于定位幽灵文本/重复内容根因
    renderCountRef.current += 1;
    const editorText = editor.getText();
    const editorFingerprint = textFingerprint(editorText);
    const generatedTextFingerprint = generatedText ? textFingerprint(generatedText) : '';
    // 幽灵文本可能是正文的前缀/片段（用户观察：幽灵文本往往比正文少一部分结尾）。
    // 用最长公共子串判断：若编辑器内容与生成内容有 ≥80% 重叠，就认为已经包含，不再显示幽灵文本。
    const lcsLen = longestCommonSubstringLength(editorFingerprint, generatedTextFingerprint);
    // v0.26.5: 增加完整文本直接包含检测，避免 fingerprint 只取前 500 字符时漏掉
    // "编辑器含用户提示词 + 完整正文 / 幽灵文本只有正文片段" 的场景。
    // v0.26.9 fix: generatedText 可能为 HTML（ContentUpdate/AppendContent 路径会先 autoFormat），
    // 直接包含检测前必须剥离 HTML 标签，否则 plain-text 编辑器内容永远匹配不上 HTML 幽灵文本。
    const normalizedEditorText = editorText.replace(/\s+/g, '');
    const normalizedGeneratedText = generatedText.replace(/<[^>]*>/g, '').replace(/\s+/g, '');
    const editorContainsGeneratedText =
      generatedText.length > 0 &&
      (normalizedEditorText.includes(normalizedGeneratedText) ||
        editorText.includes(generatedText.replace(/<[^>]*>/g, '')) ||
        lcsLen / generatedTextFingerprint.length >= 0.8);
    const shouldShowGhostTree = !!(generatedText || isGenerating) && !isHidingGhost;
    const shouldShowGhostParagraph = !!(
      generatedText &&
      Date.now() > postAcceptHideUntilRef.current &&
      Date.now() > hideGhostUntil &&
      !editorContainsGeneratedText
    );
    if (renderCountRef.current <= 100 || generatedText?.length || isHidingGhost) {
      logRenderDiagnostics('render', {
        shouldShowGhostTree,
        shouldShowGhostParagraph,
        editorContainsGeneratedText,
        lcsLen,
        generatedTextFingerprintLen: generatedTextFingerprint.length,
        generatedTextPreview: generatedText?.slice(0, 60) || '',
      });
    }

    // v0.25.2: 幽灵树可见性变化时单独记录，便于精确追踪
    useEffect(() => {
      logRenderDiagnostics('ghost_visibility_changed', {
        shouldShowGhostTree,
        shouldShowGhostParagraph,
        generatedTextLen: generatedText?.length ?? 0,
      });
    }, [
      shouldShowGhostTree,
      shouldShowGhostParagraph,
      generatedText?.length,
      logRenderDiagnostics,
    ]);

    const currentStyle = defaultStyle;
    const themeColors = getCurrentEditorColors();

    const styleVars = {
      '--fs-font-family': editorConfig.fontFamily,
      '--fs-font-size': externalFontSize ? `${externalFontSize}px` : `${editorConfig.fontSize}px`,
      '--fs-line-height': editorConfig.lineHeight,
      '--fs-letter-spacing': 'normal',
      '--fs-paragraph-spacing': '1.5em',
      '--fs-paper-color': themeColors.paperColor,
      '--fs-ink-color': themeColors.inkColor,
      '--fs-accent-color': themeColors.accentColor,
    } as React.CSSProperties;

    return (
      <div
        ref={containerRef}
        className={cn(
          'rich-text-editor flex flex-col h-full relative',
          isZenMode && 'zen-mode',
          className
        )}
        style={styleVars}
        onContextMenu={e => {
          e.preventDefault();
          e.stopPropagation();
          setContextMenu({ visible: true, x: e.clientX, y: e.clientY });
        }}
      >
        {/* 编辑器内容区 */}
        <div className="flex-1 overflow-auto relative min-h-0">
          <EditorContentBoundary editorProps={{ logToBackend }}>
            <EditorContent editor={editor} />
          </EditorContentBoundary>

          {/* 编辑器内 Slash 指令输入框 */}
          {showSlashInput && (
            <div
              className="editor-slash-input-box"
              style={{ left: slashInputPos.x, top: slashInputPos.y }}
            >
              <input
                ref={slashInputRef}
                type="text"
                value={slashInputText}
                onChange={e => setSlashInputText(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    handleSlashSubmit();
                  } else if (e.key === 'Escape') {
                    e.preventDefault();
                    handleSlashCancel();
                  } else if (e.key === '/') {
                    e.preventDefault();
                    handleSlashInsertSlash();
                  }
                }}
                onBlur={() => {
                  // 延迟关闭，避免点击时先失焦
                  setTimeout(() => {
                    if (document.activeElement !== slashInputRef.current) {
                      handleSlashCancel();
                    }
                  }, 150);
                }}
                placeholder="输入指令，如 续写 / 润色 / 古风，或任意创作要求..."
                className="editor-slash-input"
              />
              <div className="editor-slash-input-hint">
                <span>回车发送</span>
                <span className="hint-dot">·</span>
                <span>再按 / 输出字符</span>
                <span className="hint-dot">·</span>
                <span>Esc 取消</span>
              </div>
            </div>
          )}

          {/* Ghost Text 正文延续 + 生成中指示器 */}
          {/* v0.24.5: 外层也受 isHidingGhost 控制。Tab 接受后整棵幽灵树从 React 树中移除，
              不再依赖 CSS 隐藏兜底，避免任何竞态下幽灵文本继续占位或显示。 */}
          {/* v0.25.2: 增加渲染条件日志和 user-select:none，即使异常渲染也不会被复制。 */}
          {shouldShowGhostTree && (
            <div className="editor-ghost-continuation">
              {shouldShowGhostParagraph && (
                <p
                  className="ghost-paragraph"
                  data-testid="ghost-paragraph"
                  style={{ userSelect: 'none' }}
                >
                  {generatedText}
                </p>
              )}
              {generatedText && (
                <div className="ghost-hint-bar">
                  <kbd className="ghost-kbd">Tab</kbd>
                  <span className="ghost-hint-text">接受</span>
                  <kbd className="ghost-kbd">Esc</kbd>
                  <span className="ghost-hint-text">拒绝</span>
                </div>
              )}
            </div>
          )}

          {/* 右侧边缘萤火提示 */}
          {smartGhostText && wensiMode !== 'off' && !isZenMode && (
            <div key={smartGhostText} className="firefly-sidebar">
              <span className="firefly-dot" />
              <span className="firefly-message">{smartGhostText}</span>
            </div>
          )}

          {/* 空白态引导 */}
          {editor?.isEmpty && wensiMode !== 'off' && !isZenMode && !generatedText && (
            <div className="blank-state-hint">
              <p>开始写下第一句话，文思将随你而行</p>
              <span className="blank-state-sub">按 / 查看可用命令</span>
            </div>
          )}
        </div>

        {/* 编辑器右键菜单 */}
        <EditorContextMenu
          visible={contextMenu.visible}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu({ visible: false, x: 0, y: 0 })}
          editor={editor}
          hasSelection={!!selectedRange}
        />

        {/* 角色卡片弹窗 */}
        <CharacterCardPopup
          character={
            selectedCharacter || { id: '', story_id: '', name: '', created_at: '', updated_at: '' }
          }
          position={popupPosition}
          visible={showPopup}
          onClose={() => setShowPopup(false)}
          anchorEl={popupAnchor}
        />

        {/* 角色 hover 微型卡片 */}
        <CharacterPeekCard character={peekCharacter} position={peekPosition} visible={showPeek} />
      </div>
    );
  }
);

RichTextEditor.displayName = 'RichTextEditor';

export default RichTextEditor;
