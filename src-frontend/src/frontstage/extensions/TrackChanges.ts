import { Mark, mergeAttributes } from '@tiptap/core';

export interface TrackChangeOptions {
  HTMLAttributes: Record<string, any>;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    trackChange: {
      setTrackInsert: (attributes: { changeId: string; authorId?: string }) => ReturnType;
      setTrackDelete: (attributes: {
        changeId: string;
        authorId?: string;
        originalText?: string;
      }) => ReturnType;
      unsetTrackChange: () => ReturnType;
    };
  }
}

export const TrackInsertMark = Mark.create<TrackChangeOptions>({
  name: 'trackInsert',

  addOptions() {
    return {
      HTMLAttributes: {},
    };
  },

  addAttributes() {
    return {
      changeId: {
        default: null,
        parseHTML: element => element.getAttribute('data-change-id'),
        renderHTML: attributes => ({
          'data-change-id': attributes.changeId,
        }),
      },
      authorId: {
        default: 'user',
        parseHTML: element => element.getAttribute('data-author-id'),
        renderHTML: attributes => ({
          'data-author-id': attributes.authorId,
        }),
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-change-id][data-track="insert"]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      'span',
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
        'data-track': 'insert',
        style:
          'background-color: rgba(59, 130, 246, 0.18); border-bottom: 2px solid rgba(59, 130, 246, 0.7); cursor: pointer; border-radius: 2px;',
      }),
      0,
    ];
  },

  addCommands() {
    return {
      setTrackInsert:
        attributes =>
        ({ commands }) => {
          return commands.setMark(this.name, attributes);
        },
      unsetTrackChange:
        () =>
        ({ commands }) => {
          return commands.unsetMark(this.name);
        },
    };
  },
});

export const TrackDeleteMark = Mark.create<TrackChangeOptions>({
  name: 'trackDelete',

  addOptions() {
    return {
      HTMLAttributes: {},
    };
  },

  addAttributes() {
    return {
      changeId: {
        default: null,
        parseHTML: element => element.getAttribute('data-change-id'),
        renderHTML: attributes => ({
          'data-change-id': attributes.changeId,
        }),
      },
      authorId: {
        default: 'user',
        parseHTML: element => element.getAttribute('data-author-id'),
        renderHTML: attributes => ({
          'data-author-id': attributes.authorId,
        }),
      },
      originalText: {
        default: '',
        parseHTML: element => element.getAttribute('data-original-text'),
        renderHTML: attributes => ({
          'data-original-text': attributes.originalText,
        }),
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-change-id][data-track="delete"]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      'span',
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
        'data-track': 'delete',
        style:
          'background-color: rgba(239, 68, 68, 0.12); text-decoration: line-through; color: rgba(239, 68, 68, 0.8); cursor: pointer; border-radius: 2px;',
      }),
      0,
    ];
  },

  addCommands() {
    return {
      setTrackDelete:
        attributes =>
        ({ commands }) => {
          return commands.setMark(this.name, attributes);
        },
      unsetTrackChange:
        () =>
        ({ commands }) => {
          return commands.unsetMark(this.name);
        },
    };
  },
});
