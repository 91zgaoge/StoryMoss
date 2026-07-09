import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { PromptsPanel } from '../PromptsPanel';

const loggedInvoke = vi.fn();
const openShell = vi.fn();

vi.mock('@/services/api/core', () => ({
  loggedInvoke: (...args: [string, Record<string, unknown>?]) => loggedInvoke(...args),
}));

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: (path: string) => openShell(path),
}));

vi.mock('@monaco-editor/react', () => ({
  default: ({ value, onChange }: { value?: string; onChange?: (v?: string) => void }) => (
    <textarea
      data-testid="monaco-editor"
      value={value || ''}
      onChange={e => onChange?.(e.target.value)}
      readOnly={!onChange}
    />
  ),
}));

vi.mock('@/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn(), debug: vi.fn(), info: vi.fn(), warn: vi.fn() }),
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

const mockEntry = {
  id: 'writer_system',
  name: '写作助手',
  description: '核心写作提示词',
  category: 'Writer',
  default_content: 'default',
  current_content: 'current',
  is_overridden: false,
  variables: ['story_title'],
};

describe('PromptsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loggedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_prompt_entries') {
        return [mockEntry];
      }
      if (cmd === 'get_prompts_directory') {
        return '/Users/yuzaimu/projects/StoryForge/resources/prompts';
      }
      return undefined;
    });
  });

  it('加载并展示提示词列表', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('写作助手')).toBeInTheDocument();
    });

    expect(loggedInvoke).toHaveBeenCalledWith('list_prompt_entries');
  });

  it('导入提示词覆盖时应使用 snake_case 参数 prompt_id', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('写作助手')).toBeInTheDocument();
    });

    const file = new File(
      [JSON.stringify([{ prompt_id: 'writer_system', content: 'imported content' }])],
      'prompts.json',
      { type: 'application/json' }
    );

    const importInput = screen.getByTestId('prompt-import-input');
    fireEvent.change(importInput, { target: { files: [file] } });

    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('save_prompt_override', {
        prompt_id: 'writer_system',
        content: 'imported content',
      });
    });
  });

  it('点击打开目录按钮应调用 get_prompts_directory 并用 shell.open 打开路径', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('写作助手')).toBeInTheDocument();
    });

    const openDirButton = screen.getByRole('button', { name: /打开目录/i });
    fireEvent.click(openDirButton);

    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('get_prompts_directory');
      expect(openShell).toHaveBeenCalledWith(
        '/Users/yuzaimu/projects/StoryForge/resources/prompts'
      );
    });
  });
});
