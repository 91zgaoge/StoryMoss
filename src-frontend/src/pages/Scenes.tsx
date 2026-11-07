import { confirmDialog } from '@/components/ui/confirm';
import { useState, useEffect } from 'react';
import { Plus, BookOpen, AlertCircle, History, FileText } from 'lucide-react';
import { ExecutionPanel } from '@/components/ExecutionPanel';
import { Button } from '@/components/ui/Button';
import { EmptyState } from '@/components/ui/EmptyState';
import { StoryTimeline } from '@/components/StoryTimeline';
import { SceneEditor } from '@/components/SceneEditor';
import { VersionTimeline } from '@/components/VersionTimeline';
import { DiffViewer } from '@/components/DiffViewer';
import {
  useScenes,
  useCreateScene,
  useUpdateScene,
  useDeleteScene,
  useReorderScenes,
} from '@/hooks/useScenes';
import { useCharacters } from '@/hooks/useCharacters';
import { useAppStore } from '@/stores/appStore';
import { useCreateSceneVersion } from '@/hooks/useSceneVersions';
import type { Scene, SceneVersion } from '@/types';
import toast from 'react-hot-toast';

export function Scenes() {
  const currentStory = useAppStore(s => s.currentStory);
  const pendingSceneId = useAppStore(s => s.pendingSceneId);
  const setPendingSceneId = useAppStore(s => s.setPendingSceneId);
  const [selectedSceneId, setSelectedSceneId] = useState<string | null>(null);

  const [isEditing, setIsEditing] = useState(false);
  const [previewTab, setPreviewTab] = useState<'content' | 'versions'>('content');
  const [compareVersions, setCompareVersions] = useState<[SceneVersion, SceneVersion] | null>(null);

  const { data: scenes = [], isLoading } = useScenes(currentStory?.id || null);
  const { data: characters = [] } = useCharacters(currentStory?.id || null);

  const createScene = useCreateScene();
  const updateScene = useUpdateScene();
  const deleteScene = useDeleteScene();
  const reorderScenes = useReorderScenes();
  const createVersion = useCreateSceneVersion();

  const selectedScene = scenes.find(s => s.id === selectedSceneId) || null;

  useEffect(() => {
    if (!pendingSceneId || scenes.length === 0) return;
    const scene = scenes.find(s => s.id === pendingSceneId);
    if (scene) {
      setSelectedSceneId(scene.id);
      setIsEditing(false);
    }
    setPendingSceneId(null);
  }, [pendingSceneId, scenes, setPendingSceneId]);

  const handleCreateScene = () => {
    if (!currentStory) {
      toast.error('请先选择一个故事');
      return;
    }

    const nextSequence =
      scenes.length > 0 ? Math.max(...scenes.map(s => s.sequence_number)) + 1 : 1;

    createScene.mutate(
      {
        storyId: currentStory.id,
        sequenceNumber: nextSequence,
        title: `场景 ${nextSequence}`,
      },
      {
        onSuccess: newScene => {
          toast.success('场景创建成功');
          setSelectedSceneId(newScene.id);
          setIsEditing(true);
        },
      }
    );
  };

  const handleSelectScene = (scene: Scene) => {
    setSelectedSceneId(scene.id);
    setIsEditing(false);
  };

  const handleEditScene = (scene: Scene) => {
    setSelectedSceneId(scene.id);
    setIsEditing(true);
  };

  const handleSaveScene = (updates: Partial<Scene>) => {
    if (!selectedScene || !currentStory) return;

    updateScene.mutate(
      {
        sceneId: selectedScene.id,
        storyId: currentStory.id,
        updates: {
          title: updates.title,
          dramatic_goal: updates.dramatic_goal,
          external_pressure: updates.external_pressure,
          conflict_type: updates.conflict_type,
          characters_present: updates.characters_present,
          character_conflicts: updates.character_conflicts,
          content: updates.content,
          setting_location: updates.setting_location,
          setting_time: updates.setting_time,
          setting_atmosphere: updates.setting_atmosphere,
          execution_stage: updates.execution_stage,
          outline_content: updates.outline_content,
          draft_content: updates.draft_content,
        },
      },
      {
        onSuccess: () => {
          toast.success('场景已保存');
          setIsEditing(false);
          // 自动创建版本快照（检测任何字段变更）
          const hasContentChange =
            updates.content !== undefined && updates.content !== selectedScene.content;
          const hasMetaChange =
            updates.title !== selectedScene.title ||
            updates.dramatic_goal !== selectedScene.dramatic_goal ||
            updates.external_pressure !== selectedScene.external_pressure ||
            updates.conflict_type !== selectedScene.conflict_type ||
            updates.characters_present !== selectedScene.characters_present ||
            updates.character_conflicts !== selectedScene.character_conflicts ||
            updates.setting_location !== selectedScene.setting_location ||
            updates.setting_time !== selectedScene.setting_time ||
            updates.setting_atmosphere !== selectedScene.setting_atmosphere;
          if (hasContentChange || hasMetaChange) {
            const changeParts: string[] = [];
            if (hasContentChange) changeParts.push('内容');
            if (updates.title !== selectedScene.title) changeParts.push('标题');
            if (updates.dramatic_goal !== selectedScene.dramatic_goal) changeParts.push('戏剧目标');
            if (updates.external_pressure !== selectedScene.external_pressure)
              changeParts.push('外部压迫');
            if (updates.conflict_type !== selectedScene.conflict_type) changeParts.push('冲突类型');
            if (updates.setting_location !== selectedScene.setting_location)
              changeParts.push('场景地点');
            if (updates.setting_time !== selectedScene.setting_time) changeParts.push('场景时间');
            if (updates.setting_atmosphere !== selectedScene.setting_atmosphere)
              changeParts.push('场景氛围');
            createVersion.mutate({
              sceneId: selectedScene.id,
              changeSummary:
                changeParts.length > 0 ? `编辑${changeParts.join('、')}` : '编辑场景元数据',
              createdBy: 'user',
            });
          }
        },
      }
    );
  };

  const handleDeleteScene = async (sceneId: string) => {
    if (!currentStory) return;

    if (await confirmDialog({ title: '确定要删除这个场景吗？此操作不可撤销。' })) {
      deleteScene.mutate(
        { sceneId, storyId: currentStory.id },
        {
          onSuccess: () => {
            toast.success('场景已删除');
            if (selectedSceneId === sceneId) {
              setSelectedSceneId(null);
              setIsEditing(false);
            }
          },
        }
      );
    }
  };

  const handleReorderScenes = (sceneIds: string[]) => {
    if (!currentStory) return;

    reorderScenes.mutate({
      storyId: currentStory.id,
      sceneIds,
    });
  };

  if (!currentStory) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <EmptyState
          icon={BookOpen}
          title="先选择一个故事"
          action={{
            label: '去故事库',
            onClick: () => useAppStore.getState().setCurrentView('stories'),
          }}
        />
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <div className="loading-reel" />
      </div>
    );
  }

  return (
    /* T2 主从模板（设计文档 §4）：<lg 单列堆叠（右栏给 70dvh 保底高度），
       lg 起双栏、左栏定宽区间、右栏自适应，全部 min-w-0/min-h-0 防溢出 */
    <div
      className={`grid h-full min-h-0 min-w-0 grid-cols-1 overflow-y-auto lg:grid-cols-[minmax(260px,320px)_minmax(0,1fr)] lg:grid-rows-[100%_auto] xl:grid-rows-[minmax(0,1fr)] ${
        isEditing
          ? ''
          : 'xl:grid-cols-[minmax(280px,320px)_minmax(420px,1fr)_minmax(280px,320px)] xl:overflow-hidden'
      }`}
      data-testid="scenes-workspace"
    >
      {/* Left Panel - Timeline */}
      <div
        className="flex h-80 min-h-0 min-w-0 flex-col border-b border-cinema-700 bg-cinema-900/50 lg:h-full lg:border-b-0 lg:border-r"
        data-testid="scenes-timeline-column"
      >
        <div className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
          <StoryTimeline
            scenes={scenes}
            currentSceneId={selectedSceneId}
            characters={characters}
            onSelectScene={handleSelectScene}
            onCreateScene={handleCreateScene}
            onDeleteScene={handleDeleteScene}
            onReorderScenes={handleReorderScenes}
            onEditScene={handleEditScene}
          />
        </div>
      </div>

      {/* Middle Panel - Editor */}
      <div
        className="min-w-0 min-h-0 h-[70dvh] lg:h-full bg-cinema-950"
        data-testid="scenes-preview-column"
      >
        {isEditing && selectedScene ? (
          <div className="h-full p-6">
            <SceneEditor
              scene={selectedScene}
              characters={characters}
              onSave={handleSaveScene}
              onCancel={() => setIsEditing(false)}
            />
          </div>
        ) : selectedScene ? (
          <div className="h-full flex flex-col">
            {/* Preview Tabs */}
            <div className="flex items-center gap-1 px-6 pt-4 pb-2 border-b border-cinema-700">
              <button
                onClick={() => setPreviewTab('content')}
                className={`
                  flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors
                  ${
                    previewTab === 'content'
                      ? 'bg-cinema-gold text-cinema-900'
                      : 'text-gray-400 hover:text-white hover:bg-cinema-800'
                  }
                `}
              >
                <FileText className="w-4 h-4" />
                内容
              </button>
              <button
                onClick={() => setPreviewTab('versions')}
                className={`
                  flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors
                  ${
                    previewTab === 'versions'
                      ? 'bg-cinema-gold text-cinema-900'
                      : 'text-gray-400 hover:text-white hover:bg-cinema-800'
                  }
                `}
              >
                <History className="w-4 h-4" />
                版本历史
              </button>
            </div>

            {previewTab === 'content' ? (
              <>
                {/* Scene Preview */}
                <div className="flex-1 p-8 overflow-auto">
                  <div className="max-w-3xl mx-auto">
                    <h1 className="text-2xl font-bold text-white mb-4">
                      {selectedScene.title || `场景 ${selectedScene.sequence_number}`}
                    </h1>

                    {/* Scene Meta */}
                    <div className="flex flex-wrap gap-2 mb-6">
                      {selectedScene.conflict_type && (
                        <span className="px-3 py-1 text-sm rounded-full bg-cinema-800 text-gray-300">
                          冲突: {selectedScene.conflict_type}
                        </span>
                      )}
                      {selectedScene.setting_location && (
                        <span className="px-3 py-1 text-sm rounded-full bg-cinema-800 text-gray-300">
                          地点: {selectedScene.setting_location}
                        </span>
                      )}
                      {selectedScene.characters_present.length > 0 && (
                        <span className="px-3 py-1 text-sm rounded-full bg-cinema-800 text-gray-300">
                          {selectedScene.characters_present.length} 个角色
                        </span>
                      )}
                      {selectedScene.foreshadowing_ids &&
                        selectedScene.foreshadowing_ids.length > 0 && (
                          <span className="px-3 py-1 text-sm rounded-full bg-purple-500/20 text-purple-300">
                            伏笔: {selectedScene.foreshadowing_ids.length}
                          </span>
                        )}
                    </div>

                    {/* Characters present */}
                    {selectedScene.characters_present.length > 0 && (
                      <div className="flex items-center gap-2 mb-4">
                        <span className="text-xs text-gray-500">出场角色:</span>
                        <div className="flex flex-wrap gap-2">
                          {selectedScene.characters_present.map(charId => {
                            const char = characters.find(c => c.id === charId);
                            return (
                              <span
                                key={charId}
                                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-cinema-800 text-xs text-gray-300"
                              >
                                <span className="w-4 h-4 rounded-full bg-cinema-700 flex items-center justify-center text-[10px]">
                                  {char?.name?.charAt(0) || '?'}
                                </span>
                                {char?.name || '未知角色'}
                              </span>
                            );
                          })}
                        </div>
                      </div>
                    )}

                    {/* Drama Info */}
                    {(selectedScene.dramatic_goal || selectedScene.external_pressure) && (
                      <div className="mb-6 grid grid-cols-1 gap-4 sm:grid-cols-2">
                        {selectedScene.dramatic_goal && (
                          <div className="p-4 bg-cinema-800/50 rounded-lg">
                            <h3 className="text-sm font-medium text-cinema-gold mb-2">戏剧目标</h3>
                            <p className="text-sm text-gray-300">{selectedScene.dramatic_goal}</p>
                          </div>
                        )}
                        {selectedScene.external_pressure && (
                          <div className="p-4 bg-cinema-800/50 rounded-lg">
                            <h3 className="text-sm font-medium text-cinema-gold mb-2">外部压迫</h3>
                            <p className="text-sm text-gray-300">
                              {selectedScene.external_pressure}
                            </p>
                          </div>
                        )}
                      </div>
                    )}

                    {/* Content */}
                    {selectedScene.content ? (
                      <div className="prose prose-invert max-w-none">
                        <div className="whitespace-pre-wrap text-gray-200 leading-relaxed font-serif">
                          {selectedScene.content}
                        </div>
                      </div>
                    ) : (
                      <div className="text-center py-12">
                        <AlertCircle className="w-12 h-12 text-cinema-700 mx-auto mb-4" />
                        <p className="text-gray-500 mb-4">这个场景还没有内容</p>
                        <Button variant="primary" onClick={() => setIsEditing(true)}>
                          <Plus className="w-4 h-4 mr-2" />
                          开始写作
                        </Button>
                      </div>
                    )}
                  </div>
                </div>

                {/* Action Bar */}
                <div className="p-4 border-t border-cinema-700 bg-cinema-900">
                  <div className="flex justify-center">
                    <Button variant="primary" onClick={() => setIsEditing(true)}>
                      编辑场景
                    </Button>
                  </div>
                </div>
              </>
            ) : (
              <div className="flex-1 p-6 overflow-hidden">
                <VersionTimeline
                  sceneId={selectedScene.id}
                  storyId={currentStory.id}
                  onCompare={(v1, v2) => setCompareVersions([v1, v2])}
                />
              </div>
            )}

            {/* Diff Comparison Modal */}
            {compareVersions && (
              <div className="absolute inset-0 bg-cinema-950/90 z-50 flex items-center justify-center p-8">
                <div className="w-full max-w-5xl h-full max-h-[80vh] flex flex-col">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-lg font-semibold text-white">版本对比</h3>
                    <Button variant="ghost" size="sm" onClick={() => setCompareVersions(null)}>
                      关闭
                    </Button>
                  </div>
                  <div className="flex-1 overflow-hidden">
                    <DiffViewer
                      oldContent={compareVersions[0].content || ''}
                      newContent={compareVersions[1].content || ''}
                      oldLabel={`${compareVersions[0].change_summary || '旧版本'} (v${compareVersions[0].version_number})`}
                      newLabel={`${compareVersions[1].change_summary || '新版本'} (v${compareVersions[1].version_number})`}
                      fromVersionId={compareVersions[0].id}
                      toVersionId={compareVersions[1].id}
                      className="h-full"
                    />
                  </div>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="h-full flex flex-col items-center justify-center text-center p-8">
            <BookOpen className="w-16 h-16 text-cinema-700 mb-4" />
            <h2 className="font-display text-xl font-semibold text-white mb-2">选择一个场景</h2>
            <p className="text-gray-500 max-w-md">从左侧选择一个场景查看详情，或创建新场景</p>
          </div>
        )}
      </div>

      {/* Right Panel - Execution preview only (pipeline lives inside SceneEditor when editing) */}
      {!isEditing && (
        <div
          className="h-auto w-full min-w-0 lg:col-span-2 xl:col-span-1 xl:h-full xl:min-h-0"
          data-testid="scenes-execution-column"
        >
          <ExecutionPanel
            storyId={currentStory.id}
            className="h-auto border-l-0 border-t border-cinema-800 xl:h-full xl:border-l xl:border-t-0"
            onCreateScene={handleCreateScene}
            onEditScene={sceneId => {
              const scene = scenes.find(s => s.id === sceneId);
              if (scene) handleEditScene(scene);
            }}
          />
        </div>
      )}
    </div>
  );
}
