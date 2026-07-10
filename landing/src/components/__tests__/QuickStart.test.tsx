import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { QuickStart } from '../QuickStart';

describe('QuickStart', () => {
  it('renders three steps', () => {
    render(<QuickStart />);
    expect(screen.getByText('下载安装桌面版')).toBeInTheDocument();
    expect(screen.getByText('用 Genesis 创建故事')).toBeInTheDocument();
    expect(screen.getByText('进入幕前，写下第一段')).toBeInTheDocument();
  });
});
