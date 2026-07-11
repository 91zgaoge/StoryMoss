import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { Genesis } from '../Genesis';

describe('Genesis', () => {
  it('renders four genesis steps', () => {
    render(<Genesis />);
    expect(screen.getByText('概念解析')).toBeInTheDocument();
    expect(screen.getByText('策略选择')).toBeInTheDocument();
    expect(screen.getByText('开篇骨架')).toBeInTheDocument();
    expect(screen.getByText('生成正文')).toBeInTheDocument();
  });
});
