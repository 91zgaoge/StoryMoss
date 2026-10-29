import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import FrontstageHeader from '../FrontstageHeader';

const cssPath = resolve(dirname(fileURLToPath(import.meta.url)), '../../styles/frontstage.css');

function cssRuleBlocks(css: string): { selectors: string; body: string }[] {
  return [...css.matchAll(/([^{]+)\{([^}]*)\}/g)].map(([, selectors, body]) => ({
    selectors,
    body,
  }));
}

vi.mock('../IngestHealthIndicator', () => ({
  IngestHealthIndicator: () => null,
}));

vi.mock('../DebtIndicator', () => ({
  default: () => null,
}));

// v0.30.17: useAgencyAgentActivity 通过动态 import('@tauri-apps/api/event') 订阅事件。
// 捕获 listen 回调以便在测试中模拟 Agency agent 活动 / run 结束事件。
const { listeners } = vi.hoisted(() => ({
  listeners: {} as Record<string, (e: { payload: unknown }) => void>,
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    listeners[event] = cb;
    return Promise.resolve(() => {});
  }),
}));

describe('FrontstageHeader', () => {
  const defaultProps = {
    currentStory: { id: '1', title: '测试故事' },
    displayTitle: '测试故事',
    canRename: true,
    currentChapter: { id: 'c1', story_id: '1', title: '第一章', chapter_number: 1 },
    wordCount: 1234,
    totalWordCount: 5678,
    fontSize: 18,
    isSaved: true,
    isZenMode: false,
    wensiMode: 'passive' as const,
    orchestratorStatus: null,
    bootstrapProgress: null,
    dbPoolStatus: null,
    onOpenBackstage: vi.fn(),
    onOpenFontSettings: vi.fn(),
    onCycleWensiMode: vi.fn(),
    onToggleZenMode: vi.fn(),
    onRenameStory: vi.fn().mockResolvedValue(undefined),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应该显示 displayTitle', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByText('测试故事')).toBeInTheDocument();
  });

  it('默认状态下应该显示"草苔"', () => {
    render(
      <FrontstageHeader
        {...defaultProps}
        currentStory={null}
        displayTitle="草苔"
        canRename={false}
      />
    );
    expect(screen.getByText('草苔')).toBeInTheDocument();
  });

  it('应该显示章节标题和字数统计', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByText('第1章')).toBeInTheDocument();
    expect(screen.getByText(/1234 字/)).toBeInTheDocument();
    expect(screen.getByText(/5678 字/)).toBeInTheDocument();
  });

  it('字数使用 tabular-nums', () => {
    render(<FrontstageHeader {...defaultProps} />);
    const el = screen.getByTitle('当前章节字数 / 全文字数');
    expect(el.className).toMatch(/tabular-nums/);
  });

  it('应该显示字体大小', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByText('18px')).toBeInTheDocument();
  });

  it('点击字体大小应该触发打开字体设置', async () => {
    const onOpenFontSettings = vi.fn();
    render(<FrontstageHeader {...defaultProps} onOpenFontSettings={onOpenFontSettings} />);

    await userEvent.click(screen.getByText('18px'));
    expect(onOpenFontSettings).toHaveBeenCalledTimes(1);
  });

  it('未保存时应该显示"保存中..."提示', () => {
    render(<FrontstageHeader {...defaultProps} isSaved={false} />);
    expect(screen.getByText('保存中...')).toBeInTheDocument();
  });

  it('应该显示禅模式按钮和文思模式按钮', () => {
    render(<FrontstageHeader {...defaultProps} />);
    expect(screen.getByTitle('进入全屏禅写模式（F11）')).toBeInTheDocument();
    expect(screen.getByTitle(/文思/)).toBeInTheDocument();
  });

  it('禅模式下文思/禅写按钮隐藏，但设置按钮仍可回幕后', () => {
    render(<FrontstageHeader {...defaultProps} isZenMode={true} />);
    expect(screen.queryByTitle('进入全屏禅写模式（F11）')).not.toBeInTheDocument();
    expect(screen.queryByTitle(/文思/)).not.toBeInTheDocument();
    expect(screen.getByLabelText('打开设置 / 幕后工作室')).toBeInTheDocument();
  });

  it('单击故事名称不应打开幕后（避免与双击改名冲突）', async () => {
    const onOpenBackstage = vi.fn();
    render(<FrontstageHeader {...defaultProps} onOpenBackstage={onOpenBackstage} />);

    await userEvent.click(screen.getByText('测试故事'));
    expect(onOpenBackstage).not.toHaveBeenCalled();
  });

  it('设置按钮应打开幕后工作室', async () => {
    const onOpenBackstage = vi.fn();
    render(<FrontstageHeader {...defaultProps} onOpenBackstage={onOpenBackstage} />);

    await userEvent.click(screen.getByLabelText('打开设置 / 幕后工作室'));
    expect(onOpenBackstage).toHaveBeenCalledTimes(1);
  });

  it('双击故事名称应该进入编辑态且不打开幕后', async () => {
    const user = userEvent.setup();
    const onOpenBackstage = vi.fn();
    render(<FrontstageHeader {...defaultProps} onOpenBackstage={onOpenBackstage} />);

    await user.dblClick(screen.getByText('测试故事'));
    expect(screen.getByLabelText('编辑故事名称')).toBeInTheDocument();
    expect(screen.getByDisplayValue('测试故事')).toBeInTheDocument();
    expect(onOpenBackstage).not.toHaveBeenCalled();
  });

  it('改名后失焦应调用 onRenameStory', async () => {
    const user = userEvent.setup();
    const onRenameStory = vi.fn().mockResolvedValue(undefined);
    render(<FrontstageHeader {...defaultProps} onRenameStory={onRenameStory} />);

    await user.dblClick(screen.getByText('测试故事'));
    const input = screen.getByLabelText('编辑故事名称');
    await user.clear(input);
    await user.type(input, '新书名');
    await user.tab();

    await waitFor(() => {
      expect(onRenameStory).toHaveBeenCalledWith('新书名');
    });
  });

  it('清空标题失焦不应调用 onRenameStory', async () => {
    const user = userEvent.setup();
    const onRenameStory = vi.fn().mockResolvedValue(undefined);
    render(<FrontstageHeader {...defaultProps} onRenameStory={onRenameStory} />);

    await user.dblClick(screen.getByText('测试故事'));
    const input = screen.getByLabelText('编辑故事名称');
    await user.clear(input);
    await user.tab();

    await waitFor(() => {
      expect(screen.queryByLabelText('编辑故事名称')).not.toBeInTheDocument();
    });
    expect(onRenameStory).not.toHaveBeenCalled();
  });

  it('点击禅模式按钮应该触发回调', async () => {
    const onToggleZenMode = vi.fn();
    render(<FrontstageHeader {...defaultProps} onToggleZenMode={onToggleZenMode} />);

    await userEvent.click(screen.getByTitle('进入全屏禅写模式（F11）'));
    expect(onToggleZenMode).toHaveBeenCalledTimes(1);
  });

  it('点击文思模式按钮应该触发回调', async () => {
    const onCycleWensiMode = vi.fn();
    render(<FrontstageHeader {...defaultProps} onCycleWensiMode={onCycleWensiMode} />);

    await userEvent.click(screen.getByTitle(/文思/));
    expect(onCycleWensiMode).toHaveBeenCalledTimes(1);
  });

  it('文思活跃模式应该显示正确的提示', () => {
    render(<FrontstageHeader {...defaultProps} wensiMode="active" />);
    expect(screen.getByTitle('文思活跃：按 Ctrl+Enter 触发 AI 续写')).toBeInTheDocument();
  });

  it('文思关闭模式应该显示正确的提示', () => {
    render(<FrontstageHeader {...defaultProps} wensiMode="off" />);
    expect(screen.getByTitle('文思已关闭')).toBeInTheDocument();
  });

  it('文思活跃使用动态火焰图标，被动保持静态图标', () => {
    const { rerender } = render(<FrontstageHeader {...defaultProps} wensiMode="active" />);
    expect(document.querySelector('.wensi-flame')).not.toBeNull();

    rerender(<FrontstageHeader {...defaultProps} wensiMode="passive" />);
    expect(document.querySelector('.wensi-flame')).toBeNull();
  });

  it('活跃火焰有跳动动画，且尊重减少动态偏好', () => {
    const css = readFileSync(cssPath, 'utf-8');
    expect(css).toMatch(
      /\.wensi-active\s+\.wensi-flame\s*\{[^}]*animation:\s*wensi-flame-flicker/s
    );
    expect(css).toMatch(/@keyframes\s+wensi-flame-flicker/);

    const reduced = css.slice(css.indexOf('prefers-reduced-motion: reduce'));
    expect(reduced).toMatch(/\.wensi-active\s+\.wensi-flame\s*\{[^}]*animation:\s*none/s);
  });

  it('创世进行中应显示三 Agent（主创/管理/编辑审计）的动作与进度', async () => {
    render(<FrontstageHeader {...defaultProps} />);
    await waitFor(() => {
      expect(listeners['agency-agent-activity']).toBeTruthy();
    });
    act(() => {
      listeners['agency-agent-activity']({
        payload: { run_id: 'r1', role: 'lead_writer', action: 'start', detail: '首章' },
      });
      listeners['agency-agent-activity']({
        payload: { run_id: 'r1', role: 'producer', action: 'done', detail: '深度资产' },
      });
      listeners['agency-agent-activity']({
        payload: { run_id: 'r1', role: 'editor_auditor', action: 'start', detail: '审查' },
      });
    });
    expect(screen.getByText('主创正在写第一章')).toBeInTheDocument();
    expect(screen.getByText('管理已完成深度资产')).toBeInTheDocument();
    expect(screen.getByText('编辑审计正在质检')).toBeInTheDocument();
  });

  it('创世 run 结束后应清空三 Agent 进度', async () => {
    render(<FrontstageHeader {...defaultProps} />);
    await waitFor(() => {
      expect(listeners['agency-agent-activity']).toBeTruthy();
      expect(listeners['agency-run-progress']).toBeTruthy();
    });
    act(() => {
      listeners['agency-agent-activity']({
        payload: { run_id: 'r1', role: 'lead_writer', action: 'start', detail: '首章' },
      });
    });
    expect(screen.getByText('主创正在写第一章')).toBeInTheDocument();
    act(() => {
      listeners['agency-run-progress']({ payload: { run_id: 'r1', status: 'completed' } });
    });
    expect(screen.queryByText('主创正在写第一章')).not.toBeInTheDocument();
  });

  const chaptersProp = [
    { id: 'c1', story_id: '1', title: '第一章', chapter_number: 1 },
    { id: 'c2', story_id: '1', title: '', chapter_number: 2 },
    { id: 'c3', story_id: '1', title: '临江夜雨', chapter_number: 3 },
  ];

  it('单击章节名应展开章节下拉并触发 onOpenChapterList', async () => {
    const onOpenChapterList = vi.fn();
    render(
      <FrontstageHeader
        {...defaultProps}
        chapters={chaptersProp}
        onOpenChapterList={onOpenChapterList}
      />
    );

    await userEvent.click(screen.getByText('第1章'));
    await waitFor(() => {
      expect(screen.getByRole('listbox', { name: '章节列表' })).toBeInTheDocument();
    });
    expect(onOpenChapterList).toHaveBeenCalledTimes(1);
    // 空标题章节按「第N章」展示
    expect(screen.getByRole('option', { name: '第2章' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '临江夜雨' })).toBeInTheDocument();
    // 当前章节高亮
    expect(screen.getByRole('option', { name: '第1章' })).toHaveAttribute('aria-selected', 'true');
  });

  it('下拉中选中其他章节应调用 onSelectChapter 并关闭菜单', async () => {
    const user = userEvent.setup();
    const onSelectChapter = vi.fn();
    render(
      <FrontstageHeader
        {...defaultProps}
        chapters={chaptersProp}
        onSelectChapter={onSelectChapter}
      />
    );

    await user.click(screen.getByLabelText('展开章节列表'));
    await user.click(screen.getByRole('option', { name: '临江夜雨' }));

    expect(onSelectChapter).toHaveBeenCalledTimes(1);
    expect(onSelectChapter).toHaveBeenCalledWith(chaptersProp[2]);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('选中当前章节不应触发 onSelectChapter', async () => {
    const user = userEvent.setup();
    const onSelectChapter = vi.fn();
    render(
      <FrontstageHeader
        {...defaultProps}
        chapters={chaptersProp}
        onSelectChapter={onSelectChapter}
      />
    );

    await user.click(screen.getByLabelText('展开章节列表'));
    await user.click(screen.getByRole('option', { name: '第1章' }));
    expect(onSelectChapter).not.toHaveBeenCalled();
  });

  it('双击章节名改名不应展开下拉', async () => {
    const user = userEvent.setup();
    render(
      <FrontstageHeader
        {...defaultProps}
        chapters={chaptersProp}
        canRenameChapter
        onRenameChapter={vi.fn()}
      />
    );

    await user.dblClick(screen.getByText('第1章'));
    expect(screen.getByLabelText('编辑章节名称')).toBeInTheDocument();
    // 等待超过单击延迟（250ms），确认下拉未弹出
    await new Promise(resolve => setTimeout(resolve, 350));
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('Escape 应关闭章节下拉', async () => {
    const user = userEvent.setup();
    render(<FrontstageHeader {...defaultProps} chapters={chaptersProp} />);

    await user.click(screen.getByLabelText('展开章节列表'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('设置/禅/文思按钮走 press 曲线，无 hover scale 1.1', () => {
    const css = readFileSync(cssPath, 'utf-8');
    const headerBtn = /\.settings-btn|\.zen-mode-btn|\.wensi-mode-toggle/;
    const rules = cssRuleBlocks(css);

    for (const sel of ['.settings-btn', '.zen-mode-btn', '.wensi-mode-toggle']) {
      const block =
        css.match(new RegExp(sel.replace('.', '\\.') + '\\s*\\{[^}]+\\}', 's'))?.[0] ?? '';
      expect(block, sel).toMatch(/--transition-press/);
      expect(block, sel).toMatch(/border-radius:\s*6px/);
      expect(block, sel).toMatch(/color:\s*var\(--stone-gray\)/);
    }

    for (const { selectors, body } of rules) {
      if (headerBtn.test(selectors + body)) {
        expect(body, selectors).not.toMatch(/scale\(1\.1\)/);
      }
    }

    expect(
      rules.some(
        r =>
          headerBtn.test(r.selectors) &&
          /color-mix\(\s*in oklch,\s*var\(--terracotta\)\s*18%,\s*transparent\s*\)/.test(r.body)
      )
    ).toBe(true);

    for (const sel of [
      '.settings-btn:active',
      '.zen-mode-btn:active',
      '.wensi-mode-toggle:active',
    ]) {
      expect(
        rules.some(r => r.selectors.includes(sel) && /scale\(0\.98\)/.test(r.body)),
        sel
      ).toBe(true);
    }

    const reduced = rules.find(
      r =>
        /prefers-reduced-motion:\s*reduce/.test(r.selectors + r.body) &&
        /transform:\s*none/.test(r.body) &&
        /\.settings-btn:active/.test(r.selectors + r.body) &&
        /\.zen-mode-btn:active/.test(r.selectors + r.body) &&
        /\.wensi-mode-toggle:active/.test(r.selectors + r.body)
    );
    expect(reduced).toBeTruthy();
  });
});
