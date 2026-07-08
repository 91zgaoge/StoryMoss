import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CreationPathGuide } from '../CreationPathGuide';

describe('CreationPathGuide', () => {
  it('渲染三条创作路径卡片', () => {
    render(<CreationPathGuide />);
    expect(screen.getByText('幕前 Genesis Pipeline')).toBeInTheDocument();
    expect(screen.getByText('幕后 AI 向导')).toBeInTheDocument();
    expect(screen.getByText('幕后快速创作')).toBeInTheDocument();
  });

  it('点击幕前 Genesis Pipeline 卡片触发 onFrontstage', async () => {
    const onFrontstage = vi.fn();
    render(<CreationPathGuide onFrontstage={onFrontstage} onWizard={vi.fn()} onQuick={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /幕前 Genesis Pipeline/ }));
    expect(onFrontstage).toHaveBeenCalledTimes(1);
  });

  it('点击幕后 AI 向导卡片触发 onWizard', async () => {
    const onWizard = vi.fn();
    render(<CreationPathGuide onFrontstage={vi.fn()} onWizard={onWizard} onQuick={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /幕后 AI 向导/ }));
    expect(onWizard).toHaveBeenCalledTimes(1);
  });

  it('点击幕后快速创作卡片触发 onQuick', async () => {
    const onQuick = vi.fn();
    render(<CreationPathGuide onFrontstage={vi.fn()} onWizard={vi.fn()} onQuick={onQuick} />);
    await userEvent.click(screen.getByRole('button', { name: /幕后快速创作/ }));
    expect(onQuick).toHaveBeenCalledTimes(1);
  });

  it('未传入回调时卡片不可交互', () => {
    render(<CreationPathGuide />);
    expect(screen.queryByRole('button', { name: /幕前 Genesis Pipeline/ })).not.toBeInTheDocument();
  });
});
