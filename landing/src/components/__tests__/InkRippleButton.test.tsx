import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { InkRippleButton } from '../InkRippleButton';

describe('InkRippleButton', () => {
  it('renders children', () => {
    render(<InkRippleButton>下载</InkRippleButton>);
    expect(screen.getByRole('button', { name: '下载' })).toBeInTheDocument();
  });

  it('calls onClick when clicked', () => {
    const handleClick = vi.fn();
    render(<InkRippleButton onClick={handleClick}>下载</InkRippleButton>);
    fireEvent.click(screen.getByRole('button', { name: '下载' }));
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('renders primary variant by default', () => {
    render(<InkRippleButton>下载</InkRippleButton>);
    const button = screen.getByRole('button', { name: '下载' });
    expect(button.className).toContain('bg-terracotta');
  });

  it('renders secondary variant', () => {
    render(<InkRippleButton variant="secondary">了解更多</InkRippleButton>);
    const button = screen.getByRole('button', { name: '了解更多' });
    expect(button.className).toContain('border-terracotta');
  });
});
