import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StatusIcon } from '../StatusIcon';

describe('StatusIcon', () => {
  it('准备上下文应渲染 Lucide SVG，并剥离前缀 emoji', () => {
    const { container } = render(<StatusIcon text="📂 准备上下文..." />);
    expect(container.querySelector('svg')).toBeInTheDocument();
    expect(screen.getByText('准备上下文...')).toBeInTheDocument();
    // 契约：文案节点不得残留 emoji（WebView 会显示为 □□）
    expect(screen.getByText('准备上下文...').textContent).toBe('准备上下文...');
  });

  it('纯文案准备上下文也应映射到文件夹图标', () => {
    const { container } = render(<StatusIcon text="准备上下文..." />);
    expect(container.querySelector('svg')).toBeInTheDocument();
    expect(screen.getByText('准备上下文...')).toBeInTheDocument();
  });

  it('完成态不应旋转', () => {
    const { container } = render(<StatusIcon text="创作计划执行完成..." />);
    const svg = container.querySelector('svg');
    expect(svg).toBeInTheDocument();
    expect(svg?.classList.contains('animate-spin')).toBe(false);
  });
});
