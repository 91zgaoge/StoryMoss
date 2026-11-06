import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render, screen, fireEvent } from '@testing-library/react';
import { Panel } from '../Panel';

describe('Panel', () => {
  it('renders title and children', () => {
    render(<Panel title="Settings">content</Panel>);
    expect(screen.getByText('Settings')).toBeInTheDocument();
    expect(screen.getByText('content')).toBeInTheDocument();
  });

  it('collapses on click', () => {
    render(
      <Panel title="Advanced" collapsible>
        secret
      </Panel>
    );
    const content = screen.getByText('secret').parentElement;
    fireEvent.click(screen.getByText('Advanced'));
    expect(content).toHaveClass('max-h-0');
    expect(content).toHaveClass('opacity-0');
  });

  it('starts collapsed when defaultOpen is false', () => {
    render(
      <Panel title="Hidden" collapsible defaultOpen={false}>
        secret
      </Panel>
    );
    const content = screen.getByText('secret').parentElement;
    expect(content).toHaveClass('max-h-0');
    expect(content).toHaveClass('opacity-0');
  });

  it('keeps static content fully visible without a height cap', () => {
    render(
      <Panel title="Static" collapsible={false}>
        secret
      </Panel>
    );
    const content = screen.getByText('secret').parentElement;
    expect(content).toHaveClass('opacity-100');
    expect(content).not.toHaveClass('max-h-[1000px]');
    expect(content).not.toHaveClass('overflow-hidden');

    fireEvent.click(screen.getByText('Static'));
    expect(content).not.toHaveClass('max-h-0');
    expect(content).toHaveClass('opacity-100');
  });

  it('re-opens after being collapsed', () => {
    render(
      <Panel title="Toggle" collapsible>
        secret
      </Panel>
    );
    const content = screen.getByText('secret').parentElement;
    const header = screen.getByText('Toggle');

    fireEvent.click(header);
    expect(content).toHaveClass('max-h-0');
    expect(content).toHaveClass('opacity-0');

    fireEvent.click(header);
    expect(content).toHaveClass('max-h-none');
    expect(content).not.toHaveClass('max-h-[1000px]');
    expect(content).toHaveClass('opacity-100');
  });

  it('exposes accessibility attributes on the collapsible header', () => {
    render(
      <Panel title="Accessible" collapsible>
        secret
      </Panel>
    );
    const button = screen.getByRole('button', { name: 'Accessible' });
    expect(button).toHaveAttribute('aria-expanded', 'true');
    const content = screen.getByText('secret').parentElement;
    expect(button).toHaveAttribute('aria-controls', content?.id);

    fireEvent.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'false');
  });

  it('外壳 bezel + 内芯半径级差', () => {
    const { container } = render(<Panel title="设定">内容</Panel>);
    const shell = container.firstChild as HTMLElement;
    expect(shell.className).toMatch(/p-1/);
    expect(shell.className).toMatch(/rounded-panel/);
    expect(container.querySelector('.bg-cinema-850')).not.toBeNull();
  });

  it('内芯有 inset 顶边高光，外壳不加第二圈 ring', () => {
    const src = readFileSync(
      resolve(dirname(fileURLToPath(import.meta.url)), '../Panel.tsx'),
      'utf-8'
    );
    const shellOpen = src.indexOf('<div className="rounded-panel');
    expect(shellOpen).toBeGreaterThanOrEqual(0);
    const shellClass = src.slice(shellOpen, src.indexOf('>', shellOpen));
    expect(src).toMatch(/inset 0 1px 0/);
    expect(shellClass).not.toMatch(/ring-1|ring-2|0 0 0 1px/);
    expect(src).toMatch(/duration-500/);
  });
});
