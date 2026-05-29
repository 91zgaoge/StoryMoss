/**
 * ChapterOutline - 章节大纲侧边栏组件
 *
 * 功能：
 * - 显示章节大纲结构
 * - 支持拖动排序
 * - 快速跳转到指定位置
 * - 显示章节标题和内容预览
 */

import React, { useState, useCallback } from 'react';
import { GripVertical, ChevronRight, ChevronDown, Plus, Edit3, Trash2 } from 'lucide-react';
import { cn } from '@/utils/cn';

interface OutlineItem {
  id: string;
  level: number;
  title: string;
  content?: string;
  wordCount?: number;
}

interface ChapterOutlineProps {
  items: OutlineItem[];
  onReorder: (items: OutlineItem[]) => void;
  onSelect: (id: string) => void;
  onEdit: (id: string, title: string) => void;
  onDelete: (id: string) => void;
  onAdd: (parentId?: string) => void;
  selectedId?: string;
  className?: string;
}

export const ChapterOutline: React.FC<ChapterOutlineProps> = ({
  items,
  onReorder,
  onSelect,
  onEdit,
  onDelete,
  onAdd,
  selectedId,
  className,
}) => {
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');

  const toggleExpand = (id: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const handleDragStart = (e: React.DragEvent, id: string) => {
    setDraggedId(id);
    e.dataTransfer.effectAllowed = 'move';
  };

  const handleDragOver = (e: React.DragEvent, id: string) => {
    e.preventDefault();
    if (id !== draggedId) {
      setDragOverId(id);
    }
  };

  const handleDrop = (e: React.DragEvent, targetId: string) => {
    e.preventDefault();
    if (!draggedId || draggedId === targetId) {
      setDraggedId(null);
      setDragOverId(null);
      return;
    }

    const draggedIndex = items.findIndex(item => item.id === draggedId);
    const targetIndex = items.findIndex(item => item.id === targetId);

    const newItems = [...items];
    const [removed] = newItems.splice(draggedIndex, 1);
    newItems.splice(targetIndex, 0, removed);

    onReorder(newItems);
    setDraggedId(null);
    setDragOverId(null);
  };

  const handleEditStart = (item: OutlineItem) => {
    setEditingId(item.id);
    setEditValue(item.title);
  };

  const handleEditSave = () => {
    if (editingId && editValue.trim()) {
      onEdit(editingId, editValue.trim());
    }
    setEditingId(null);
    setEditValue('');
  };

  const handleEditCancel = () => {
    setEditingId(null);
    setEditValue('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleEditSave();
    } else if (e.key === 'Escape') {
      handleEditCancel();
    }
  };

  const getIndentStyle = (level: number) => ({
    paddingLeft: `${level * 20 + 12}px`,
  });

  return (
    <div
      className={cn('chapter-outline flex flex-col h-full bg-[var(--parchment-dark)]', className)}
    >
      {/* 头部 */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--warm-sand)]">
        <h3 className="text-sm font-medium text-[var(--charcoal)]">章节大纲</h3>
        <button
          onClick={() => onAdd()}
          className="p-1.5 rounded-md hover:bg-[var(--warm-sand)] text-[var(--terracotta)] transition-colors"
          title="添加章节"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* 大纲列表 */}
      <div className="flex-1 overflow-y-auto py-2">
        {items.length === 0 ? (
          <div className="px-4 py-8 text-center">
            <p className="text-sm text-[var(--stone-gray)]">暂无大纲</p>
            <button
              onClick={() => onAdd()}
              className="mt-2 text-sm text-[var(--terracotta)] hover:underline"
            >
              添加第一个章节
            </button>
          </div>
        ) : (
          items.map((item, index) => (
            <div
              key={item.id}
              draggable
              onDragStart={e => handleDragStart(e, item.id)}
              onDragOver={e => handleDragOver(e, item.id)}
              onDrop={e => handleDrop(e, item.id)}
              className={cn(
                'group relative flex items-center py-2 px-3 mx-2 rounded-lg cursor-pointer transition-colors duration-200',
                'hover:bg-[var(--warm-sand)]',
                selectedId === item.id && 'bg-[var(--terracotta)]/10',
                draggedId === item.id && 'opacity-50',
                dragOverId === item.id && 'border-t-2 border-[var(--terracotta)]',
                index > 0 && items[index - 1].level < item.level && 'mt-1'
              )}
              style={getIndentStyle(item.level)}
              onClick={() => onSelect(item.id)}
            >
              {/* 拖动手柄 */}
              <div
                className="absolute left-0 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 cursor-grab active:cursor-grabbing"
                onClick={e => e.stopPropagation()}
              >
                <GripVertical className="w-4 h-4 text-[var(--stone-gray)]" />
              </div>

              {/* 展开/折叠按钮 */}
              {item.content && (
                <button
                  onClick={e => {
                    e.stopPropagation();
                    toggleExpand(item.id);
                  }}
                  className="mr-1 p-0.5 rounded hover:bg-[var(--warm-sand)]"
                >
                  {expandedIds.has(item.id) ? (
                    <ChevronDown className="w-3.5 h-3.5 text-[var(--stone-gray)]" />
                  ) : (
                    <ChevronRight className="w-3.5 h-3.5 text-[var(--stone-gray)]" />
                  )}
                </button>
              )}

              {/* 标题内容 */}
              <div className="flex-1 min-w-0 ml-1">
                {editingId === item.id ? (
                  <input
                    type="text"
                    value={editValue}
                    onChange={e => setEditValue(e.target.value)}
                    onBlur={handleEditSave}
                    onKeyDown={handleKeyDown}
                    autoFocus
                    className="w-full px-2 py-1 text-sm bg-white border border-[var(--terracotta)] rounded focus:outline-none"
                    onClick={e => e.stopPropagation()}
                  />
                ) : (
                  <>
                    <p
                      className={cn(
                        'text-sm truncate',
                        selectedId === item.id
                          ? 'text-[var(--terracotta)] font-medium'
                          : 'text-[var(--charcoal)]'
                      )}
                    >
                      {item.title || '未命名章节'}
                    </p>
                    {item.wordCount !== undefined && (
                      <p className="text-xs text-[var(--stone-gray)]">{item.wordCount} 字</p>
                    )}
                  </>
                )}
              </div>

              {/* 操作按钮 */}
              <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={e => {
                    e.stopPropagation();
                    handleEditStart(item);
                  }}
                  className="p-1 rounded hover:bg-[var(--warm-sand)] text-[var(--stone-gray)] hover:text-[var(--charcoal)] active:scale-90"
                  title="编辑"
                >
                  <Edit3 className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={e => {
                    e.stopPropagation();
                    onDelete(item.id);
                  }}
                  className="p-1 rounded hover:bg-red-100 text-[var(--stone-gray)] hover:text-red-500 active:scale-90"
                  title="删除"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          ))
        )}
      </div>

      {/* 底部统计 */}
      {items.length > 0 && (
        <div className="px-4 py-3 border-t border-[var(--warm-sand)] text-xs text-[var(--stone-gray)]">
          <p>共 {items.length} 个章节</p>
          <p>总计 {items.reduce((sum, item) => sum + (item.wordCount || 0), 0)} 字</p>
        </div>
      )}
    </div>
  );
};

export default ChapterOutline;
