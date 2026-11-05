import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Table } from '../Table';
import { EmptyState } from '../EmptyState';
import { Skeleton } from '../Skeleton';

interface Row {
  id: string;
  name: string;
  count: number;
}

const rows: Row[] = [
  { id: 'a', name: '临江夜雨', count: 12 },
  { id: 'b', name: '开端', count: 3 },
];

const baseColumns = [
  { key: 'name', header: '名称' },
  { key: 'count', header: '次数', align: 'right' as const },
];

describe('Table', () => {
  it('渲染表头与行，render 优先于字段取值', () => {
    render(
      <Table<Row>
        columns={[
          { key: 'name', header: '名称', render: r => <strong>{r.name}</strong> },
          { key: 'count', header: '次数', align: 'right' },
        ]}
        rows={rows}
        rowKey={r => r.id}
      />
    );
    expect(screen.getByText('名称')).toBeInTheDocument();
    expect(screen.getByText('次数')).toBeInTheDocument();
    expect(screen.getByText('临江夜雨').tagName).toBe('STRONG');
    expect(screen.getByText('3')).toHaveClass('tabular-nums');
  });

  it('无 render 时回退字段取值', () => {
    render(<Table<Row> columns={baseColumns} rows={rows} rowKey={r => r.id} />);
    expect(screen.getByText('临江夜雨')).toBeInTheDocument();
  });

  it('loading 显示骨架行', () => {
    render(<Table<Row> columns={baseColumns} rows={[]} rowKey={r => r.id} loading />);
    expect(
      screen.getByText('次数').closest('table')!.querySelectorAll('.animate-pulse').length
    ).toBe(3);
  });

  it('空态插槽', () => {
    render(
      <Table<Row>
        columns={baseColumns}
        rows={[]}
        rowKey={r => r.id}
        empty={<EmptyState title="还没有语料" action={{ label: '导入语料', onClick: () => {} }} />}
      />
    );
    expect(screen.getByText('还没有语料')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '导入语料' })).toBeInTheDocument();
  });

  it('maxHeight 生效且表头吸顶，行点击回调', async () => {
    const user = userEvent.setup();
    const onRowClick = vi.fn();
    render(
      <Table<Row>
        columns={baseColumns}
        rows={rows}
        rowKey={r => r.id}
        maxHeight="480px"
        onRowClick={onRowClick}
      />
    );
    const wrapper = screen.getByText('临江夜雨').closest('div')!;
    expect(wrapper).toHaveClass('overflow-y-auto');
    expect(wrapper).toHaveStyle({ maxHeight: '480px' });
    expect(screen.getByText('名称').closest('thead')).toHaveClass('sticky');
    await user.click(screen.getByText('开端'));
    expect(onRowClick).toHaveBeenCalledWith(rows[1]);
  });
});

describe('Skeleton / EmptyState', () => {
  it('Skeleton 无障碍隐藏且带脉冲', () => {
    const { container } = render(<Skeleton className="h-4 w-10" />);
    const el = container.firstElementChild!;
    expect(el).toHaveAttribute('aria-hidden', 'true');
    expect(el).toHaveClass('animate-pulse');
  });

  it('EmptyState 默认图标，action 触发', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(<EmptyState title="暂无任务" action={{ label: '新建任务', onClick }} />);
    await user.click(screen.getByRole('button', { name: '新建任务' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
