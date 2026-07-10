import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ValueProp } from '../ValueProp';

describe('ValueProp', () => {
  it('renders the value proposition', () => {
    render(<ValueProp />);
    expect(
      screen.getByText(/草苔是专为长篇小说作者设计的系统工作台/i)
    ).toBeInTheDocument();
  });
});
