import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import React from 'react';
import { render, act } from '@testing-library/react';

let capturedOptions: Record<string, unknown> | null = null;
let fakeHTML = '<p></p>';
let fakeText = '';

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
    getText: () => fakeText,
    isFocused: false,
    isEmpty: fakeText.length === 0,
    isDestroyed: false,
    commands: {
      setContent: vi.fn((html: string) => {
        fakeHTML = html;
        fakeText = html.replace(/<[^>]+>/g, '');
      }),
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
vi.mock('@/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn(), warn: vi.fn(), info: vi.fn() }),
}));
vi.mock('lucide-react', () => ({
  Sparkles: () => null,
  X: () => null,
  Check: () => null,
  Scissors: () => null,
  ArrowUp: () => null,
  ChevronRight: () => null,
  RefreshCw: () => null,
}));

const OLD_CHAPTER = '<p>第二章正文：进到王府大堂，论级别，苏会山虽贵为镇北王。</p>';
const NEW_CHAPTER = '<p>第三章正文：苏福贵没有动。他仍跪在门边，一只手按着地上的门闩。</p>';

let RichTextEditor: typeof import('../RichTextEditor').default;

describe('RichTextEditor 切章正文同步（v0.65.3 焦点/幽灵锁下不得吞掉新章正文）', () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    capturedOptions = null;
    fakeHTML = OLD_CHAPTER;
    fakeText = OLD_CHAPTER.replace(/<[^>]+>/g, '');
    fakeEditor = createFakeEditor();
    fakeEditor.isFocused = true;
    const mod = await import('../RichTextEditor');
    RichTextEditor = mod.default;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('章 id 变化时必须把新章正文写进编辑器（即便编辑器仍有焦点）', async () => {
    const { rerender } = render(
      <RichTextEditor content={OLD_CHAPTER} chapterId="ch-2" onChange={vi.fn()} />
    );
    expect(fakeHTML).toBe(OLD_CHAPTER);
    expect(fakeEditor.isFocused).toBe(true);

    await act(async () => {
      rerender(<RichTextEditor content={NEW_CHAPTER} chapterId="ch-3" onChange={vi.fn()} />);
      await vi.advanceTimersByTimeAsync(0);
    });

    // 修复前：isFocused 守卫直接 return，编辑器停在旧章正文（用户看到「续文不见了」）
    expect(fakeHTML).toBe(NEW_CHAPTER);
  });

  it('章 id 变化时即便处于 Tab 接受后 30s 幽灵隐藏窗口，也必须加载新章正文', async () => {
    const hideGhostUntil = Date.now() + 30000;
    const { rerender } = render(
      <RichTextEditor
        content={OLD_CHAPTER}
        chapterId="ch-2"
        hideGhostUntil={hideGhostUntil}
        onChange={vi.fn()}
      />
    );

    await act(async () => {
      rerender(
        <RichTextEditor
          content={NEW_CHAPTER}
          chapterId="ch-3"
          hideGhostUntil={hideGhostUntil}
          onChange={vi.fn()}
        />
      );
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(fakeHTML).toBe(NEW_CHAPTER);
  });

  it('同一章内的外部同步（后台回写）仍让位于焦点守卫，不抢焦点', async () => {
    const backgroundSync = '<p>第二章正文：后台 auto_commit 回写的同一章正文。</p>';
    const { rerender } = render(
      <RichTextEditor content={OLD_CHAPTER} chapterId="ch-2" onChange={vi.fn()} />
    );

    await act(async () => {
      rerender(<RichTextEditor content={backgroundSync} chapterId="ch-2" onChange={vi.fn()} />);
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(fakeHTML).toBe(OLD_CHAPTER);
  });
});
