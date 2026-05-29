/**
 * CharacterNameMark - TipTap 扩展
 *
 * 识别和高亮文本中的角色名，支持点击/悬停显示角色卡片
 */

import { Mark, mergeAttributes } from '@tiptap/core';

export interface CharacterNameOptions {
  HTMLAttributes: Record<string, any>;
  characterNames: string[];
  onCharacterClick: (name: string, element: HTMLElement) => void;
  onCharacterHover: (name: string, element: HTMLElement, isEnter: boolean) => void;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    characterName: {
      /**
       * 设置角色名列表
       */
      setCharacterNames: (names: string[]) => ReturnType;
    };
  }
}

export const CharacterNameMark = Mark.create<CharacterNameOptions>({
  name: 'characterName',

  addOptions() {
    return {
      HTMLAttributes: {
        class: 'character-name-mark',
      },
      characterNames: [],
      onCharacterClick: () => {},
      onCharacterHover: () => {},
    };
  },

  addAttributes() {
    return {
      'data-character-name': {
        default: null,
        parseHTML: element => element.getAttribute('data-character-name'),
        renderHTML: attributes => {
          if (!attributes['data-character-name']) {
            return {};
          }
          return {
            'data-character-name': attributes['data-character-name'],
          };
        },
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-character-name]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(this.options.HTMLAttributes, HTMLAttributes), 0];
  },

  addCommands() {
    return {
      setCharacterNames:
        (names: string[]) =>
        ({ editor }) => {
          this.options.characterNames = names;
          return true;
        },
    };
  },

  addProseMirrorPlugins() {
    const { characterNames, onCharacterClick, onCharacterHover } = this.options;

    return [
      // 这个插件会在每次文档变化时重新扫描并高亮角色名
      // 实际实现会在 React 组件中通过 useEffect 处理
    ];
  },
});

/**
 * 在文本中查找角色名的位置
 */
export function findCharacterNamePositions(
  text: string,
  characterNames: string[]
): Array<{ start: number; end: number; name: string }> {
  const positions: Array<{ start: number; end: number; name: string }> = [];

  for (const name of characterNames) {
    if (!name || name.length < 2) continue;

    const regex = new RegExp(
      // 匹配完整的角色名，前面和后面不能是中文或英文单词字符
      `(?<![\\u4e00-\\u9fa5a-zA-Z])${escapeRegExp(name)}(?![\\u4e00-\\u9fa5a-zA-Z])`,
      'g'
    );

    let match;
    while ((match = regex.exec(text)) !== null) {
      positions.push({
        start: match.index,
        end: match.index + name.length,
        name,
      });
    }
  }

  // 按位置排序
  positions.sort((a, b) => a.start - b.start);

  // 移除重叠的匹配（优先保留较长的匹配）
  const filtered: typeof positions = [];
  let lastEnd = -1;

  for (const pos of positions) {
    if (pos.start >= lastEnd) {
      filtered.push(pos);
      lastEnd = pos.end;
    }
  }

  return filtered;
}

function escapeRegExp(string: string): string {
  return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
