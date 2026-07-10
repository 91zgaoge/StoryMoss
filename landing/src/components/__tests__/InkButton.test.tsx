import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { InkButton } from '../InkButton';

describe('InkButton', () => {
  it('renders primary button', () => {
    render(<InkButton variant="primary">下载</InkButton>);
    const button = screen.getByRole('button', { name: /下载/i });
    expect(button).toBeInTheDocument();
  });

  it('renders secondary button', () => {
    render(<InkButton variant="secondary">查看</InkButton>);
    const button = screen.getByRole('button', { name: /查看/i });
    expect(button).toBeInTheDocument();
  });

  it('forwards className', () => {
    render(
      <InkButton variant="primary" className="extra-class">
        下载
      </InkButton>
    );
    const button = screen.getByRole('button', { name: /下载/i });
    expect(button.className).toContain('extra-class');
  });
});
