/**
 * TipTap Node 扩展：AI 内联建议段落
 *
 * 在编辑器中显示为灰色/半透明的建议修改文本，紧跟在原文段落之后。
 * Tab 接受 → 替换原文段落
 * Esc 拒绝 → 删除建议段落
 * 鼠标悬停显示接受/拒绝按钮
 */

import { Node, mergeAttributes } from '@tiptap/core';
import { NodeViewWrapper } from '@tiptap/react';
import React from 'react';
import { recordFeedback } from '@/services/tauri';

export interface AiSuggestionOptions {
  HTMLAttributes: Record<string, any>;
}

export interface AiSuggestionAttributes {
  suggestionId: string;
  category: string;
  priority: string;
  originalText: string;
  targetParagraphIndex: number;
  storyId?: string;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    aiSuggestion: {
      insertAiSuggestion: (attrs: AiSuggestionAttributes, suggestedText: string) => ReturnType;
      acceptAiSuggestion: (suggestionId: string) => ReturnType;
      rejectAiSuggestion: (suggestionId: string) => ReturnType;
      clearAllAiSuggestions: () => ReturnType;
    };
  }
}

// React NodeView 组件
const AiSuggestionView: React.FC<any> = ({ node, editor }) => {
  const attrs = node.attrs as AiSuggestionAttributes;
  const category = attrs.category || 'default';
  const suggestedText = node.textContent || '';

  const handleAccept = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (attrs.suggestionId) {
      editor.commands.acceptAiSuggestion(attrs.suggestionId);
    }
  };

  const handleReject = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (attrs.suggestionId) {
      editor.commands.rejectAiSuggestion(attrs.suggestionId);
    }
  };

  const labels: Record<string, string> = {
    pacing: '节奏', dialogue: '对话', description: '描写',
    vocabulary: '词汇', sentence: '句式', emotion: '情感',
    plot: '情节', structure: '结构',
  };

  return (
    <NodeViewWrapper as="div" className={`ai-suggestion-node ai-suggestion-${category}`}>
      <div className="ai-suggestion-label">
        {labels[category] || '建议'} · Tab接受 · Esc拒绝
      </div>
      <p className="ai-suggestion-text">{suggestedText}</p>
      <div className="ai-suggestion-actions">
        <button className="ai-suggestion-action-accept" onClick={handleAccept} title="接受修改 (Tab)">
          接受
        </button>
        <button className="ai-suggestion-action-reject" onClick={handleReject} title="拒绝修改 (Esc)">
          拒绝
        </button>
      </div>
    </NodeViewWrapper>
  );
};

export const AiSuggestionNode = Node.create<AiSuggestionOptions>({
  name: 'aiSuggestion',

  group: 'block',

  content: 'text*',

  isolating: true,

  addAttributes() {
    return {
      suggestionId: {
        default: '',
        parseHTML: element => element.getAttribute('data-suggestion-id') || '',
        renderHTML: attributes => {
          if (!attributes.suggestionId) return {};
          return { 'data-suggestion-id': attributes.suggestionId };
        },
      },
      category: {
        default: '',
        parseHTML: element => element.getAttribute('data-category') || '',
        renderHTML: attributes => {
          if (!attributes.category) return {};
          return { 'data-category': attributes.category };
        },
      },
      priority: {
        default: 'medium',
        parseHTML: element => element.getAttribute('data-priority') || 'medium',
        renderHTML: attributes => {
          return { 'data-priority': attributes.priority };
        },
      },
      originalText: {
        default: '',
        parseHTML: element => element.getAttribute('data-original-text') || '',
        renderHTML: attributes => {
          if (!attributes.originalText) return {};
          return { 'data-original-text': attributes.originalText };
        },
      },
      targetParagraphIndex: {
        default: 0,
        parseHTML: element => parseInt(element.getAttribute('data-target-paragraph') || '0', 10),
        renderHTML: attributes => {
          return { 'data-target-paragraph': String(attributes.targetParagraphIndex) };
        },
      },
      storyId: {
        default: '',
        parseHTML: element => element.getAttribute('data-story-id') || '',
        renderHTML: attributes => {
          if (!attributes.storyId) return {};
          return { 'data-story-id': attributes.storyId };
        },
      },
    };
  },

  parseHTML() {
    return [{ tag: 'p[data-ai-suggestion]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['p', mergeAttributes(
      { 'data-ai-suggestion': 'true' },
      HTMLAttributes,
      { class: `ai-suggestion-paragraph ai-suggestion-${HTMLAttributes['data-category'] || 'default'}` }
    ), 0];
  },

  addNodeView() {
    return ReactNodeViewRenderer(AiSuggestionView);
  },

  addKeyboardShortcuts() {
    return {
      Tab: () => {
        const { state } = this.editor;
        const cursorPos = state.selection.from;
        let suggestionId: string | null = null;
        state.doc.descendants((node, nodePos) => {
          if (node.type.name === 'aiSuggestion' && !suggestionId && Math.abs(nodePos - cursorPos) < 200) {
            suggestionId = node.attrs.suggestionId || null;
          }
        });
        if (suggestionId) {
          this.editor.commands.acceptAiSuggestion(suggestionId);
          return true;
        }
        return false;
      },
      Escape: () => {
        const { state } = this.editor;
        const cursorPos = state.selection.from;
        let suggestionId: string | null = null;
        state.doc.descendants((node, nodePos) => {
          if (node.type.name === 'aiSuggestion' && !suggestionId && Math.abs(nodePos - cursorPos) < 200) {
            suggestionId = node.attrs.suggestionId || null;
          }
        });
        if (suggestionId) {
          this.editor.commands.rejectAiSuggestion(suggestionId);
          return true;
        }
        return false;
      },
    };
  },

  addCommands() {
    return {
      insertAiSuggestion:
        (attrs, suggestedText) =>
        ({ chain, state }) => {
          const paragraphs: { pos: number; nodeSize: number; index: number }[] = [];
          let paraIndex = 0;
          state.doc.descendants((node, pos) => {
            if (node.type.name === 'paragraph' && node.childCount > 0) {
              paragraphs.push({ pos, nodeSize: node.nodeSize, index: paraIndex++ });
            }
          });

          const targetPara = paragraphs[attrs.targetParagraphIndex];
          if (!targetPara) return false;

          const insertPos = targetPara.pos + targetPara.nodeSize;

          return chain()
            .insertContentAt(insertPos, {
              type: 'aiSuggestion',
              attrs,
              content: [{ type: 'text', text: suggestedText }],
            })
            .run();
        },

      acceptAiSuggestion:
        (suggestionId) =>
        ({ state, chain }) => {
          let suggestionPos = -1;
          let suggestionNodeSize = 0;
          let suggestedText = '';
          let nearestParaPos = -1;
          let nearestParaSize = 0;
          let storyId = '';

          state.doc.descendants((node, pos) => {
            if (node.type.name === 'aiSuggestion' && node.attrs.suggestionId === suggestionId) {
              suggestionPos = pos;
              suggestionNodeSize = node.nodeSize;
              suggestedText = node.textContent || '';
              storyId = node.attrs.storyId || '';
            }
          });

          if (suggestionPos === -1) return false;

          // 记录反馈
          if (storyId) {
            recordFeedback({
              story_id: storyId,
              feedback_type: 'accept',
              agent_type: 'inline_suggestion',
              original_ai_text: suggestedText,
            });
          }

          // 找离建议最近的前一个普通段落
          state.doc.descendants((node, pos) => {
            if (node.type.name === 'paragraph' && pos < suggestionPos) {
              const endPos = pos + node.nodeSize;
              if (endPos <= suggestionPos && endPos > nearestParaPos) {
                nearestParaPos = pos;
                nearestParaSize = node.nodeSize;
              }
            }
          });

          let c = chain();

          // 删除建议段落
          c = c.deleteRange({ from: suggestionPos, to: suggestionPos + suggestionNodeSize });

          // 替换原文段落：先删除原文，再在相同位置插入新内容
          if (nearestParaPos !== -1 && suggestedText) {
            c = c
              .deleteRange({ from: nearestParaPos, to: nearestParaPos + nearestParaSize })
              .insertContentAt(nearestParaPos, {
                type: 'paragraph',
                content: [{ type: 'text', text: suggestedText }],
              });
          }

          return c.run();
        },

      rejectAiSuggestion:
        (suggestionId) =>
        ({ state, chain }) => {
          let suggestionPos = -1;
          let suggestionNodeSize = 0;
          let suggestedText = '';
          let storyId = '';

          state.doc.descendants((node, pos) => {
            if (node.type.name === 'aiSuggestion' && node.attrs.suggestionId === suggestionId) {
              suggestionPos = pos;
              suggestionNodeSize = node.nodeSize;
              suggestedText = node.textContent || '';
              storyId = node.attrs.storyId || '';
            }
          });

          if (suggestionPos === -1) return false;

          // 记录反馈
          if (storyId) {
            recordFeedback({
              story_id: storyId,
              feedback_type: 'reject',
              agent_type: 'inline_suggestion',
              original_ai_text: suggestedText,
            });
          }

          return chain()
            .setTextSelection({ from: suggestionPos, to: suggestionPos + suggestionNodeSize })
            .deleteSelection()
            .run();
        },

      clearAllAiSuggestions:
        () =>
        ({ state, chain }) => {
          const ranges: Array<{ from: number; to: number }> = [];
          state.doc.descendants((node, pos) => {
            if (node.type.name === 'aiSuggestion') {
              ranges.push({ from: pos, to: pos + node.nodeSize });
            }
          });

          if (ranges.length === 0) return false;

          let c = chain();
          for (let i = ranges.length - 1; i >= 0; i--) {
            c = c.setTextSelection({ from: ranges[i].from, to: ranges[i].to }).deleteSelection();
          }
          return c.run();
        },
    };
  },
});

// 需要 @tiptap/react 的 ReactNodeViewRenderer
import { ReactNodeViewRenderer } from '@tiptap/react';

export default AiSuggestionNode;
