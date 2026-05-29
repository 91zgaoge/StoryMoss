import React, { useState, useCallback } from 'react';
import {
  Plus,
  Trash2,
  Edit3,
  ArrowUp,
  ArrowDown,
  GripVertical,
  Target,
  Zap,
  Users,
  Eye,
  FileText,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Card } from '@/components/ui/Card';
import type { Scene, ConflictType } from '@/types';
import { getConflictTypeLabel, getConflictTypeColor } from '@/hooks/useScenes';

// 根据场景序号计算叙事阶段颜色
function getScenePhaseColor(sequenceNumber: number): string {
  if (sequenceNumber <= 15) return 'bg-blue-500';
  if (sequenceNumber <= 70) return 'bg-amber-500';
  if (sequenceNumber <= 85) return 'bg-red-500';
  return 'bg-green-500';
}

function getScenePhaseLabel(sequenceNumber: number): string {
  if (sequenceNumber <= 15) return '铺垫';
  if (sequenceNumber <= 70) return '上升';
  if (sequenceNumber <= 85) return '高潮';
  return '收尾';
}

// W4-B11: 场景执行阶段标签
function getExecutionStageLabel(stage?: string): string {
  switch (stage) {
    case 'planning':
      return '规划';
    case 'outline':
      return '大纲';
    case 'drafting':
      return '起草';
    case 'review':
      return '审校';
    case 'final':
      return '定稿';
    default:
      return '规划';
  }
}

function getExecutionStageColor(stage?: string): { bg: string; text: string } {
  switch (stage) {
    case 'planning':
      return { bg: 'bg-gray-500/20', text: 'text-gray-400' };
    case 'outline':
      return { bg: 'bg-blue-500/20', text: 'text-blue-400' };
    case 'drafting':
      return { bg: 'bg-purple-500/20', text: 'text-purple-400' };
    case 'review':
      return { bg: 'bg-orange-500/20', text: 'text-orange-400' };
    case 'final':
      return { bg: 'bg-green-500/20', text: 'text-green-400' };
    default:
      return { bg: 'bg-gray-500/20', text: 'text-gray-400' };
  }
}

interface StoryTimelineProps {
  scenes: Scene[];
  currentSceneId?: string | null;
  characters: { id: string; name: string }[];
  onSelectScene: (scene: Scene) => void;
  onCreateScene: () => void;
  onDeleteScene: (sceneId: string) => void;
  onReorderScenes: (sceneIds: string[]) => void;
  onEditScene: (scene: Scene) => void;
}

export function StoryTimeline({
  scenes,
  currentSceneId,
  characters,
  onSelectScene,
  onCreateScene,
  onDeleteScene,
  onReorderScenes,
  onEditScene,
}: StoryTimelineProps) {
  const [draggedScene, setDraggedScene] = useState<Scene | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

  const characterMap = React.useMemo(() => {
    const map = new Map<string, string>();
    for (const c of characters) {
      map.set(c.id, c.name);
    }
    return map;
  }, [characters]);

  const handleDragStart = useCallback((scene: Scene) => {
    setDraggedScene(scene);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault();
    setDragOverIndex(index);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, dropIndex: number) => {
      e.preventDefault();
      if (!draggedScene) return;

      const draggedIndex = scenes.findIndex(s => s.id === draggedScene.id);
      if (draggedIndex === dropIndex) {
        setDraggedScene(null);
        setDragOverIndex(null);
        return;
      }

      // Reorder
      const newScenes = [...scenes];
      const [removed] = newScenes.splice(draggedIndex, 1);
      newScenes.splice(dropIndex, 0, removed);

      onReorderScenes(newScenes.map(s => s.id));
      setDraggedScene(null);
      setDragOverIndex(null);
    },
    [draggedScene, scenes, onReorderScenes]
  );

  const moveScene = useCallback(
    (index: number, direction: 'up' | 'down') => {
      if (direction === 'up' && index === 0) return;
      if (direction === 'down' && index === scenes.length - 1) return;

      const newScenes = [...scenes];
      const swapIndex = direction === 'up' ? index - 1 : index + 1;
      [newScenes[index], newScenes[swapIndex]] = [newScenes[swapIndex], newScenes[index]];

      onReorderScenes(newScenes.map(s => s.id));
    },
    [scenes, onReorderScenes]
  );

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-white">故事线</h2>
          <p className="text-sm text-gray-400">{scenes.length} 个场景</p>
        </div>
        <Button variant="primary" size="sm" onClick={onCreateScene}>
          <Plus className="w-4 h-4 mr-1" />
          添加场景
        </Button>
      </div>

      {/* Timeline */}
      <div className="relative space-y-2">
        {/* Timeline line */}
        <div className="absolute left-6 top-4 bottom-4 w-0.5 bg-cinema-700" />

        {scenes.map((scene, index) => (
          <div
            key={scene.id}
            draggable
            onDragStart={() => handleDragStart(scene)}
            onDragOver={e => handleDragOver(e, index)}
            onDrop={e => handleDrop(e, index)}
            className={`
              relative flex items-start gap-3 p-3 rounded-lg cursor-pointer
              transition-all duration-200 group
              ${
                currentSceneId === scene.id
                  ? 'bg-cinema-gold/10 border border-cinema-gold/30'
                  : 'bg-cinema-800/50 border border-transparent hover:bg-cinema-800'
              }
              ${dragOverIndex === index ? 'border-dashed border-cinema-gold' : ''}
            `}
            onClick={() => onSelectScene(scene)}
          >
            {/* Timeline dot with phase color */}
            <div className="relative z-10 flex-shrink-0 flex flex-col items-center gap-1">
              <div
                className={`w-3 h-3 rounded-full ${getScenePhaseColor(scene.sequence_number)}`}
              />
              <span className="text-[10px] text-gray-500 leading-none">
                {getScenePhaseLabel(scene.sequence_number)}
              </span>
            </div>

            {/* Scene number */}
            <div className="flex-shrink-0 w-8 text-center text-xs text-gray-500 font-mono">
              #{scene.sequence_number}
            </div>

            {/* Scene content */}
            <div className="flex-1 min-w-0">
              <div className="flex items-start justify-between gap-2">
                <h3 className="font-medium text-white truncate">
                  {scene.title || `场景 ${scene.sequence_number}`}
                </h3>

                {/* Execution stage badge */}
                <span
                  className={`
                  inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium shrink-0
                  ${getExecutionStageColor(scene.execution_stage).bg}
                  ${getExecutionStageColor(scene.execution_stage).text}
                `}
                >
                  {getExecutionStageLabel(scene.execution_stage)}
                </span>

                {/* Actions */}
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={e => {
                      e.stopPropagation();
                      moveScene(index, 'up');
                    }}
                    disabled={index === 0}
                  >
                    <ArrowUp className="w-3 h-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={e => {
                      e.stopPropagation();
                      moveScene(index, 'down');
                    }}
                    disabled={index === scenes.length - 1}
                  >
                    <ArrowDown className="w-3 h-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={e => {
                      e.stopPropagation();
                      onEditScene(scene);
                    }}
                  >
                    <Edit3 className="w-3 h-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0 text-red-400 hover:text-red-300"
                    onClick={e => {
                      e.stopPropagation();
                      onDeleteScene(scene.id);
                    }}
                  >
                    <Trash2 className="w-3 h-3" />
                  </Button>
                </div>
              </div>

              {/* Outline content subtitle */}
              {scene.outline_content && (
                <p className="mt-1 text-xs text-gray-500 line-clamp-2 flex items-center gap-1">
                  <FileText className="w-3 h-3 flex-shrink-0" />
                  {scene.outline_content}
                </p>
              )}

              {/* Drama info */}
              <div className="mt-2 flex flex-wrap gap-2">
                {scene.conflict_type && (
                  <span
                    className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full"
                    style={{
                      backgroundColor: `${getConflictTypeColor(scene.conflict_type)}20`,
                      color: getConflictTypeColor(scene.conflict_type),
                    }}
                  >
                    <Zap className="w-3 h-3" />
                    {getConflictTypeLabel(scene.conflict_type)}
                  </span>
                )}

                {scene.dramatic_goal && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full bg-cinema-700 text-gray-300">
                    <Target className="w-3 h-3" />
                    有戏剧目标
                  </span>
                )}

                {scene.characters_present.length > 0 && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full bg-cinema-700 text-gray-300">
                    <Users className="w-3 h-3" />
                    {scene.characters_present.length} 角色
                  </span>
                )}

                {scene.foreshadowing_ids && scene.foreshadowing_ids.length > 0 && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full bg-purple-500/20 text-purple-300">
                    <Eye className="w-3 h-3" />
                    {scene.foreshadowing_ids.length} 伏笔
                  </span>
                )}
              </div>

              {/* Character avatars */}
              {scene.characters_present.length > 0 && (
                <div className="mt-2 flex items-center gap-1">
                  <div className="flex -space-x-1.5">
                    {scene.characters_present.slice(0, 5).map(charId => {
                      const charName = characterMap.get(charId) || '?';
                      return (
                        <div
                          key={charId}
                          className="w-5 h-5 rounded-full bg-cinema-700 border border-cinema-800 flex items-center justify-center text-[9px] text-gray-300"
                          title={charName}
                        >
                          {charName.charAt(0)}
                        </div>
                      );
                    })}
                    {scene.characters_present.length > 5 && (
                      <div className="w-5 h-5 rounded-full bg-cinema-800 border border-cinema-900 flex items-center justify-center text-[9px] text-gray-400">
                        +{scene.characters_present.length - 5}
                      </div>
                    )}
                  </div>
                  <span className="text-[10px] text-gray-600 ml-1">
                    {scene.characters_present
                      .map(id => characterMap.get(id) || '?')
                      .slice(0, 3)
                      .join('、')}
                    {scene.characters_present.length > 3 && ' 等'}
                  </span>
                </div>
              )}

              {/* External pressure preview */}
              {scene.external_pressure && (
                <p className="mt-2 text-xs text-gray-400 line-clamp-2">
                  压迫: {scene.external_pressure}
                </p>
              )}

              {/* Content preview */}
              {scene.content && (
                <p className="mt-1 text-xs text-gray-500 line-clamp-2">
                  {scene.content.substring(0, 100)}
                  {scene.content.length > 100 ? '...' : ''}
                </p>
              )}
            </div>

            {/* Drag handle */}
            <div className="flex-shrink-0 text-gray-600 cursor-grab active:cursor-grabbing">
              <GripVertical className="w-4 h-4" />
            </div>
          </div>
        ))}

        {scenes.length === 0 && (
          <div className="text-center py-8">
            <p className="text-gray-500 mb-4">还没有场景</p>
            <Button variant="primary" onClick={onCreateScene}>
              <Plus className="w-4 h-4 mr-2" />
              创建第一个场景
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
