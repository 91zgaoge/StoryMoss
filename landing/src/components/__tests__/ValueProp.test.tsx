import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ValueProp } from '../ValueProp';

describe('ValueProp', () => {
  it('renders the value proposition', () => {
    render(<ValueProp />);
    expect(
      screen.getByText(/草苔不是聊天式 AI，而是一套把「灵感 → 规划 → 写作 → 审校」串起来的长篇小说创作系统/i)
    ).toBeInTheDocument();
  });
});
