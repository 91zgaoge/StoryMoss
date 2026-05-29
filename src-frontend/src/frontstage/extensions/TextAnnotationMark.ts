import { Mark, mergeAttributes } from '@tiptap/core';

export interface TextAnnotationMarkOptions {
  HTMLAttributes: Record<string, any>;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    textAnnotation: {
      setTextAnnotation: (attributes: { type: string; annotationId: string }) => ReturnType;
      unsetTextAnnotation: () => ReturnType;
    };
  }
}

export const TextAnnotationMark = Mark.create<TextAnnotationMarkOptions>({
  name: 'textAnnotation',

  addOptions() {
    return {
      HTMLAttributes: {},
    };
  },

  addAttributes() {
    return {
      type: {
        default: 'note',
        parseHTML: element => element.getAttribute('data-annotation-type'),
        renderHTML: attributes => ({
          'data-annotation-type': attributes.type,
        }),
      },
      annotationId: {
        default: null,
        parseHTML: element => element.getAttribute('data-annotation-id'),
        renderHTML: attributes => ({
          'data-annotation-id': attributes.annotationId,
        }),
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-annotation-id]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const type = HTMLAttributes['data-annotation-type'] || 'note';
    const colorMap: Record<string, string> = {
      note: 'rgba(59, 130, 246, 0.25)',
      todo: 'rgba(249, 115, 22, 0.25)',
      warning: 'rgba(239, 68, 68, 0.25)',
      idea: 'rgba(168, 85, 247, 0.25)',
    };
    const borderColorMap: Record<string, string> = {
      note: 'rgba(59, 130, 246, 0.6)',
      todo: 'rgba(249, 115, 22, 0.6)',
      warning: 'rgba(239, 68, 68, 0.6)',
      idea: 'rgba(168, 85, 247, 0.6)',
    };

    return [
      'span',
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
        style: `background-color: ${colorMap[type] || colorMap.note}; border-bottom: 2px solid ${borderColorMap[type] || borderColorMap.note}; cursor: pointer; border-radius: 2px;`,
      }),
      0,
    ];
  },

  addCommands() {
    return {
      setTextAnnotation:
        attributes =>
        ({ commands }) => {
          return commands.setMark(this.name, attributes);
        },
      unsetTextAnnotation:
        () =>
        ({ commands }) => {
          return commands.unsetMark(this.name);
        },
    };
  },
});
