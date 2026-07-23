import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FrontstageBottomBar from '../FrontstageBottomBar';
import { useBackendActivityStore } from '@/stores/backendActivityStore';

describe('FrontstageBottomBar', () => {
  beforeEach(() => {
    useBackendActivityStore.setState({ activities: [] });
  });

  const defaultProps = {
    isZenMode: false,
    isGenerating: false,
    isGenesis: false,
    generationStatus: '',
    inputValue: '',
    ghostHint: '',
    hintSource: 'llm' as const,
    gatewayModels: [
      {
        model_id: 'gpt-4',
        model_name: 'GPT-4',
        status: 'healthy' as const,
        enabled: true,
        is_primary: true,
      },
    ],
    allModels: [
      {
        id: 'gpt-4',
        name: 'GPT-4',
        provider: 'openai' as const,
        model: 'gpt-4',
        type: 'chat' as const,
        temperature: 0.7,
        max_tokens: 4096,
        enabled: true,
        capabilities: ['chat' as const],
      },
    ],
    onInputChange: vi.fn(),
    onInputSubmit: vi.fn(),
    onCancelGeneration: vi.fn(),
    onInputFocus: vi.fn(),
    onInputKeyDown: vi.fn(),
  };

  it('应该渲染输入框', () => {
    render(<FrontstageBottomBar {...defaultProps} />);
    expect(screen.getByPlaceholderText('输入任意指令…')).toBeInTheDocument();
  });

  it('非生成状态下应该显示发送按钮', () => {
    render(<FrontstageBottomBar {...defaultProps} />);
    expect(screen.getByTitle('发送')).toBeInTheDocument();
    expect(screen.queryByTitle('取消生成')).not.toBeInTheDocument();
  });

  it('生成状态下应该显示取消按钮', () => {
    render(
      <FrontstageBottomBar {...defaultProps} isGenerating={true} generationStatus="正在生成…" />
    );
    expect(screen.getByTitle('取消生成')).toBeInTheDocument();
    expect(screen.queryByTitle('发送')).not.toBeInTheDocument();
  });

  it('生成状态下应该显示生成状态行', () => {
    render(
      <FrontstageBottomBar {...defaultProps} isGenerating={true} generationStatus="正在思考…" />
    );
    expect(screen.getByTitle('正在思考…')).toBeInTheDocument();
  });

  it('应该显示 ghost hint 当输入为空时', () => {
    render(<FrontstageBottomBar {...defaultProps} ghostHint="试试输入情节建议" />);
    expect(screen.getByText('试试输入情节建议')).toBeInTheDocument();
  });

  it('输入框应该有正确的 placeholder 当没有 ghost hint', () => {
    render(<FrontstageBottomBar {...defaultProps} />);
    expect(screen.getByPlaceholderText('输入任意指令…')).toBeInTheDocument();
  });

  it('输入时应该触发 onInputChange', async () => {
    const onInputChange = vi.fn();
    render(<FrontstageBottomBar {...defaultProps} onInputChange={onInputChange} />);

    const textarea = screen.getByPlaceholderText('输入任意指令…');
    await userEvent.type(textarea, '测试输入');
    // 受控组件在测试中逐字输入会多次触发 onChange，只要验证确实被调用即可
    expect(onInputChange).toHaveBeenCalled();
    expect(onInputChange.mock.calls.length).toBeGreaterThanOrEqual(1);
  });

  it('点击发送按钮应该触发 onInputSubmit', async () => {
    const onInputSubmit = vi.fn();
    render(
      <FrontstageBottomBar {...defaultProps} inputValue="测试内容" onInputSubmit={onInputSubmit} />
    );

    await userEvent.click(screen.getByTitle('发送'));
    expect(onInputSubmit).toHaveBeenCalledTimes(1);
  });

  it('点击取消按钮应该触发 onCancelGeneration', async () => {
    const onCancelGeneration = vi.fn();
    render(
      <FrontstageBottomBar
        {...defaultProps}
        isGenerating={true}
        onCancelGeneration={onCancelGeneration}
      />
    );

    await userEvent.click(screen.getByTitle('取消生成'));
    expect(onCancelGeneration).toHaveBeenCalledTimes(1);
  });

  it('输入框获得焦点时应该触发 onInputFocus', async () => {
    const onInputFocus = vi.fn();
    render(<FrontstageBottomBar {...defaultProps} onInputFocus={onInputFocus} />);

    const textarea = screen.getByPlaceholderText('输入任意指令…');
    await userEvent.click(textarea);
    expect(onInputFocus).toHaveBeenCalledTimes(1);
  });

  it('禅模式下应该完全隐藏', () => {
    const { container } = render(<FrontstageBottomBar {...defaultProps} isZenMode={true} />);
    expect(container.firstChild).toBeNull();
  });

  it('模型状态指示器应该存在', () => {
    render(<FrontstageBottomBar {...defaultProps} />);
    expect(document.querySelector('.model-signal-bar')).toBeInTheDocument();
  });

  it('空输入时发送按钮应该被禁用', () => {
    render(<FrontstageBottomBar {...defaultProps} inputValue="  " />);
    expect(screen.getByTitle('发送')).toBeDisabled();
  });

  it('后台活动类别图标应该渲染为 SVG 图标而非 emoji', () => {
    // 注册一个 running 状态的后台活动
    useBackendActivityStore.getState().registerActivity({
      id: 'test-activity',
      category: 'orchestrator',
      stage: 'running',
      message: '测试中',
    });

    render(<FrontstageBottomBar {...defaultProps} />);

    const iconWrapper = document.querySelector('.generation-status-category-icon');
    expect(iconWrapper).toBeInTheDocument();
    expect(iconWrapper?.querySelector('svg')).toBeInTheDocument();
    // emoji 是文本节点，修复后不应再直接出现
    expect(iconWrapper?.textContent?.trim()).toBe('');
  });

  it('状态文案含 emoji 时应用 StatusIcon 渲染 SVG，不直接显示 emoji', () => {
    // 契约：getMajorPhase 历史路径会把 📂 写入 generationStatus；
    // WebView 缺 emoji 字体会显示 □□，必须经 StatusIcon 剥离并换 Lucide。
    render(
      <FrontstageBottomBar
        {...defaultProps}
        isGenerating={true}
        generationStatus="📂 准备上下文..."
      />
    );

    const base = document.querySelector('.generation-status-base');
    expect(base).toBeInTheDocument();
    expect(base?.querySelector('svg')).toBeInTheDocument();
    expect(screen.getByText('准备上下文...')).toBeInTheDocument();
    expect(base?.textContent).not.toMatch(/📂/);
  });

  // ===== v0.30.24: Logline 幽灵提示测试 =====

  it('loglineHint 非空且输入非空时应该渲染建议条', () => {
    render(
      <FrontstageBottomBar
        {...defaultProps}
        inputValue="写一部现代间谍的长篇小说"
        loglineHint="当一个退役特工发现妻子是潜伏二十年的敌方间谍后必须阻止她引爆情报网络"
      />
    );
    expect(
      screen.getByText('当一个退役特工发现妻子是潜伏二十年的敌方间谍后必须阻止她引爆情报网络')
    ).toBeInTheDocument();
    expect(document.querySelector('.frontstage-logline-hint')).toBeInTheDocument();
  });

  it('loglineHintLoading 时应该显示加载提示', () => {
    render(
      <FrontstageBottomBar
        {...defaultProps}
        inputValue="写一部科幻小说"
        loglineHintLoading={true}
      />
    );
    expect(screen.getByText('正在生成增强版指令…')).toBeInTheDocument();
    expect(document.querySelector('.frontstage-logline-loading')).toBeInTheDocument();
  });

  it('点击建议条应该用 logline 替换输入', async () => {
    const onInputChange = vi.fn();
    const logline = '当一个退役特工发现妻子是间谍后必须阻止她引爆情报网络';
    render(
      <FrontstageBottomBar
        {...defaultProps}
        inputValue="写一部间谍小说"
        loglineHint={logline}
        onInputChange={onInputChange}
      />
    );
    const hintBar = document.querySelector('.frontstage-logline-hint') as HTMLElement;
    expect(hintBar).toBeInTheDocument();
    await userEvent.click(hintBar);
    expect(onInputChange).toHaveBeenCalledWith(logline);
  });

  it('输入为空时不应渲染 logline 建议条', () => {
    render(<FrontstageBottomBar {...defaultProps} inputValue="" loglineHint="一些 logline 内容" />);
    expect(document.querySelector('.frontstage-logline-hint')).not.toBeInTheDocument();
  });
});
