import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ModelCard } from '../ModelCard';
import type { ChatModelConfig } from '@/types/llm';

vi.mock('@/services/settings', () => ({
  getModelProviders: () => [
    { id: 'openai', name: 'OpenAI', requiresApiKey: true, supports: ['chat'] },
  ],
}));

const baseModel: ChatModelConfig = {
  id: 'm1',
  name: 'Test Model',
  provider: 'openai',
  model: 'gpt-4',
  type: 'chat',
  enabled: true,
  temperature: 0.7,
  max_tokens: 2000,
  capabilities: ['chat'],
};

describe('ModelCard enable toggle', () => {
  it('shows 开启 when enabled and calls onToggleEnabled', () => {
    const onToggle = vi.fn();
    render(
      <ModelCard
        model={baseModel}
        onEdit={vi.fn()}
        onSetActive={vi.fn()}
        onToggleEnabled={onToggle}
      />
    );
    const toggle = screen.getByRole('switch', { name: '关闭模型' });
    expect(toggle).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText('开启')).toBeInTheDocument();
    fireEvent.click(toggle);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('shows 关闭 when disabled and hides 设为当前', () => {
    render(
      <ModelCard
        model={{ ...baseModel, enabled: false }}
        onEdit={vi.fn()}
        onSetActive={vi.fn()}
        onToggleEnabled={vi.fn()}
      />
    );
    expect(screen.getByRole('switch', { name: '开启模型' })).toHaveAttribute(
      'aria-checked',
      'false'
    );
    expect(screen.getByText('关闭')).toBeInTheDocument();
    expect(screen.queryByText('设为当前')).not.toBeInTheDocument();
    expect(screen.getByText('禁用')).toBeInTheDocument();
  });
});
