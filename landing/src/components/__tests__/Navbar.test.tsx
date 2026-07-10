import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Navbar } from '../Navbar';

describe('Navbar', () => {
  it('renders brand name', () => {
    render(<Navbar />);
    expect(screen.getByText('草苔')).toBeInTheDocument();
    expect(screen.getByText('StoryForge')).toBeInTheDocument();
  });

  it('renders desktop download button', () => {
    render(<Navbar />);
    expect(screen.getByRole('button', { name: '免费下载' })).toBeInTheDocument();
  });

  it('toggles mobile menu', () => {
    render(<Navbar />);
    const toggle = screen.getByLabelText('打开菜单');
    fireEvent.click(toggle);
    expect(screen.getByLabelText('关闭菜单')).toBeInTheDocument();
    const mobileMenu = screen.getByRole('list');
    expect(mobileMenu).toBeInTheDocument();
    expect(mobileMenu.querySelector('button')).toHaveTextContent('免费下载');
  });
});
