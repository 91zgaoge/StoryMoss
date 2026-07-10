import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { WhyStoryForge } from '../WhyStoryForge';

describe('WhyStoryForge', () => {
  it('renders three advantage cards', () => {
    render(<WhyStoryForge />);
    expect(screen.getByText('长上下文不丢约束')).toBeInTheDocument();
    expect(screen.getByText('稳定压倒灵感')).toBeInTheDocument();
    expect(screen.getByText('本地运行，数据归你')).toBeInTheDocument();
  });
});
