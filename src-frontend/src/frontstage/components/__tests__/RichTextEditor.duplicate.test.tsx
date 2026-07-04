import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import React from 'react';
import { render, screen, act } from '@testing-library/react';
import type { RichTextEditorRef } from '../RichTextEditor';

const GENERATED_TEXT =
  '空气是粘稠的，带着一种金属锈蚀和腐败的甜腥味。\n\n凯尔的呼吸声在头盔内部被放大成粗重的喘息。';

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
    isEmpty: true,
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
vi.mock('@/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn(), warn: vi.fn(), info: vi.fn() }),
}));
vi.mock('lucide-react', () => ({
  Sparkles: () => null,
  X: () => null,
  Check: () => null,
}));

// 必须在 mock 之后动态导入被测组件，确保 mock 生效
let RichTextEditor: typeof import('../RichTextEditor').default;

describe('RichTextEditor 内容重复防护', () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    capturedOptions = null;
    fakeHTML = '<p></p>';
    fakeText = '';
    fakeEditor = createFakeEditor();
    const mod = await import('../RichTextEditor');
    RichTextEditor = mod.default;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('空编辑器应显示幽灵文本', () => {
    render(
      <RichTextEditor
        content=""
        generatedText={GENERATED_TEXT}
        isGenerating={false}
        onChange={vi.fn()}
      />
    );

    const ghost = screen.getByTestId('ghost-paragraph');
    expect(ghost).toBeInTheDocument();
    expect(ghost.textContent).toContain('空气是粘稠的');
  });

  it('Tab 接受后幽灵文本应从 DOM 中移除', async () => {
    const onAccept = vi.fn();

    const { unmount } = render(
      <RichTextEditor
        content=""
        generatedText={GENERATED_TEXT}
        isGenerating={false}
        onChange={vi.fn()}
        onAcceptGeneration={onAccept}
      />
    );

    expect(screen.getByTestId('ghost-paragraph')).toBeInTheDocument();

    // 模拟 Tab 接受：RichTextEditor 在 window 上监听 keydown
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }));
    });

    // 等待 React 更新
    await vi.advanceTimersByTimeAsync(0);

    expect(screen.queryByTestId('ghost-paragraph')).not.toBeInTheDocument();
    expect(onAccept).toHaveBeenCalled();

    unmount();
  });

  it('编辑器已包含生成内容时，再次传入相同 generatedText 不应显示幽灵文本', () => {
    // 模拟已经接受后的编辑器状态：编辑器里已有生成内容
    fakeHTML = `<p>${GENERATED_TEXT.replace(/\n\n/g, '</p><p>')}</p>`;
    fakeText = GENERATED_TEXT.replace(/\n\n/g, '');
    fakeEditor.isEmpty = false;

    render(
      <RichTextEditor
        content={fakeHTML}
        generatedText={GENERATED_TEXT}
        isGenerating={false}
        onChange={vi.fn()}
      />
    );

    expect(screen.queryByTestId('ghost-paragraph')).not.toBeInTheDocument();
  });

  it('幽灵文本是正文前缀/片段时（缺少结尾），也不应显示幽灵文本', () => {
    // 编辑器里有完整生成内容
    fakeHTML = `<p>${GENERATED_TEXT.replace(/\n\n/g, '</p><p>')}</p>`;
    fakeText = GENERATED_TEXT.replace(/\n\n/g, '');
    fakeEditor.isEmpty = false;

    // 但 generatedText 只包含前半部分（缺少结尾），模拟用户观察到的现象
    const partialGeneratedText = GENERATED_TEXT.slice(0, GENERATED_TEXT.length - 20);

    render(
      <RichTextEditor
        content={fakeHTML}
        generatedText={partialGeneratedText}
        isGenerating={false}
        onChange={vi.fn()}
      />
    );

    expect(screen.queryByTestId('ghost-paragraph')).not.toBeInTheDocument();
  });

  it('编辑器已包含生成内容时，传入不同 generatedText 仍应显示幽灵文本', () => {
    fakeHTML = `<p>${GENERATED_TEXT.replace(/\n\n/g, '</p><p>')}</p>`;
    fakeText = GENERATED_TEXT.replace(/\n\n/g, '');
    fakeEditor.isEmpty = false;

    render(
      <RichTextEditor
        content={fakeHTML}
        generatedText="这是完全不同的后续内容。"
        isGenerating={false}
        onChange={vi.fn()}
      />
    );

    expect(screen.getByTestId('ghost-paragraph')).toBeInTheDocument();
  });
});
