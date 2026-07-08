import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import KnowledgeGraphView from '../KnowledgeGraphView';
import type { Entity, Relation } from '@/types/v3';

// Mock reactflow so the test can run in jsdom without a real canvas/WebGL.
vi.mock('reactflow', () => {
  const React = require('react');
  const ReactFlow = ({
    nodes,
    children,
    onNodeClick,
  }: {
    nodes: any[];
    children?: React.ReactNode;
    onNodeClick?: (_: any, node: any) => void;
  }) => (
    <div data-testid="reactflow">
      {nodes.map((n: any) => (
        <div key={n.id} data-testid="kg-node" onClick={() => onNodeClick?.(null, n)}>
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

vi.mock('@/services/api/genesis', () => ({
  archiveEntity: vi.fn(() => Promise.resolve({ id: 'entity-0', name: '角色 0' })),
  deleteRelation: vi.fn(() => Promise.resolve()),
}));

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

const mockEntity: Entity = {
  id: 'entity-0',
  story_id: 'story-1',
  name: '主角',
  entity_type: 'Character',
  attributes: {},
  first_seen: new Date().toISOString(),
  last_updated: new Date().toISOString(),
  access_count: 0,
  is_archived: false,
};

const mockRelation: Relation = {
  id: 'relation-1',
  story_id: 'story-1',
  source_id: 'entity-0',
  target_id: 'entity-1',
  relation_type: 'Friend',
  strength: 0.8,
  evidence: [],
  first_seen: new Date().toISOString(),
};

beforeEach(() => {
  vi.stubGlobal(
    'confirm',
    vi.fn(() => true)
  );
});

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

describe('KnowledgeGraphView delete actions', () => {
  it('renders entity archive button and calls onEntityDelete when confirmed', async () => {
    const onEntityDelete = vi.fn();
    const entities = [mockEntity, { ...mockEntity, id: 'entity-1', name: '配角' }];
    render(
      <KnowledgeGraphView
        entities={entities}
        relations={[mockRelation]}
        storyId="story-1"
        onEntityDelete={onEntityDelete}
      />
    );

    await userEvent.click(screen.getAllByTestId('kg-node')[0]);

    const archiveBtn = screen.getByTitle('归档');
    expect(archiveBtn).toBeInTheDocument();

    await userEvent.click(archiveBtn);

    await waitFor(() => {
      expect(onEntityDelete).toHaveBeenCalledWith(expect.objectContaining({ id: 'entity-0' }));
    });
  });

  it('renders relation delete button and calls onRelationDelete when confirmed', async () => {
    const onRelationDelete = vi.fn();
    const entities = [mockEntity, { ...mockEntity, id: 'entity-1', name: '配角' }];
    render(
      <KnowledgeGraphView
        entities={entities}
        relations={[mockRelation]}
        storyId="story-1"
        onRelationDelete={onRelationDelete}
      />
    );

    await userEvent.click(screen.getAllByTestId('kg-node')[0]);

    const relationDeleteBtn = screen.getByTitle('删除关系');
    expect(relationDeleteBtn).toBeInTheDocument();

    await userEvent.click(relationDeleteBtn);

    await waitFor(() => {
      expect(onRelationDelete).toHaveBeenCalledWith(expect.objectContaining({ id: 'relation-1' }));
    });
  });

  it('does not call onEntityDelete when user cancels confirmation', async () => {
    (window.confirm as ReturnType<typeof vi.fn>).mockReturnValue(false);
    const onEntityDelete = vi.fn();
    render(
      <KnowledgeGraphView
        entities={[mockEntity]}
        relations={[]}
        storyId="story-1"
        onEntityDelete={onEntityDelete}
      />
    );

    await userEvent.click(screen.getByTestId('kg-node'));
    await userEvent.click(screen.getByTitle('归档'));

    expect(onEntityDelete).not.toHaveBeenCalled();
  });
});
