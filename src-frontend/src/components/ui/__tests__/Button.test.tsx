import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Button } from '../Button';

describe('Button', () => {
  it('paper 是 primary 的兼容别名（金色调）', () => {
    render(<Button variant="paper">写</Button>);
    expect(screen.getByRole('button', { name: '写' })).toHaveClass('bg-cinema-gold/15');
  });

  it('cinema 是 primary 的兼容别名', () => {
    render(<Button variant="cinema">保存</Button>);
    expect(screen.getByRole('button', { name: '保存' })).toHaveClass('bg-cinema-gold/15');
  });

  it('cinema-outline 是 secondary 的兼容别名', () => {
    render(<Button variant="cinema-outline">取消</Button>);
    expect(screen.getByRole('button', { name: '取消' })).toHaveClass('bg-cinema-800');
  });

  it('press 用 scale-98 而非 scale-95', () => {
    render(<Button variant="primary">确定</Button>);
    const cls = screen.getByRole('button').className;
    expect(cls).toMatch(/active:scale-\[0\.98\]/);
    expect(cls).not.toMatch(/active:scale-95/);
    expect(cls).toMatch(/ease-press/);
    expect(cls).toMatch(/motion-reduce:active:scale-100/);
  });

  it('主按钮是淡彩不是实心高饱和填充', () => {
    render(<Button variant="primary">做</Button>);
    expect(screen.getByRole('button').className).not.toMatch(/bg-cinema-gold(?!\/)/);
  });

  it('不引用未定义的 cinema 灰阶（回归：cinema-100/300/50）', () => {
    const { rerender } = render(<Button variant="secondary">次</Button>);
    expect(screen.getByRole('button').className).not.toMatch(/cinema-(50|100|300)\b/);
    rerender(<Button variant="ghost">潜</Button>);
    expect(screen.getByRole('button').className).not.toMatch(/cinema-(50|100|300)\b/);
  });
});
