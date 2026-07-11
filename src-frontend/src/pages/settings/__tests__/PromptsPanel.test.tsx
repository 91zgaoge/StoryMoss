import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { PromptsPanel } from '../PromptsPanel';

const loggedInvoke = vi.fn();

vi.mock('@/services/api/core', () => ({
  loggedInvoke: (...args: [string, Record<string, unknown>?]) => loggedInvoke(...args),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  writeFile: vi.fn(),
  readFile: vi.fn(),
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
  default_content: 'default content body',
  current_content: 'current content body for editing',
  is_overridden: false,
  variables: ['story_title'],
};

const mockComposition = {
  scene: 'timesliced',
  scene_label: 'TimeSliced 续写',
  layers: [
    {
      role: 'system',
      prompt_id: 'writer_system',
      name: '写作助手',
      source: 'system_prompt',
    },
  ],
};

describe('PromptsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loggedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_prompt_entries') {
        return [mockEntry];
      }
      if (cmd === 'get_prompts_directory') {
        return '/Users/yuzaimu/projects/StoryMoss/resources/prompts';
      }
      if (cmd === 'open_prompts_directory') {
        return '/Users/yuzaimu/projects/StoryMoss/resources/prompts';
      }
      if (cmd === 'preview_prompt_composition') {
        return mockComposition;
      }
      return undefined;
    });
  });

  it('加载并展示提示词列表', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('核心写作提示词')).toBeInTheDocument();
    });

    expect(loggedInvoke).toHaveBeenCalledWith('list_prompt_entries');
  });

  it('展开提示词后立即显示正文编辑器，不出现 Loading', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('核心写作提示词')).toBeInTheDocument();
    });

    const row = document.querySelector('[data-prompt-id="writer_system"] button');
    expect(row).toBeTruthy();
    fireEvent.click(row!);

    await waitFor(() => {
      const editor = screen.getByTestId('prompt-editor');
      expect(editor).toBeInTheDocument();
      expect(editor).toHaveValue('current content body for editing');
    });

    expect(screen.queryByText('Loading...')).not.toBeInTheDocument();
  });

  it('导入提示词覆盖时应使用 snake_case 参数 prompt_id', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('核心写作提示词')).toBeInTheDocument();
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

  it('点击打开目录按钮应调用 open_prompts_directory', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('核心写作提示词')).toBeInTheDocument();
    });

    const openDirButton = screen.getByRole('button', { name: /打开目录/i });
    fireEvent.click(openDirButton);

    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('open_prompts_directory');
    });
  });

  it('场景组合预览应加载并展示分层', async () => {
    render(<PromptsPanel />);

    await waitFor(() => {
      expect(screen.getByText('场景组合预览')).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('preview_prompt_composition', {
        scene: 'timesliced',
      });
    });

    expect(screen.getByTestId('composition-scene-select')).toHaveValue('timesliced');
  });
});
