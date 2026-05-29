import { Mark, mergeAttributes } from '@tiptap/core';

export interface CommentAnchorOptions {
  HTMLAttributes: Record<string, any>;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    commentAnchor: {
      setCommentAnchor: (attributes: { threadId: string }) => ReturnType;
      unsetCommentAnchor: () => ReturnType;
    };
  }
}

export const CommentAnchorMark = Mark.create<CommentAnchorOptions>({
  name: 'commentAnchor',

  addOptions() {
    return {
      HTMLAttributes: {},
    };
  },

  addAttributes() {
    return {
      threadId: {
        default: null,
        parseHTML: element => element.getAttribute('data-thread-id'),
        renderHTML: attributes => ({
          'data-thread-id': attributes.threadId,
        }),
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-thread-id]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      'span',
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
        style:
          'background-color: rgba(250, 204, 21, 0.35); border-bottom: 2px solid rgba(234, 179, 8, 0.8); cursor: pointer; border-radius: 2px; transition: background-color 0.15s;',
        onmouseenter: "this.style.backgroundColor='rgba(250, 204, 21, 0.55)'",
        onmouseleave: "this.style.backgroundColor='rgba(250, 204, 21, 0.35)'",
      }),
      0,
    ];
  },

  addCommands() {
    return {
      setCommentAnchor:
        attributes =>
        ({ commands }) => {
          return commands.setMark(this.name, attributes);
        },
      unsetCommentAnchor:
        () =>
        ({ commands }) => {
          return commands.unsetMark(this.name);
        },
    };
  },
});
