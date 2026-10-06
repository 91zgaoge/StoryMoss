import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import React from 'react';
import { render, screen, act } from '@testing-library/react';

// 契约：划词不得弹出润色/扩写浮条。v0.39.0 挂上、v0.48.1 改成够长才出，
// 仍挡住手工写作；v0.49.1 整条卸掉。选区跟踪若残留，也不得渲染该 UI。
let fakeSelection = { from: 0, to: 0, empty: true };
let fakeText = '';
let selectionUpdateHandler: (() => void) | null = null;

function createFakeEditor() {
  const chainable = {
    focus: () => chainable,
    insertContent: () => chainable,
    insertContentAt: () => chainable,
    setTextSelection: () => chainable,
    run: () => true,
  };
  return {
    getHTML: () => '<p>initial</p>',
    getText: () => 'initial',
    isFocused: false,
    isEmpty: false,
    commands: {
      setContent: vi.fn(),
      insertContent: vi.fn(),
      insertContentAt: vi.fn(),
      setTextSelection: vi.fn(),
    },
    chain: () => chainable,
    on: (event: string, cb: () => void) => {
      if (event === 'selectionUpdate') selectionUpdateHandler = cb;
    },
    off: vi.fn(),
    state: {
      get selection() {
        return fakeSelection;
      },
      doc: {
        content: { size: 1000 },
        textBetween: (_from: number, _to: number, _sep?: string) => fakeText,
      },
    },
  };
}

let fakeEditor = createFakeEditor();

vi.mock('@tiptap/react', () => ({
  useEditor: () => fakeEditor,
  EditorContent: function MockEditorContent() {
    return <div data-testid="editor-content" />;
  },
}));

vi.mock('@tiptap/starter-kit', () => ({
  default: { configure: () => ({ name: 'starter-kit' }) },
}));
vi.mock('@tiptap/extension-placeholder', () => ({
  default: { configure: () => ({ name: 'placeholder' }) },
}));
vi.mock('@tiptap/extension-underline', () => ({
  default: { configure: () => ({ name: 'underline' }) },
}));
vi.mock('@tiptap/extension-highlight', () => ({
  default: { configure: () => ({ name: 'highlight' }) },
}));

vi.mock('../tiptap/AiSuggestionNode', () => ({ AiSuggestionNode: {} }));

vi.mock('@/utils/cn', () => ({
  cn: (...classes: (string | false | undefined)[]) => classes.filter(Boolean).join(' '),
}));
vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: { editorConfig: unknown }) => unknown) =>
    selector({ editorConfig: null }),
}));
vi.mock('@/services/tauri', () => ({
  getCharacterByName: vi.fn(),
  smartExecute: vi.fn(),
  formatText: vi.fn(),
}));
vi.mock('./CharacterCardPopup', () => ({ CharacterCardPopup: () => null }));
vi.mock('./CharacterPeekCard', () => ({ CharacterPeekCard: () => null }));
vi.mock('./EditorContextMenu', () => ({ EditorContextMenu: () => null }));
vi.mock('@/frontstage/config/writingStyles', () => ({ defaultStyle: {} }));
vi.mock('@/frontstage/config/colorThemes', () => ({ getCurrentEditorColors: () => ({}) }));
vi.mock('@/hooks/useSubscription', () => ({ useSubscription: () => ({ isPro: false }) }));
vi.mock('@/utils/logger', () => ({ createLogger: () => ({ error: vi.fn() }) }));
vi.mock('lucide-react', () => ({
  Sparkles: () => null,
  X: () => null,
  Check: () => null,
  Scissors: () => null,
}));

let RichTextEditor: typeof import('../RichTextEditor').default;

function fireSelection(from: number, to: number, text: string) {
  fakeSelection = { from, to, empty: from === to };
  fakeText = text;
  act(() => {
    selectionUpdateHandler?.();
  });
}

describe('RichTextEditor 不弹出划词浮条', () => {
  beforeEach(async () => {
    fakeSelection = { from: 0, to: 0, empty: true };
    fakeText = '';
    selectionUpdateHandler = null;
    fakeEditor = createFakeEditor();
    const mod = await import('../RichTextEditor');
    RichTextEditor = mod.default;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('划选长句也不出现润色/扩写/指令条', () => {
    render(<RichTextEditor content="<p>initial</p>" onChange={() => {}} />);
    fireSelection(1, 12, '被选中的一段文字');
    expect(screen.queryByTestId('ai-selection-actions')).toBeNull();
    expect(screen.queryByRole('button', { name: /润色/ })).toBeNull();
    expect(screen.queryByRole('button', { name: /扩写/ })).toBeNull();
    expect(screen.queryByRole('button', { name: /指令/ })).toBeNull();
  });
});
