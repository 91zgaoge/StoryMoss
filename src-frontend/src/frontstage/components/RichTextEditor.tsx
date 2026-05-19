/**
 * RichTextEditor - 富文本编辑器组件 (v4.0)
 *
 * 极简沉浸式写作编辑器
 * - 编辑器内 / 命令菜单
 * - 精简修订模式横幅
 * - 批注与评论统一入口
 */

import React, { useEffect, useCallback, forwardRef, useImperativeHandle, useRef, useState } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import Underline from '@tiptap/extension-underline';
import Highlight from '@tiptap/extension-highlight';
import {
  Sparkles,
  Loader2,
  X,
  Check,
  GitBranch,
  CheckCheck,
  Undo2,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import type { Character } from '@/types/index';
import { CharacterCardPopup } from './CharacterCardPopup';
import { CharacterPeekCard } from './CharacterPeekCard';
import { getCharacterByName } from '@/services/tauri';
import type { CharacterQuickView } from '@/services/tauri';
import {
  loadEditorConfig,
  type EditorConfig
} from '@/components/EditorSettings';
import { defaultStyle } from '@/frontstage/config/writingStyles';
import { getCurrentEditorColors } from '@/frontstage/config/colorThemes';
import { useSubscription } from '@/hooks/useSubscription';
import type { ParagraphCommentary } from '@/types/v3';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const rtEditorLogger = createLogger('ui:frontstage:RichTextEditor');
import { generateParagraphCommentaries, smartExecute, formatText } from '@/services/tauri';
import { TrackInsertMark, TrackDeleteMark } from '@/frontstage/extensions/TrackChanges';
import { AiSuggestionNode } from '../tiptap/AiSuggestionNode';
import { SceneDividerNode } from '@/frontstage/extensions/SceneDividerNode';
import { EditorContextMenu } from './EditorContextMenu';
import { usePendingChanges, useTrackChange, useAcceptChange, useRejectChange, useAcceptAllChanges, useRejectAllChanges } from '@/hooks/useChangeTracking';
import type { ChangeTrack } from '@/types/v3';

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
  isRevisionMode?: boolean;
  onRevisionModeChange?: (v: boolean) => void;
  /** 智能文思 Ghost Text 建议 */
  smartGhostText?: string;
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
    dailyUsed: number;
    dailyLimit: number;
    hasQuota: () => Promise<boolean>;
    hasAutoWriteQuota?: (chars: number) => Promise<boolean>;
    hasAutoReviseQuota?: (chars: number) => Promise<boolean>;
    getQuotaText?: () => string;
  };
  /** 配额用尽时的回调 */
  onQuotaExhausted?: () => void;
}

export interface RichTextEditorRef {
  insertText: (text: string) => void;
  getText: () => string;
  getSelectedText: () => string;
  focus: () => void;
  generateCommentary: () => void;
  setContent: (text: string) => void;
  /** 加载聚合场景内容 — 将多个 Scene 用 divider 拼接后写入编辑器 */
  loadAggregatedScenes: (scenes: Array<{ id: string; sequence_number: number; title?: string | null; content?: string | null }>) => void;
}

const RichTextEditor = forwardRef<RichTextEditorRef, RichTextEditorProps>(
  ({
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
    isRevisionMode: externalIsRevisionMode = false,
    onRevisionModeChange,
    smartGhostText,
    inlineSuggestion,
    onClearInlineSuggestion,
    subscription,
    onQuotaExhausted,
  }, ref) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [editorConfig, setEditorConfig] = useState<EditorConfig>(loadEditorConfig());
    const [isAiThinking, setIsAiThinking] = useState(false);
    const [isGeneratingCommentary, setIsGeneratingCommentary] = useState(false);

    // 选区状态（用于角色卡片弹窗）
    const [selectedRange, setSelectedRange] = useState<{ from: number; to: number; text: string } | null>(null);

    // ===== 编辑器内 Slash 指令输入框 =====
    const [showSlashInput, setShowSlashInput] = useState(false);
    const [slashInputText, setSlashInputText] = useState('');
    const [slashInputPos, setSlashInputPos] = useState({ x: 0, y: 0 });
    const slashInputRef = useRef<HTMLInputElement>(null);

    // 修订模式状态（受控）
    const isRevisionMode = externalIsRevisionMode;
    const setIsRevisionMode = (v: boolean) => onRevisionModeChange?.(v);
    const prevTextRef = useRef('');
    const isRevisionModeRef = useRef(isRevisionMode);
    isRevisionModeRef.current = isRevisionMode;
    const chapterIdRef = useRef(chapterId);
    chapterIdRef.current = chapterId;

    // 右键菜单状态
    const [contextMenu, setContextMenu] = useState<{ visible: boolean; x: number; y: number }>({ visible: false, x: 0, y: 0 });
    const { data: pendingChanges = [] } = usePendingChanges(undefined, chapterId || undefined);
    const trackChangeMutation = useTrackChange();
    const acceptChangeMutation = useAcceptChange();
    const rejectChangeMutation = useRejectChange();
    const acceptAllMutation = useAcceptAllChanges();
    const rejectAllMutation = useRejectAllChanges();

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
    useEffect(() => { showSlashInputRef.current = showSlashInput; }, [showSlashInput]);

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
        TrackInsertMark,
        TrackDeleteMark,
        AiSuggestionNode,
        SceneDividerNode,
      ],
      content,
      onUpdate: ({ editor }) => {
        onChange(editor.getHTML());

        if (isRevisionModeRef.current && chapterIdRef.current) {
          const currentText = editor.getText();
          const prevText = prevTextRef.current;

          if (prevText && currentText !== prevText) {
            if (currentText.length > prevText.length) {
              const insertPos = findFirstDiff(prevText, currentText);
              const insertedText = currentText.slice(insertPos, insertPos + (currentText.length - prevText.length));
              if (insertedText.trim()) {
                trackChangeMutation.mutate({
                  chapterId: chapterIdRef.current,
                  changeType: 'Insert',
                  fromPos: insertPos,
                  toPos: insertPos + insertedText.length,
                  content: insertedText,
                });
                const pmPos = textOffsetToPmPosition(editor, insertPos, insertedText.length);
                if (pmPos) {
                  editor.chain().focus().setTextSelection({ from: pmPos.from, to: pmPos.to }).setMark('trackInsert', { changeId: `temp-${Date.now()}` }).setTextSelection(pmPos.to).run();
                }
              }
            } else if (currentText.length < prevText.length) {
              const deletePos = findFirstDiff(prevText, currentText);
              const deletedText = prevText.slice(deletePos, deletePos + (prevText.length - currentText.length));
              if (deletedText.trim()) {
                trackChangeMutation.mutate({
                  chapterId: chapterIdRef.current,
                  changeType: 'Delete',
                  fromPos: deletePos,
                  toPos: deletePos + deletedText.length,
                  content: deletedText,
                });
              }
            }
          }
          prevTextRef.current = currentText;
        }
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
          if (event.key === '/' && wensiMode !== 'off' && !isZenMode && !showSlashInputRef.current) {
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

    // W2-F2: 监听配置变化（替代 editor-config-changed DOM CustomEvent）
    const storeEditorConfig = useAppStore((state) => state.editorConfig);
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

    // 同步外部内容变化
    // W2-F1: 编辑器有焦点时不强制 setContent，避免保存/同步过程中丢焦点
    useEffect(() => {
      if (editor && content !== editor.getHTML() && !editor.isFocused) {
        editor.commands.setContent(content);
      }
    }, [content, editor]);

    // 修订模式：初始化/同步 prevTextRef
    useEffect(() => {
      if (editor) {
        prevTextRef.current = editor.getText();
      }
    }, [editor, isRevisionMode]);

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
          toast.error(`文思生成失败：${msg}`);
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
        toast.error('编辑器内容为空，无法排版');
        return;
      }
      setIsAiThinking(true);
      try {
        const formatted = await formatText(text);
        editor.commands.setContent(`<p>${formatted.replace(/\n/g, '</p><p>')}</p>`);
        toast.success('排版完成');
      } catch (error) {
        rtEditorLogger.error('Format text error', { error });
        const msg = error instanceof Error ? error.message : String(error);
        toast.error(`排版失败：${msg}`);
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
      onAcceptGeneration?.();
      if (wensiMode === 'active' && !isZenMode) {
        setTimeout(() => {
          onRequestGeneration?.('续写');
        }, 300);
      }
    }, [onAcceptGeneration, wensiMode, isZenMode, onRequestGeneration]);

    // 生成古典评点
    const handleGenerateCommentary = useCallback(async () => {
      if (!editor || !storyId) return;

      const text = editor.getText();
      if (!text.trim()) return;

      setIsGeneratingCommentary(true);
      try {
        const result = await generateParagraphCommentaries({
          story_id: storyId,
          story_title: '',
          genre: '',
          text,
        });

        const commentaries: ParagraphCommentary[] = JSON.parse(result);
        if (!commentaries.length) return;

        const paragraphs: { pos: number; nodeSize: number }[] = [];
        editor.state.doc.descendants((node, pos) => {
          if (node.type.name === 'paragraph') {
            paragraphs.push({ pos, nodeSize: node.nodeSize });
          }
        });

        const chain = editor.chain().focus();
        const sorted = [...commentaries]
          .filter(c => c.paragraph_index < paragraphs.length)
          .sort((a, b) => b.paragraph_index - a.paragraph_index);

        for (const c of sorted) {
          const para = paragraphs[c.paragraph_index];
          const insertPos = para.pos + para.nodeSize;
          chain.insertContentAt(insertPos, {
            type: 'paragraph',
            attrs: { class: 'commentary-paragraph' },
            content: [{ type: 'text', text: `【批】${c.commentary}` }],
          });
        }
        chain.run();
      } catch (error) {
        rtEditorLogger.error('Commentary error', { error });
      } finally {
        setIsGeneratingCommentary(false);
      }
    }, [editor, storyId]);

    // 创建文本批注
    const handleAcceptChange = async (changeId: string) => {
      try {
        await acceptChangeMutation.mutateAsync({ changeId, chapterId });
        toast.success('已接受变更');
      } catch (error) {
        rtEditorLogger.error('Failed to accept change', { error });
        toast.error('操作失败');
      }
    };

    const handleRejectChange = async (changeId: string) => {
      try {
        await rejectChangeMutation.mutateAsync({ changeId, chapterId });
        toast.success('已拒绝变更');
      } catch (error) {
        rtEditorLogger.error('Failed to reject change', { error });
        toast.error('操作失败');
      }
    };

    // 辅助函数
    const findFirstDiff = (a: string, b: string): number => {
      let i = 0;
      while (i < a.length && i < b.length && a[i] === b[i]) i++;
      return i;
    };

    const textOffsetToPmPosition = (editorInstance: any, offset: number, length: number) => {
      let currentOffset = 0;
      let startPos = -1;
      let endPos = -1;
      editorInstance.state.doc.descendants((node: any, pos: number) => {
        if (!node.isText) return;
        const text = node.text || '';
        const nodeStart = currentOffset;
        const nodeEnd = currentOffset + text.length;
        if (startPos === -1 && nodeEnd > offset) {
          startPos = pos + (offset - nodeStart);
        }
        if (endPos === -1 && nodeEnd >= offset + length) {
          endPos = pos + (offset + length - nodeStart);
        }
        currentOffset += text.length;
      });
      if (startPos !== -1 && endPos !== -1) {
        return { from: startPos, to: endPos };
      }
      return null;
    };

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
    useImperativeHandle(ref, () => ({
      insertText: (text: string) => {
        if (editor) {
          if (selectedRange) {
            editor.chain().focus().setTextSelection({ from: selectedRange.from, to: selectedRange.to }).insertContent(text).run();
          } else {
            editor.chain().focus().insertContent(text).run();
          }
        }
      },
      getText: () => editor?.getText() || '',
      getSelectedText: () => {
        if (!editor) return '';
        const { from, to } = editor.state.selection;
        if (from === to) return '';
        return editor.state.doc.textBetween(from, to, '\n');
      },
      focus: () => editor?.commands.focus(),
      generateCommentary: () => {
        handleGenerateCommentary();
      },
      setContent: (text: string) => {
        if (editor) {
          editor.commands.setContent(`<p>${text.replace(/\n/g, '</p><p>')}</p>`);
        }
      },
      loadAggregatedScenes: (scenes) => {
        if (!editor) return;
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
      },
    }), [editor, handleGenerateCommentary, selectedRange]);

    // AI 生成时自动滚动到编辑器底部，让幽灵文本和 Tab/Esc 提示可见
    useEffect(() => {
      if (generatedText || isGenerating) {
        requestAnimationFrame(() => {
          const scrollContainer = containerRef.current?.querySelector('.overflow-auto') as HTMLElement | null;
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
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setContextMenu({ visible: true, x: e.clientX, y: e.clientY });
        }}
      >
        {/* 编辑器内容区 */}
        <div className="flex-1 overflow-auto relative min-h-0">
          {/* 修订模式横幅 — 精简为单行 */}
          {isRevisionMode && (
            <div className="revision-banner">
              <div className="revision-banner-main">
                <div className="flex items-center gap-2 text-sm text-blue-400">
                  <GitBranch className="w-4 h-4" />
                  <span>修订模式</span>
                  <span className="text-xs text-blue-500/70">({pendingChanges.length} 处待审)</span>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => chapterId && acceptAllMutation.mutate({ chapterId })}
                    disabled={acceptAllMutation.isPending || pendingChanges.length === 0}
                    className="revision-banner-btn revision-banner-btn-accept"
                  >
                    <CheckCheck className="w-3.5 h-3.5" />
                    全部接受
                  </button>
                  <button
                    onClick={() => chapterId && rejectAllMutation.mutate({ chapterId })}
                    disabled={rejectAllMutation.isPending || pendingChanges.length === 0}
                    className="revision-banner-btn revision-banner-btn-reject"
                  >
                    <Undo2 className="w-3.5 h-3.5" />
                    全部拒绝
                  </button>
                  <button
                    onClick={() => setIsRevisionMode(false)}
                    className="revision-banner-btn"
                  >
                    退出
                  </button>
                </div>
              </div>
              {/* 变更列表折叠展开 */}
              {pendingChanges.length > 0 && (
                <div className="revision-changes-list">
                  {pendingChanges.map((change) => (
                    <div key={change.id} className="revision-change-item">
                      <div className="flex items-center gap-2 min-w-0">
                        <span className={cn(
                          'revision-change-badge',
                          change.change_type === 'Insert' && 'revision-badge-insert',
                          change.change_type === 'Delete' && 'revision-badge-delete',
                          change.change_type === 'Format' && 'revision-badge-format'
                        )}>
                          {change.change_type === 'Insert' ? '插入' : change.change_type === 'Delete' ? '删除' : '排版'}
                        </span>
                        <span className="text-gray-300 truncate">
                          {change.content || '（无内容）'}
                        </span>
                      </div>
                      <div className="flex items-center gap-1 shrink-0 ml-2">
                        <button
                          onClick={() => handleAcceptChange(change.id)}
                          disabled={acceptChangeMutation.isPending}
                          className="revision-change-action revision-change-accept"
                        >
                          <Check className="w-3 h-3" />
                          接受
                        </button>
                        <button
                          onClick={() => handleRejectChange(change.id)}
                          disabled={rejectChangeMutation.isPending}
                          className="revision-change-action revision-change-reject"
                        >
                          <X className="w-3 h-3" />
                          拒绝
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          <EditorContent editor={editor} />

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
                onChange={(e) => setSlashInputText(e.target.value)}
                onKeyDown={(e) => {
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
          {(generatedText || isGenerating) && (
            <div className="editor-ghost-continuation">
              {generatedText && (
                <p className="ghost-paragraph">{generatedText}</p>
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
          isRevisionMode={isRevisionMode}
          onToggleRevision={() => setIsRevisionMode(!isRevisionMode)}
          onGenerateCommentary={() => {
            handleGenerateCommentary();
            setContextMenu({ visible: false, x: 0, y: 0 });
          }}
          isGeneratingCommentary={isGeneratingCommentary}
          hasSelection={!!selectedRange}
        />

        {/* 角色卡片弹窗 */}
        <CharacterCardPopup
          character={selectedCharacter || { id: '', story_id: '', name: '', created_at: '', updated_at: '' }}
          position={popupPosition}
          visible={showPopup}
          onClose={() => setShowPopup(false)}
          anchorEl={popupAnchor}
        />

        {/* 角色 hover 微型卡片 */}
        <CharacterPeekCard
          character={peekCharacter}
          position={peekPosition}
          visible={showPeek}
        />
      </div>
    );
  }
);

RichTextEditor.displayName = 'RichTextEditor';

export default RichTextEditor;
