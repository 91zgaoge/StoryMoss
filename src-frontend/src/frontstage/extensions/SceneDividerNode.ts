/**
 * SceneDividerNode - TipTap Node 扩展
 *
 * 在 Chapter 编辑器中标记 Scene 边界，支持 1:N 聚合编辑。
 * 渲染为一条带场景信息的视觉分隔线，不可直接编辑内容。
 */

import { Node, mergeAttributes } from '@tiptap/core';

export interface SceneDividerOptions {
  HTMLAttributes: Record<string, any>;
}

export interface SceneDividerAttributes {
  sceneId: string;
  sceneNumber: number;
  sceneTitle?: string;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    sceneDivider: {
      insertSceneDivider: (attrs: SceneDividerAttributes) => ReturnType;
      removeSceneDivider: (sceneId: string) => ReturnType;
      updateSceneDivider: (sceneId: string, attrs: Partial<SceneDividerAttributes>) => ReturnType;
    };
  }
}

export const SceneDividerNode = Node.create<SceneDividerOptions>({
  name: 'sceneDivider',

  group: 'block',

  // 空内容 — divider 是纯装饰性节点
  content: '',

  // 标记为原子节点，防止光标进入
  atom: true,

  // 隔离 — 不允许跨 divider 选择
  isolating: true,

  selectable: true,

  draggable: false,

  addOptions() {
    return {
      HTMLAttributes: {
        class: 'scene-divider',
      },
    };
  },

  addAttributes() {
    return {
      sceneId: {
        default: '',
        parseHTML: (element) => element.getAttribute('data-scene-id') || '',
        renderHTML: (attributes) => {
          if (!attributes.sceneId) return {};
          return { 'data-scene-id': attributes.sceneId };
        },
      },
      sceneNumber: {
        default: 1,
        parseHTML: (element) => parseInt(element.getAttribute('data-scene-number') || '1', 10),
        renderHTML: (attributes) => {
          return { 'data-scene-number': String(attributes.sceneNumber) };
        },
      },
      sceneTitle: {
        default: '',
        parseHTML: (element) => element.getAttribute('data-scene-title') || '',
        renderHTML: (attributes) => {
          if (!attributes.sceneTitle) return {};
          return { 'data-scene-title': attributes.sceneTitle };
        },
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'div[data-scene-divider]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const { sceneNumber, sceneTitle, ...rest } = HTMLAttributes;
    const label = sceneTitle
      ? `场景 ${sceneNumber}: ${sceneTitle}`
      : `场景 ${sceneNumber}`;

    return [
      'div',
      mergeAttributes(
        this.options.HTMLAttributes,
        rest,
        { 'data-scene-divider': 'true', 'data-label': label }
      ),
      ['span', { class: 'scene-divider-label' }, label],
    ];
  },

  addCommands() {
    return {
      insertSceneDivider:
        (attrs) =>
        ({ chain, state }) => {
          const { from } = state.selection;
          return chain()
            .insertContentAt(from, {
              type: 'sceneDivider',
              attrs,
            })
            .run();
        },

      removeSceneDivider:
        (sceneId) =>
        ({ state, chain }) => {
          let dividerPos = -1;
          let dividerNodeSize = 0;

          state.doc.descendants((node, pos) => {
            if (node.type.name === 'sceneDivider' && node.attrs.sceneId === sceneId) {
              dividerPos = pos;
              dividerNodeSize = node.nodeSize;
            }
          });

          if (dividerPos === -1) return false;

          return chain()
            .deleteRange({ from: dividerPos, to: dividerPos + dividerNodeSize })
            .run();
        },

      updateSceneDivider:
        (sceneId, attrs) =>
        ({ state, chain }) => {
          let dividerPos = -1;

          state.doc.descendants((node, pos) => {
            if (node.type.name === 'sceneDivider' && node.attrs.sceneId === sceneId) {
              dividerPos = pos;
            }
          });

          if (dividerPos === -1) return false;

          return chain()
            .setTextSelection(dividerPos)
            .updateAttributes('sceneDivider', { ...attrs })
            .run();
        },
    };
  },

  addKeyboardShortcuts() {
    return {
      // Divider 前后按 Enter 时，在 divider 外侧创建新段落
      Enter: () => {
        const { state, view } = this.editor;
        const { selection } = state;
        const { $from } = selection;

        // 如果光标紧接在 divider 后面
        const nodeAfter = $from.nodeAfter;
        if (nodeAfter && nodeAfter.type.name === 'sceneDivider') {
          const pos = $from.pos;
          view.dispatch(
            state.tr.insert(pos, state.schema.nodes.paragraph.create())
          );
          return true;
        }

        // 如果光标在 divider 正前面
        const nodeBefore = $from.nodeBefore;
        if (nodeBefore && nodeBefore.type.name === 'sceneDivider') {
          const pos = $from.pos;
          view.dispatch(
            state.tr.insert(pos, state.schema.nodes.paragraph.create())
          );
          return true;
        }

        return false;
      },
    };
  },
});

export default SceneDividerNode;
