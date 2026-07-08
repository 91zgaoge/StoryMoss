import React, { useCallback, useEffect, useMemo, useState } from 'react';
import ReactFlow, {
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  useReactFlow,
  ReactFlowProvider,
  Node,
  Edge,
  MarkerType,
  Panel,
  useViewport,
  useStore,
} from 'reactflow';
import 'reactflow/dist/style.css';
import type { Entity, Relation, EntityType } from '@/types/v3';
import { createLogger } from '@/utils/logger';
import { cn } from '@/utils/cn';

const kgViewLogger = createLogger('ui:KnowledgeGraphView');
import { Search, X, Filter, Pencil, Plus, Trash2, Check, RotateCcw, Star } from 'lucide-react';
import toast from 'react-hot-toast';

interface KnowledgeGraphViewProps {
  entities: Entity[];
  relations: Relation[];
  onNodeClick?: (entity: Entity) => void;
  onEntityUpdate?: (entity: Entity) => void;
  onEntityCreate?: (entity: Entity) => void;
  onRelationCreate?: (relation: Relation) => void;
  storyId?: string;
  className?: string;
}

const ENTITY_COLORS: Record<EntityType, string> = {
  Character: '#c96442', // Terracotta
  Location: '#5b8c5a', // Sage green
  Item: '#d4af37', // Cinema gold
  Organization: '#6b5b95', // Purple
  Concept: '#4a90a4', // Teal
  Event: '#c75b39', // Rust
  PlotDevice: '#8b4513', // Saddle brown
};

const ENTITY_LABELS: Record<EntityType, string> = {
  Character: '角色',
  Location: '地点',
  Item: '物品',
  Organization: '组织',
  Concept: '概念',
  Event: '事件',
  PlotDevice: ' plot装置',
};

// C1: 图谱虚拟化/LOD 阈值
const NODE_LOD_THRESHOLD = 200;
const MINIMAP_THRESHOLD = 200;
const VIEWPORT_BUFFER_PX = 200;

// C1: 轻量浅比较，替代 reactflow 未导出的 zustand shallow
function shallow<T extends Record<string, unknown>>(a: T, b: T): boolean {
  if (a === b) return true;
  const keysA = Object.keys(a);
  const keysB = Object.keys(b);
  if (keysA.length !== keysB.length) return false;
  return keysA.every(key => a[key] === b[key]);
}

function calculateLayout(entities: Entity[], relations: Relation[]) {
  const nodeMap = new Map<string, Node>();
  const typeCounts: Record<string, number> = {};

  // Group by type
  entities.forEach(entity => {
    typeCounts[entity.entity_type] = (typeCounts[entity.entity_type] || 0) + 1;
  });

  const centerX = 400;
  const centerY = 300;
  const radiusBase = 180;

  // Arrange in concentric circles by type
  const typeOrder: EntityType[] = [
    'Character',
    'Location',
    'Organization',
    'Event',
    'Concept',
    'Item',
  ];

  typeOrder.forEach((type, typeIndex) => {
    const count = typeCounts[type] || 0;
    if (count === 0) return;

    const radius = radiusBase + typeIndex * 120;
    const angleStep = (2 * Math.PI) / Math.max(count, 1);
    let currentAngle = typeIndex * 0.3; // Offset each ring

    entities
      .filter(e => e.entity_type === type)
      .forEach(entity => {
        const x = centerX + radius * Math.cos(currentAngle);
        const y = centerY + radius * Math.sin(currentAngle);
        currentAngle += angleStep;

        nodeMap.set(entity.id, {
          id: entity.id,
          position: { x, y },
          data: { entity },
          type: 'default',
          style: {
            background: ENTITY_COLORS[type],
            color: '#fff',
            border: '2px solid rgba(255,255,255,0.2)',
            borderRadius: '8px',
            padding: '8px 12px',
            fontSize: '13px',
            fontWeight: 500,
            minWidth: 80,
            textAlign: 'center',
            boxShadow: '0 2px 8px rgba(0,0,0,0.3)',
            opacity: 1,
          },
        });
      });
  });

  const edges: Edge[] = relations.map(relation => ({
    id: relation.id,
    source: relation.source_id,
    target: relation.target_id,
    label: relation.relation_type,
    type: 'smoothstep',
    animated: relation.strength > 0.7,
    style: {
      stroke: `rgba(212, 175, 55, ${0.3 + relation.strength * 0.7})`,
      strokeWidth: 1 + relation.strength * 3,
    },
    labelStyle: {
      fill: '#a0a0a0',
      fontSize: 11,
      fontWeight: 400,
    },
    labelBgStyle: {
      fill: '#1a1a1a',
      fillOpacity: 0.8,
    },
    labelBgPadding: [4, 4],
    labelShowBg: true,
    markerEnd: {
      type: MarkerType.ArrowClosed,
      color: `rgba(212, 175, 55, ${0.4 + relation.strength * 0.6})`,
    },
  }));

  return { nodes: Array.from(nodeMap.values()), edges };
}

// C1: 计算节点重要性（关系数 + 类型权重），用于 LOD 分层
function computeEntityImportance(entities: Entity[], relations: Relation[]): Map<string, number> {
  const relationCounts = new Map<string, number>();
  relations.forEach(r => {
    relationCounts.set(r.source_id, (relationCounts.get(r.source_id) || 0) + 1);
    relationCounts.set(r.target_id, (relationCounts.get(r.target_id) || 0) + 1);
  });

  const typePriority: Record<EntityType, number> = {
    Character: 100,
    Event: 80,
    Organization: 60,
    Location: 40,
    Concept: 30,
    Item: 20,
    PlotDevice: 50,
  };

  const scores = new Map<string, number>();
  entities.forEach(e => {
    const count = relationCounts.get(e.id) || 0;
    scores.set(e.id, count * 10 + (typePriority[e.entity_type] || 0));
  });
  return scores;
}

const KnowledgeGraphViewInner: React.FC<KnowledgeGraphViewProps> = ({
  entities,
  relations,
  onNodeClick,
  onEntityUpdate,
  onEntityCreate,
  onRelationCreate,
  storyId,
  className,
}) => {
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);
  const [selectedEntity, setSelectedEntity] = useState<Entity | null>(null);
  const [fitViewFlag, setFitViewFlag] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [visibleTypes, setVisibleTypes] = useState<Set<EntityType>>(
    () => new Set(Object.keys(ENTITY_COLORS) as EntityType[])
  );
  const [showFilters, setShowFilters] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [editName, setEditName] = useState('');
  const [editAttributes, setEditAttributes] = useState<[string, string][]>([]);
  const [isSaving, setIsSaving] = useState(false);
  // C1: LOD 分层显示开关
  const [showAllNodes, setShowAllNodes] = useState(false);
  const { fitView } = useReactFlow();
  // C1: 获取当前 viewport 用于视口外裁剪
  const { x, y, zoom } = useViewport();
  const { width, height } = useStore(s => ({ width: s.width, height: s.height }), shallow);

  const filteredEntities = useMemo(() => {
    const query = searchQuery.toLowerCase().trim();
    return entities.filter(
      e => visibleTypes.has(e.entity_type) && (!query || e.name.toLowerCase().includes(query))
    );
  }, [entities, searchQuery, visibleTypes]);

  // C1: LOD 分层——超过阈值时默认只显示高重要性节点
  const lodEntities = useMemo(() => {
    if (showAllNodes || filteredEntities.length <= NODE_LOD_THRESHOLD) {
      return filteredEntities;
    }
    const importance = computeEntityImportance(filteredEntities, relations);
    const sorted = [...filteredEntities].sort((a, b) => {
      const scoreA = importance.get(a.id) || 0;
      const scoreB = importance.get(b.id) || 0;
      return scoreB - scoreA;
    });
    return sorted.slice(0, NODE_LOD_THRESHOLD);
  }, [filteredEntities, relations, showAllNodes]);

  const filteredRelations = useMemo(() => {
    const visibleIds = new Set(lodEntities.map(e => e.id));
    return relations.filter(r => visibleIds.has(r.source_id) && visibleIds.has(r.target_id));
  }, [lodEntities, relations]);

  const layout = useMemo(
    () => calculateLayout(lodEntities, filteredRelations),
    [lodEntities, filteredRelations]
  );

  // C1: 视口外裁剪（带缓冲区），在 onlyRenderVisibleElements 之上再做一层预过滤
  const visibleNodes = useMemo(() => {
    if (!width || !height || zoom <= 0) return layout.nodes;
    const buffer = VIEWPORT_BUFFER_PX / zoom;
    const minX = -x / zoom - buffer;
    const maxX = (width - x) / zoom + buffer;
    const minY = -y / zoom - buffer;
    const maxY = (height - y) / zoom + buffer;

    return layout.nodes.filter(n => {
      const px = n.position.x;
      const py = n.position.y;
      return px >= minX && px <= maxX && py >= minY && py <= maxY;
    });
  }, [layout.nodes, x, y, zoom, width, height]);

  const visibleEdges = useMemo(() => {
    const visibleIds = new Set(visibleNodes.map(n => n.id));
    return layout.edges.filter(e => visibleIds.has(e.source) && visibleIds.has(e.target));
  }, [layout.edges, visibleNodes]);

  useEffect(() => {
    setNodes(visibleNodes);
    setEdges(visibleEdges);
  }, [visibleNodes, visibleEdges, setNodes, setEdges]);

  const handleNodeClick = useCallback(
    (_: React.MouseEvent, node: Node) => {
      const entity = entities.find(e => e.id === node.id);
      if (entity) {
        setSelectedEntity(entity);
        setIsEditing(false);
        onNodeClick?.(entity);
      }
    },
    [entities, onNodeClick]
  );

  const handleNodeDoubleClick = useCallback(
    (_: React.MouseEvent, node: Node) => {
      fitView({ nodes: [node], duration: 800, padding: 0.3 });
    },
    [fitView]
  );

  const toggleType = useCallback((type: EntityType) => {
    setVisibleTypes(prev => {
      const next = new Set(prev);
      if (next.has(type)) {
        next.delete(type);
      } else {
        next.add(type);
      }
      return next;
    });
  }, []);

  const clearSearch = useCallback(() => {
    setSearchQuery('');
  }, []);

  const startEditing = useCallback(() => {
    if (!selectedEntity) return;
    setEditName(selectedEntity.name);
    const attrs = Object.entries(selectedEntity.attributes || {}).map(([k, v]) => [
      k,
      typeof v === 'string' ? v : JSON.stringify(v),
    ]) as [string, string][];
    setEditAttributes(attrs);
    setIsEditing(true);
  }, [selectedEntity]);

  const cancelEditing = useCallback(() => {
    setIsEditing(false);
  }, []);

  const saveEditing = useCallback(async () => {
    if (!selectedEntity) return;
    setIsSaving(true);
    try {
      const { updateEntity } = await import('@/services/tauri');
      const attributes: Record<string, unknown> = {};
      editAttributes.forEach(([k, v]) => {
        if (k.trim()) {
          try {
            attributes[k.trim()] = JSON.parse(v);
          } catch {
            attributes[k.trim()] = v;
          }
        }
      });
      const updated = await updateEntity(selectedEntity.id, {
        name: editName.trim() || selectedEntity.name,
        attributes,
      });
      setSelectedEntity(updated);
      onEntityUpdate?.(updated);
      setIsEditing(false);
    } catch (error) {
      kgViewLogger.error('Failed to update entity', { error });
      // Could add toast here if desired; keeping minimal
    } finally {
      setIsSaving(false);
    }
  }, [selectedEntity, editName, editAttributes, onEntityUpdate]);

  const entityRelations = useMemo(() => {
    if (!selectedEntity) return [];
    return relations.filter(
      r => r.source_id === selectedEntity.id || r.target_id === selectedEntity.id
    );
  }, [selectedEntity, relations]);

  const handleCreateEntity = useCallback(async () => {
    if (!storyId) return;
    const name = window.prompt('实体名称');
    if (!name?.trim()) return;
    const typeInput = window.prompt(
      `实体类型 (${Object.keys(ENTITY_LABELS).join(', ')})`,
      'Character'
    );
    if (!typeInput || !(typeInput in ENTITY_LABELS)) {
      toast.error('无效的实体类型');
      return;
    }
    try {
      const { createEntity } = await import('@/services/api/genesis');
      const entity = await createEntity({
        story_id: storyId,
        name: name.trim(),
        entity_type: typeInput,
      });
      onEntityCreate?.(entity);
      toast.success('实体已创建');
    } catch (error) {
      kgViewLogger.error('Failed to create entity', { error });
      toast.error('创建实体失败');
    }
  }, [storyId, onEntityCreate]);

  const handleCreateRelation = useCallback(async () => {
    if (!storyId || !selectedEntity) return;
    const targetName = window.prompt('目标实体名称');
    if (!targetName?.trim()) return;
    const target = entities.find(
      e => e.name.toLowerCase() === targetName.trim().toLowerCase()
    );
    if (!target) {
      toast.error('未找到目标实体');
      return;
    }
    const relationType = window.prompt('关系类型', '关联');
    if (!relationType?.trim()) return;
    try {
      const { createRelation } = await import('@/services/api/genesis');
      const relation = await createRelation({
        story_id: storyId,
        from_entity_id: selectedEntity.id,
        to_entity_id: target.id,
        relation_type: relationType.trim(),
      });
      onRelationCreate?.(relation);
      toast.success('关系已创建');
    } catch (error) {
      kgViewLogger.error('Failed to create relation', { error });
      toast.error('创建关系失败');
    }
  }, [storyId, selectedEntity, entities, onRelationCreate]);

  const getConnectedEntity = (relation: Relation) => {
    const otherId =
      relation.source_id === selectedEntity?.id ? relation.target_id : relation.source_id;
    return entities.find(e => e.id === otherId);
  };

  const hiddenCount = entities.length - filteredEntities.length;
  const lodHiddenCount = filteredEntities.length - lodEntities.length;
  const showMiniMap = entities.length <= MINIMAP_THRESHOLD;

  return (
    <div className={cn('relative w-full h-full bg-cinema-950', className)}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={handleNodeClick}
        onNodeDoubleClick={handleNodeDoubleClick}
        fitView={fitViewFlag}
        onInit={() => setFitViewFlag(false)}
        minZoom={0.2}
        maxZoom={2}
        proOptions={{ hideAttribution: true }}
        // C1: 启用 ReactFlow 内置视口裁剪
        onlyRenderVisibleElements
      >
        <Background color="#333" gap={20} size={1} />
        <Controls className="bg-cinema-900 border-cinema-800" />
        {showMiniMap && (
          <MiniMap
            nodeColor={node => {
              const type = (node.data?.entity as Entity)?.entity_type;
              return type ? ENTITY_COLORS[type] : '#666';
            }}
            className="bg-cinema-900 border-cinema-800"
            maskColor="rgba(0,0,0,0.5)"
          />
        )}

        {/* Legend Panel */}
        <Panel
          position="top-left"
          className="bg-cinema-900/90 border border-cinema-800 rounded-xl p-3 m-2"
        >
          <h3 className="text-sm font-semibold text-white mb-2">图例</h3>
          <div className="space-y-1.5">
            {(Object.keys(ENTITY_COLORS) as EntityType[]).map(type => (
              <div key={type} className="flex items-center gap-2">
                <span
                  className="w-3 h-3 rounded-sm"
                  style={{ backgroundColor: ENTITY_COLORS[type] }}
                />
                <span className="text-xs text-gray-300">{ENTITY_LABELS[type]}</span>
              </div>
            ))}
          </div>
          <div className="mt-3 pt-2 border-t border-cinema-800 text-xs text-gray-500">
            <p>
              节点: {lodEntities.length}
              {hiddenCount > 0 && <span className="text-gray-600"> / {entities.length}</span>}
            </p>
            <p>关系: {filteredRelations.length}</p>
            {hiddenCount > 0 && (
              <p className="text-cinema-gold mt-1">已筛选隐藏 {hiddenCount} 个</p>
            )}
            {lodHiddenCount > 0 && (
              <p className="text-cinema-gold mt-1">LOD 已折叠 {lodHiddenCount} 个低重要性节点</p>
            )}
            {storyId && (
              <button
                onClick={handleCreateEntity}
                className="mt-3 w-full flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium bg-cinema-gold/10 text-cinema-gold hover:bg-cinema-gold/20 transition-colors"
              >
                <Plus className="w-3.5 h-3.5" />
                新建实体
              </button>
            )}
          </div>
        </Panel>

        {/* Search & Filter Panel */}
        <Panel position="top-right" className="m-2">
          <div className="bg-cinema-900/90 border border-cinema-800 rounded-xl p-3 w-64">
            <div className="flex items-center gap-2 mb-2">
              <div className="relative flex-1">
                <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={e => setSearchQuery(e.target.value)}
                  placeholder="搜索节点..."
                  className="w-full bg-cinema-800 border border-cinema-700 rounded-md pl-7 pr-7 py-1.5 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold"
                />
                {searchQuery && (
                  <button
                    onClick={clearSearch}
                    className="absolute right-1.5 top-1/2 -translate-y-1/2 text-gray-500 hover:text-white"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                )}
              </div>
              <button
                onClick={() => setShowFilters(s => !s)}
                className={cn(
                  'p-1.5 rounded-md border transition-colors',
                  showFilters
                    ? 'bg-cinema-gold/20 border-cinema-gold text-cinema-gold'
                    : 'bg-cinema-800 border-cinema-700 text-gray-400 hover:text-white'
                )}
                title="筛选类型"
              >
                <Filter className="w-4 h-4" />
              </button>
            </div>

            {showFilters && (
              <div className="pt-2 border-t border-cinema-800">
                <div className="flex flex-wrap gap-1.5">
                  {(Object.keys(ENTITY_COLORS) as EntityType[]).map(type => {
                    const active = visibleTypes.has(type);
                    return (
                      <button
                        key={type}
                        onClick={() => toggleType(type)}
                        className={cn(
                          'px-2 py-1 rounded text-[10px] font-medium border transition-all',
                          active
                            ? 'text-white border-transparent'
                            : 'text-gray-500 border-cinema-700 bg-cinema-800/50'
                        )}
                        style={
                          active
                            ? {
                                backgroundColor: `${ENTITY_COLORS[type]}40`,
                                borderColor: ENTITY_COLORS[type],
                              }
                            : undefined
                        }
                      >
                        {ENTITY_LABELS[type]}
                      </button>
                    );
                  })}
                </div>
                <div className="flex items-center justify-between mt-2">
                  <button
                    onClick={() =>
                      setVisibleTypes(new Set(Object.keys(ENTITY_COLORS) as EntityType[]))
                    }
                    className="text-[10px] text-gray-400 hover:text-white"
                  >
                    全选
                  </button>
                  <button
                    onClick={() => setVisibleTypes(new Set())}
                    className="text-[10px] text-gray-400 hover:text-white"
                  >
                    清空
                  </button>
                </div>
              </div>
            )}

            {/* C1: LOD 显示全部按钮 */}
            {filteredEntities.length > NODE_LOD_THRESHOLD && (
              <div className="pt-2 border-t border-cinema-800">
                <button
                  onClick={() => setShowAllNodes(s => !s)}
                  className="w-full py-1.5 text-[11px] font-medium rounded-md border border-cinema-gold/30 bg-cinema-gold/10 text-cinema-gold hover:bg-cinema-gold/20 transition-colors"
                >
                  {showAllNodes
                    ? '仅显示高重要性节点'
                    : `显示全部 ${filteredEntities.length} 个节点`}
                </button>
                {!showAllNodes && (
                  <p className="text-[10px] text-gray-500 mt-1 text-center">
                    默认按关系数与类型优先级显示前 {NODE_LOD_THRESHOLD} 个节点
                  </p>
                )}
              </div>
            )}

            {filteredEntities.length === 0 && entities.length > 0 && (
              <div className="pt-2 border-t border-cinema-800 text-xs text-gray-500 text-center">
                无匹配节点
              </div>
            )}
          </div>
        </Panel>
      </ReactFlow>

      {/* Entity Detail Panel */}
      {selectedEntity && (
        <div className="absolute right-4 top-4 bottom-4 w-72 bg-cinema-900/95 border border-cinema-800 rounded-xl p-4 overflow-y-auto shadow-2xl backdrop-blur-sm">
          <div className="flex items-start justify-between mb-3">
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-1">
                <span
                  className="inline-block px-2 py-0.5 rounded text-[10px] font-medium text-white"
                  style={{ backgroundColor: ENTITY_COLORS[selectedEntity.entity_type] }}
                >
                  {ENTITY_LABELS[selectedEntity.entity_type]}
                </span>
                {selectedEntity.is_auto_generated && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold flex items-center gap-1">
                    <Star className="w-3 h-3" />
                    创世
                  </span>
                )}
              </div>
              {isEditing ? (
                <input
                  type="text"
                  value={editName}
                  onChange={e => setEditName(e.target.value)}
                  className="w-full bg-cinema-800 border border-cinema-700 rounded-md px-2 py-1 text-sm text-white focus:outline-none focus:border-cinema-gold"
                  placeholder="实体名称"
                />
              ) : (
                <h3 className="text-lg font-bold text-white truncate">{selectedEntity.name}</h3>
              )}
            </div>
            <div className="flex items-center gap-1 ml-2">
              {!isEditing && (
                <button
                  onClick={startEditing}
                  className="p-1 text-gray-500 hover:text-cinema-gold transition-colors"
                  title="编辑"
                >
                  <Pencil className="w-4 h-4" />
                </button>
              )}
              <button
                onClick={() => setSelectedEntity(null)}
                className="p-1 text-gray-500 hover:text-white transition-colors"
                title="关闭"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          </div>

          <div className="mb-4">
            <div className="flex items-center justify-between mb-2">
              <h4 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">属性</h4>
              {isEditing && (
                <button
                  onClick={() => setEditAttributes(prev => [...prev, ['', '']])}
                  className="flex items-center gap-1 text-[10px] text-cinema-gold hover:text-cinema-gold/80"
                >
                  <Plus className="w-3 h-3" />
                  添加
                </button>
              )}
            </div>
            {isEditing ? (
              <div className="space-y-2">
                {editAttributes.map(([key, value], idx) => (
                  <div key={idx} className="flex items-center gap-1.5">
                    <input
                      type="text"
                      value={key}
                      onChange={e =>
                        setEditAttributes(prev => {
                          const next = [...prev];
                          next[idx] = [e.target.value, next[idx][1]];
                          return next;
                        })
                      }
                      placeholder="键"
                      className="flex-1 min-w-0 bg-cinema-800 border border-cinema-700 rounded-md px-1.5 py-1 text-[11px] text-white focus:outline-none focus:border-cinema-gold"
                    />
                    <input
                      type="text"
                      value={value}
                      onChange={e =>
                        setEditAttributes(prev => {
                          const next = [...prev];
                          next[idx] = [next[idx][0], e.target.value];
                          return next;
                        })
                      }
                      placeholder="值"
                      className="flex-[1.5] min-w-0 bg-cinema-800 border border-cinema-700 rounded-md px-1.5 py-1 text-[11px] text-white focus:outline-none focus:border-cinema-gold"
                    />
                    <button
                      onClick={() => setEditAttributes(prev => prev.filter((_, i) => i !== idx))}
                      className="p-1 text-gray-500 hover:text-red-400"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ))}
                {editAttributes.length === 0 && (
                  <p className="text-xs text-gray-500 italic">暂无属性</p>
                )}
              </div>
            ) : selectedEntity.attributes && Object.keys(selectedEntity.attributes).length > 0 ? (
              <div className="space-y-1.5">
                {Object.entries(selectedEntity.attributes).map(([key, value]) => (
                  <div key={key} className="text-sm">
                    <span className="text-cinema-gold">{key}:</span>{' '}
                    <span className="text-gray-300">
                      {typeof value === 'string' ? value : JSON.stringify(value)}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-gray-500">暂无属性</p>
            )}
          </div>

          <div className="mb-4">
            <div className="flex items-center justify-between mb-2">
              <h4 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">关系</h4>
              {storyId && (
                <button
                  onClick={handleCreateRelation}
                  className="flex items-center gap-1 text-[10px] text-cinema-gold hover:text-cinema-gold/80"
                >
                  <Plus className="w-3 h-3" />
                  添加关系
                </button>
              )}
            </div>
            {entityRelations.length === 0 ? (
              <p className="text-sm text-gray-500">暂无关系</p>
            ) : (
              <div className="space-y-2">
                {entityRelations.map(relation => {
                  const other = getConnectedEntity(relation);
                  const isSource = relation.source_id === selectedEntity.id;
                  return (
                    <div key={relation.id} className="p-2 bg-cinema-800/50 rounded-lg text-sm">
                      <div className="flex items-center gap-1 text-gray-300">
                        <span className={isSource ? 'text-cinema-gold' : 'text-gray-300'}>
                          {selectedEntity.name}
                        </span>
                        <span className="text-gray-500">→</span>
                        <span className={!isSource ? 'text-cinema-gold' : 'text-gray-300'}>
                          {other?.name || '未知'}
                        </span>
                      </div>
                      <div className="flex items-center justify-between mt-1">
                        <span className="text-xs text-gray-400">{relation.relation_type}</span>
                        <div className="flex items-center gap-1">
                          <div
                            className="h-1 rounded-full bg-cinema-gold"
                            style={{
                              width: `${relation.strength * 24}px`,
                              opacity: 0.6 + relation.strength * 0.4,
                            }}
                          />
                          <span className="text-[10px] text-gray-500">
                            {Math.round(relation.strength * 100)}%
                          </span>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <div className="text-xs text-gray-600 pt-3 border-t border-cinema-800">
            <p>首次出现: {new Date(selectedEntity.first_seen).toLocaleDateString()}</p>
          </div>

          {isEditing && (
            <div className="flex items-center gap-2 mt-4 pt-3 border-t border-cinema-800">
              <button
                onClick={saveEditing}
                disabled={isSaving}
                className="flex-1 flex items-center justify-center gap-1 px-3 py-1.5 rounded-md bg-cinema-gold/20 text-cinema-gold border border-cinema-gold/30 hover:bg-cinema-gold/30 transition-colors disabled:opacity-50 text-sm"
              >
                <Check className="w-3.5 h-3.5" />
                {isSaving ? '保存中...' : '保存'}
              </button>
              <button
                onClick={cancelEditing}
                disabled={isSaving}
                className="flex-1 flex items-center justify-center gap-1 px-3 py-1.5 rounded-md bg-cinema-800 text-gray-300 hover:bg-cinema-700 transition-colors disabled:opacity-50 text-sm"
              >
                <RotateCcw className="w-3.5 h-3.5" />
                取消
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export const KnowledgeGraphView: React.FC<KnowledgeGraphViewProps> = props => {
  return (
    <ReactFlowProvider>
      <KnowledgeGraphViewInner {...props} />
    </ReactFlowProvider>
  );
};

export default KnowledgeGraphView;
