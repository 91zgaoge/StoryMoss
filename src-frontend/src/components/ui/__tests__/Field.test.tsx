import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Field } from '../Field';
import { Input, Select, Textarea, controlClass } from '../controls';

describe('Field', () => {
  it('label/必填星标/hint 渲染', () => {
    render(
      <Field label="续写目标字数" required htmlFor="words" hint="建议范围 500-5000">
        <Input id="words" />
      </Field>
    );
    expect(screen.getByText('续写目标字数')).toBeInTheDocument();
    expect(screen.getByLabelText(/续写目标字数/)).toBeInTheDocument();
    expect(screen.getByText('建议范围 500-5000')).toBeInTheDocument();
  });

  it('error 优先于 hint 且 role=alert', () => {
    render(
      <Field label="名称" error="请输入任务名称" hint="不应显示">
        <Input />
      </Field>
    );
    expect(screen.getByRole('alert')).toHaveTextContent('请输入任务名称');
    expect(screen.queryByText('不应显示')).not.toBeInTheDocument();
  });
});

describe('controls', () => {
  it('Input/Select/Textarea 共享控件样式', () => {
    render(
      <>
        <Input placeholder="输入角色名称" data-testid="i" />
        <Select data-testid="s" />
        <Textarea data-testid="t" />
      </>
    );
    for (const id of ['i', 's', 't']) {
      expect(screen.getByTestId(id).className).toContain(controlClass);
    }
    expect(screen.getByPlaceholderText('输入角色名称')).toBeInTheDocument();
    expect(screen.getByTestId('t')).toHaveClass('resize-y');
  });
});
