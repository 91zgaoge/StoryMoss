import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import React from 'react';
import { render } from '@testing-library/react';
import type { RichTextEditorRef } from '../RichTextEditor';

let capturedOptions: Record<string, unknown> | null = null;
let fakeHTML = '<p>initial</p>';

function createFakeEditor() {
  const chainable = {
    focus: () => chainable,
    insertContent: () => chainable,
    insertContentAt: () => chainable,
    setTextSelection: () => chainable,
    run: () => true,
  };
  return {
    getHTML: () => fakeHTML,
    getText: () => fakeHTML.replace(/<[^>]+>/g, ''),
    isFocused: false,
    isEmpty: false,
    commands: {
      setContent: vi.fn(),
      insertContent: vi.fn(),
    },
    chain: () => chainable,
    on: vi.fn(),
    off: vi.fn(),
    state: {
      selection: { from: 0, to: 0 },
      doc: {
        content: { size: 0 },
        textBetween: () => '',
      },
    },
  };
}

let fakeEditor = createFakeEditor();

vi.mock('@tiptap/react', () => ({
  useEditor: (options: Record<string, unknown>) => {
    capturedOptions = options;
    return fakeEditor;
  },
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
vi.mock('@/frontstage/extensions/SceneDividerNode', () => ({ SceneDividerNode: {} }));

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
}));

// 必须在 mock 之后动态导入被测组件，确保 mock 生效
let RichTextEditor: typeof import('../RichTextEditor').default;

describe('RichTextEditor HTML serialization debounce', () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    capturedOptions = null;
    fakeHTML = '<p>initial</p>';
    fakeEditor = createFakeEditor();
    const mod = await import('../RichTextEditor');
    RichTextEditor = mod.default;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('debounces onChange callback by 200ms after editor updates', async () => {
    const onChange = vi.fn();
    const ref = React.createRef<RichTextEditorRef>();

    render(<RichTextEditor ref={ref} content="<p>initial</p>" onChange={onChange} />);

    expect(capturedOptions).not.toBeNull();
    const onUpdate = capturedOptions!.onUpdate as ({
      editor,
    }: {
      editor: typeof fakeEditor;
    }) => void;

    fakeHTML = '<p>updated once</p>';
    onUpdate({ editor: fakeEditor });

    // 200ms 内不应触发 onChange
    await vi.advanceTimersByTimeAsync(100);
    expect(onChange).not.toHaveBeenCalled();

    // 200ms 后最终触发
    await vi.advanceTimersByTimeAsync(150);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenLastCalledWith('<p>updated once</p>');

    // 连续快速更新应只保留最后一次
    fakeHTML = '<p>first rapid</p>';
    onUpdate({ editor: fakeEditor });
    await vi.advanceTimersByTimeAsync(100);
    fakeHTML = '<p>second rapid</p>';
    onUpdate({ editor: fakeEditor });
    await vi.advanceTimersByTimeAsync(100);
    fakeHTML = '<p>final rapid</p>';
    onUpdate({ editor: fakeEditor });

    await vi.advanceTimersByTimeAsync(250);
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenLastCalledWith('<p>final rapid</p>');
  });
});
