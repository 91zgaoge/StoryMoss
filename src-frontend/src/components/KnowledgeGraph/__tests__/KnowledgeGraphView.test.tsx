import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import KnowledgeGraphView from '../KnowledgeGraphView';
import type { Entity, Relation } from '@/types/v3';

// Mock reactflow so the test can run in jsdom without a real canvas/WebGL.
vi.mock('reactflow', () => {
  const React = require('react');
  const ReactFlow = ({ nodes, children }: { nodes: any[]; children?: React.ReactNode }) => (
    <div data-testid="reactflow">
      {nodes.map((n: any) => (
        <div key={n.id} data-testid="kg-node">
          {n.id}
        </div>
      ))}
      {children}
    </div>
  );
  return {
    __esModule: true,
    default: ReactFlow,
    ReactFlow,
    ReactFlowProvider: ({ children }: { children: React.ReactNode }) => children,
    Background: () => null,
    Controls: () => null,
    MiniMap: () => null,
    Panel: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    MarkerType: { ArrowClosed: 'arrowclosed' },
    useNodesState: (initial: any) => {
      const [nodes, setNodes] = React.useState(initial);
      return [nodes, setNodes, () => {}];
    },
    useEdgesState: (initial: any) => [initial, () => {}, () => {}],
    useReactFlow: () => ({ fitView: vi.fn() }),
    useViewport: () => ({ x: 0, y: 0, zoom: 1 }),
    useStore: (selector: (s: { width: number; height: number }) => any, _shallow?: any) =>
      selector({ width: 1000, height: 800 }),
  };
});

function generateEntities(count: number): Entity[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `entity-${i}`,
    story_id: 'story-1',
    name: `角色 ${i}`,
    entity_type: 'Character',
    attributes: {},
    first_seen: new Date().toISOString(),
    last_updated: new Date().toISOString(),
    access_count: 0,
    is_archived: false,
  }));
}

const emptyRelations: Relation[] = [];

describe('KnowledgeGraphView LOD', () => {
  it('默认只渲染阈值内节点，点击“显示全部”后恢复全部', async () => {
    const entities = generateEntities(250);
    render(<KnowledgeGraphView entities={entities} relations={emptyRelations} />);

    const nodes = await screen.findAllByTestId('kg-node');
    expect(nodes.length).toBe(200);

    const showAllBtn = screen.getByText(/显示全部/);
    await userEvent.click(showAllBtn);

    await waitFor(() => {
      expect(screen.getAllByTestId('kg-node').length).toBe(250);
    });
  });

  it('节点数未超过阈值时不显示 LOD 折叠按钮', () => {
    const entities = generateEntities(50);
    render(<KnowledgeGraphView entities={entities} relations={emptyRelations} />);

    expect(screen.getAllByTestId('kg-node').length).toBe(50);
    expect(screen.queryByText(/显示全部/)).not.toBeInTheDocument();
  });
});
