import { useState } from 'react';
import { Plus, Check, RotateCcw, Trash2, Edit3 } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import {
  useSceneAnnotations,
  useCreateSceneAnnotation,
  useUpdateSceneAnnotation,
  useResolveSceneAnnotation,
  useUnresolveSceneAnnotation,
  useDeleteSceneAnnotation,
  ANNOTATION_TYPE_LABELS,
  ANNOTATION_TYPE_COLORS,
} from '@/hooks/useSceneAnnotations';
import type { SceneAnnotation } from '@/types/v3';

interface SceneAnnotationPanelProps {
  sceneId: string;
  storyId: string;
}

export function SceneAnnotationPanel({ sceneId, storyId }: SceneAnnotationPanelProps) {
  const [newAnnotationContent, setNewAnnotationContent] = useState('');
  const [newAnnotationType, setNewAnnotationType] =
    useState<SceneAnnotation['annotation_type']>('note');
  const [editingAnnotationId, setEditingAnnotationId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');

  const { data: annotations = [], isLoading: annotationsLoading } = useSceneAnnotations(sceneId);
  const createAnnotation = useCreateSceneAnnotation();
  const updateAnnotation = useUpdateSceneAnnotation();
  const resolveAnnotation = useResolveSceneAnnotation();
  const unresolveAnnotation = useUnresolveSceneAnnotation();
  const deleteAnnotation = useDeleteSceneAnnotation();

  return (
    <div className="space-y-4">
      {/* New Annotation Form */}
      <Card>
        <CardContent className="p-4 space-y-3">
          <h3 className="font-medium text-white flex items-center gap-2">
            <Plus className="w-4 h-4 text-cinema-gold" />
            新建批注
          </h3>
          <div className="flex gap-2">
            {(['note', 'todo', 'warning', 'idea'] as const).map(type => (
              <button
                key={type}
                onClick={() => setNewAnnotationType(type)}
                className={`
                  px-2.5 py-1 rounded-md text-xs font-medium transition-colors
                  ${
                    newAnnotationType === type
                      ? 'bg-cinema-700 text-white'
                      : 'bg-cinema-800 text-gray-400 hover:bg-cinema-700'
                  }
                `}
              >
                <span
                  className={`inline-block w-2 h-2 rounded-full mr-1.5 ${ANNOTATION_TYPE_COLORS[type]}`}
                />
                {ANNOTATION_TYPE_LABELS[type]}
              </button>
            ))}
          </div>
          <textarea
            value={newAnnotationContent}
            onChange={e => setNewAnnotationContent(e.target.value)}
            placeholder="记录想法、待办事项或提醒..."
            rows={3}
            className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none resize-none"
          />
          <div className="flex justify-end">
            <Button
              variant="primary"
              size="sm"
              disabled={!newAnnotationContent.trim() || createAnnotation.isPending}
              onClick={() => {
                createAnnotation.mutate(
                  {
                    scene_id: sceneId,
                    story_id: storyId,
                    content: newAnnotationContent.trim(),
                    annotation_type: newAnnotationType,
                  },
                  {
                    onSuccess: () => setNewAnnotationContent(''),
                  }
                );
              }}
            >
              <Plus className="w-4 h-4 mr-1" />
              添加批注
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Annotations List */}
      {annotationsLoading ? (
        <p className="text-sm text-gray-500 text-center py-4">加载中...</p>
      ) : annotations.length === 0 ? (
        <p className="text-sm text-gray-500 text-center py-8">暂无批注</p>
      ) : (
        <div className="space-y-3">
          {annotations.map(annotation => (
            <Card key={annotation.id} className={annotation.resolved_at ? 'opacity-60' : ''}>
              <CardContent className="p-4">
                <div className="flex items-start gap-3">
                  <div
                    className={`
                    w-2 h-2 mt-1.5 rounded-full shrink-0
                    ${ANNOTATION_TYPE_COLORS[annotation.annotation_type]}
                  `}
                  />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs font-medium text-gray-400">
                        {ANNOTATION_TYPE_LABELS[annotation.annotation_type]}
                      </span>
                      <span className="text-xs text-gray-600">
                        {new Date(annotation.created_at).toLocaleString()}
                      </span>
                      {annotation.resolved_at && (
                        <span className="text-xs text-green-500">已解决</span>
                      )}
                    </div>

                    {editingAnnotationId === annotation.id ? (
                      <div className="space-y-2">
                        <textarea
                          value={editingContent}
                          onChange={e => setEditingContent(e.target.value)}
                          rows={2}
                          className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none resize-none"
                        />
                        <div className="flex gap-2">
                          <Button
                            variant="primary"
                            size="sm"
                            disabled={!editingContent.trim() || updateAnnotation.isPending}
                            onClick={() => {
                              updateAnnotation.mutate(
                                {
                                  annotationId: annotation.id,
                                  content: editingContent.trim(),
                                },
                                {
                                  onSuccess: () => setEditingAnnotationId(null),
                                }
                              );
                            }}
                          >
                            保存
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setEditingAnnotationId(null)}
                          >
                            取消
                          </Button>
                        </div>
                      </div>
                    ) : (
                      <p
                        className={`text-sm ${annotation.resolved_at ? 'text-gray-500 line-through' : 'text-gray-200'}`}
                      >
                        {annotation.content}
                      </p>
                    )}
                  </div>

                  {editingAnnotationId !== annotation.id && (
                    <div className="flex items-center gap-1 shrink-0">
                      <button
                        onClick={() => {
                          setEditingAnnotationId(annotation.id);
                          setEditingContent(annotation.content);
                        }}
                        className="p-1.5 rounded-md text-gray-500 hover:text-white hover:bg-cinema-700 transition-colors"
                        title="编辑"
                      >
                        <Edit3 className="w-3.5 h-3.5" />
                      </button>
                      {annotation.resolved_at ? (
                        <button
                          onClick={() => unresolveAnnotation.mutate(annotation.id)}
                          className="p-1.5 rounded-md text-gray-500 hover:text-white hover:bg-cinema-700 transition-colors"
                          title="标记未解决"
                        >
                          <RotateCcw className="w-3.5 h-3.5" />
                        </button>
                      ) : (
                        <button
                          onClick={() => resolveAnnotation.mutate(annotation.id)}
                          className="p-1.5 rounded-md text-gray-500 hover:text-green-400 hover:bg-cinema-700 transition-colors"
                          title="标记已解决"
                        >
                          <Check className="w-3.5 h-3.5" />
                        </button>
                      )}
                      <button
                        onClick={() => deleteAnnotation.mutate(annotation.id)}
                        className="p-1.5 rounded-md text-gray-500 hover:text-red-400 hover:bg-cinema-700 transition-colors"
                        title="删除"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
