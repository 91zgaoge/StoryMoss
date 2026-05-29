import { useState, useRef, useEffect } from 'react';
import {
  Plus,
  BookOpen,
  Download,
  Trash2,
  Edit3,
  ArrowRight,
  Check,
  X,
  FolderOpen,
  Sparkles,
  Loader2,
  Palette,
  ChevronDown,
  Wand2,
  Eye,
  Users,
  FileText,
  LayoutList,
  History,
  RotateCcw,
  GitMerge,
} from 'lucide-react';
import { useWorkflowProgress } from '@/hooks/useWorkflowProgress';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useStories, useCreateStory, useDeleteStory, useUpdateStory } from '@/hooks/useStories';
import { useCharacters } from '@/hooks/useCharacters';
import { useScenes } from '@/hooks/useScenes';
import { useForeshadowings } from '@/hooks/useForeshadowings';
import { useStoryOutline } from '@/hooks/useStoryOutline';
import { useAppStore } from '@/stores/appStore';
import { ExportDialog } from '@/components/ExportDialog';
import { formatDate, truncateText } from '@/utils/format';
import type { Story } from '@/types/index';
import toast from 'react-hot-toast';
import {
  runCreationWorkflow,
  createStoryWithWizard,
  listStyleDnas,
  setStoryStyleDna,
  analyzeStyleSample,
  getStoryStyleBlend,
  setStoryStyleBlend,
} from '@/services/tauri';
import { NovelCreationWizard } from '@/components/NovelCreationWizard';
import { StyleBlendPanel } from '@/components/style/StyleBlendPanel';
import type { StyleBlendConfig } from '@/types/index';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useAiOperations, useRollbackOperation } from '@/hooks/useAiOperations';

function StoryOverview({ storyId, isOpen }: { storyId: string; isOpen: boolean }) {
  const { data: outline } = useStoryOutline(storyId);
  const { data: characters = [] } = useCharacters(storyId);
  const { data: scenes = [] } = useScenes(storyId);
  const { data: foreshadowings = [] } = useForeshadowings(storyId);
  const { data: operations = [] } = useAiOperations(storyId);
  const rollback = useRollbackOperation();

  if (!isOpen) return null;

  return (
    <div className="mt-4 pt-4 border-t border-cinema-700 space-y-4 animate-fade-in">
      {/* Outline */}
      {outline?.content && (
        <div>
          <h4 className="text-sm font-medium text-cinema-gold mb-2 flex items-center gap-1.5">
            <FileText className="w-3.5 h-3.5" />
            故事大纲
          </h4>
          <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
            <p className="text-sm text-gray-300 whitespace-pre-wrap">{outline.content}</p>
            {outline.act_count > 0 && (
              <p className="mt-2 text-xs text-gray-500">
                {outline.act_count} 幕
                {outline.total_scenes_estimate
                  ? ` · 预计 ${outline.total_scenes_estimate} 个场景`
                  : ''}
              </p>
            )}
          </div>
        </div>
      )}

      {/* Stats Grid */}
      <div className="grid grid-cols-3 gap-3">
        <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
          <div className="flex items-center gap-1.5 text-gray-400 mb-1">
            <Users className="w-3.5 h-3.5" />
            <span className="text-xs">角色</span>
          </div>
          <p className="text-lg font-bold text-white">{characters.length}</p>
          {characters.length > 0 && (
            <div className="mt-1 flex -space-x-1">
              {characters.slice(0, 4).map(c => (
                <div
                  key={c.id}
                  className="w-5 h-5 rounded-full bg-cinema-700 border border-cinema-800 flex items-center justify-center text-[9px] text-gray-300"
                  title={c.name}
                >
                  {c.name.charAt(0)}
                </div>
              ))}
              {characters.length > 4 && (
                <div className="w-5 h-5 rounded-full bg-cinema-800 border border-cinema-900 flex items-center justify-center text-[9px] text-gray-400">
                  +{characters.length - 4}
                </div>
              )}
            </div>
          )}
        </div>

        <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
          <div className="flex items-center gap-1.5 text-gray-400 mb-1">
            <LayoutList className="w-3.5 h-3.5" />
            <span className="text-xs">场景</span>
          </div>
          <p className="text-lg font-bold text-white">{scenes.length}</p>
          {scenes.length > 0 && (
            <div className="mt-1 space-y-0.5">
              {scenes.slice(0, 3).map(s => (
                <p key={s.id} className="text-[10px] text-gray-600 truncate">
                  #{s.sequence_number} {s.title || `场景 ${s.sequence_number}`}
                </p>
              ))}
              {scenes.length > 3 && (
                <p className="text-[10px] text-gray-600">+{scenes.length - 3} 更多</p>
              )}
            </div>
          )}
        </div>

        <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
          <div className="flex items-center gap-1.5 text-gray-400 mb-1">
            <Eye className="w-3.5 h-3.5" />
            <span className="text-xs">伏笔</span>
          </div>
          <p className="text-lg font-bold text-white">{foreshadowings.length}</p>
          {foreshadowings.length > 0 && (
            <div className="mt-1 flex flex-wrap gap-1">
              {foreshadowings.slice(0, 3).map(f => (
                <span
                  key={f.id}
                  className={`text-[9px] px-1.5 py-0.5 rounded ${
                    f.status === 'setup'
                      ? 'bg-yellow-500/10 text-yellow-400'
                      : f.status === 'payoff'
                        ? 'bg-green-500/10 text-green-400'
                        : 'bg-gray-500/10 text-gray-400'
                  }`}
                >
                  {f.status === 'setup' ? '未收' : f.status === 'payoff' ? '已收' : '放弃'}
                </span>
              ))}
              {foreshadowings.length > 3 && (
                <span className="text-[9px] text-gray-600">+{foreshadowings.length - 3}</span>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Pipeline Progress */}
      {scenes.length > 0 && (
        <div>
          <h4 className="text-sm font-medium text-cinema-gold mb-2 flex items-center gap-1.5">
            <GitMerge className="w-3.5 h-3.5" />
            创作管线进度
          </h4>
          <div className="space-y-1.5">
            {scenes.map(scene => {
              const stage = scene.execution_stage || 'planning';
              const stageColors: Record<string, string> = {
                planning: 'bg-gray-500/20 text-gray-400',
                outline: 'bg-blue-500/20 text-blue-400',
                drafting: 'bg-purple-500/20 text-purple-400',
                review: 'bg-amber-500/20 text-amber-400',
                final: 'bg-green-500/20 text-green-400',
              };
              const stageLabels: Record<string, string> = {
                planning: '策划',
                outline: '大纲',
                drafting: '草稿',
                review: '审稿',
                final: '定稿',
              };
              return (
                <div key={scene.id} className="flex items-center gap-2 text-xs">
                  <span className="text-gray-500 w-8">#{scene.sequence_number}</span>
                  <span className="text-white/70 flex-1 truncate">
                    {scene.title || `场景 ${scene.sequence_number}`}
                  </span>
                  <span
                    className={`text-[10px] px-1.5 py-0.5 rounded ${stageColors[stage] || stageColors.planning}`}
                  >
                    {stageLabels[stage] || stage}
                  </span>
                </div>
              );
            })}
          </div>
          <div className="mt-2 h-1.5 bg-cinema-800 rounded-full overflow-hidden flex">
            {scenes.map(scene => {
              const stage = scene.execution_stage || 'planning';
              const stageColor =
                stage === 'final'
                  ? 'bg-green-400'
                  : stage === 'review'
                    ? 'bg-amber-400'
                    : stage === 'drafting'
                      ? 'bg-purple-400'
                      : stage === 'outline'
                        ? 'bg-blue-400'
                        : 'bg-gray-600';
              return (
                <div
                  key={scene.id}
                  className={`h-full ${stageColor}`}
                  style={{ width: `${100 / scenes.length}%` }}
                  title={`场景 ${scene.sequence_number}: ${scene.title || ''} (${stage})`}
                />
              );
            })}
          </div>
        </div>
      )}

      {/* AI Operation History */}
      {operations.length > 0 && (
        <div>
          <h4 className="text-sm font-medium text-cinema-gold mb-2 flex items-center gap-1.5">
            <History className="w-3.5 h-3.5" />
            AI 操作历史
          </h4>
          <div className="space-y-2">
            {operations.slice(0, 5).map(op => (
              <div
                key={op.id}
                className={`p-2.5 rounded-lg border text-xs ${
                  op.status === 'rolled_back'
                    ? 'bg-gray-900/30 border-gray-800 opacity-50'
                    : 'bg-cinema-900/50 border-cinema-800'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <span
                      className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
                        op.operation_type === 'bootstrap'
                          ? 'bg-green-400'
                          : op.operation_type === 'smart_execute'
                            ? 'bg-blue-400'
                            : 'bg-gray-400'
                      }`}
                    />
                    <span className="font-medium text-gray-300 truncate">{op.operation_name}</span>
                  </div>
                  <span className="text-gray-600 flex-shrink-0">{formatDate(op.created_at)}</span>
                </div>
                {op.input_summary && (
                  <p className="mt-1 text-gray-500 truncate">输入: {op.input_summary}</p>
                )}
                {op.output_summary && (
                  <p className="mt-0.5 text-gray-500 truncate">输出: {op.output_summary}</p>
                )}
                {op.status === 'success' && op.previous_content !== undefined && (
                  <button
                    onClick={() => rollback.mutate(op.id)}
                    disabled={rollback.isPending}
                    className="mt-1.5 flex items-center gap-1 text-cinema-gold hover:text-cinema-gold/80 transition-colors disabled:opacity-50"
                  >
                    <RotateCcw className="w-3 h-3" />
                    回滚
                  </button>
                )}
                {op.status === 'rolled_back' && (
                  <span className="mt-1 inline-block text-gray-600">已回滚</span>
                )}
              </div>
            ))}
            {operations.length > 5 && (
              <p className="text-xs text-gray-600 text-center">+{operations.length - 5} 更多操作</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export function Stories() {
  const { data: stories = [], isLoading } = useStories();
  const createStory = useCreateStory();
  const deleteStory = useDeleteStory();
  const updateStory = useUpdateStory();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [exportStory, setExportStory] = useState<{ id: string; title: string } | null>(null);
  const [editingStory, setEditingStory] = useState<Story | null>(null);
  const [editForm, setEditForm] = useState({
    title: '',
    description: '',
    genre: '',
    methodology_id: '',
    methodology_step: 1,
  });
  const [creatingStoryId, setCreatingStoryId] = useState<string | null>(null);
  const [styleDnaModalStory, setStyleDnaModalStory] = useState<Story | null>(null);
  const [showStyleSampleInput, setShowStyleSampleInput] = useState(false);
  const [styleSampleText, setStyleSampleText] = useState('');
  const [isAnalyzingStyle, setIsAnalyzingStyle] = useState(false);
  const [showBlendPanel, setShowBlendPanel] = useState(false);
  const [currentBlend, setCurrentBlend] = useState<StyleBlendConfig | undefined>(undefined);
  const [blendTab, setBlendTab] = useState<'single' | 'blend'>('single');
  const queryClient = useQueryClient();
  const [creationMode, setCreationMode] = useState<
    'ai_only' | 'ai_draft_human_edit' | 'human_draft_ai_polish'
  >('ai_only');
  const {
    progress,
    isActive: isWorkflowActive,
    startListening,
    stopListening,
  } = useWorkflowProgress();
  const [showAiMenu, setShowAiMenu] = useState<string | null>(null);
  const aiMenuRef = useRef<HTMLDivElement>(null);
  const [highlightedStoryId, setHighlightedStoryId] = useState<string | null>(null);
  const [openOverviewStoryId, setOpenOverviewStoryId] = useState<string | null>(null);
  const [isWizardOpen, setIsWizardOpen] = useState(false);
  const [wizardStory, setWizardStory] = useState<Story | null>(null);
  const [isWizardCreating, setIsWizardCreating] = useState(false);

  // W2-F2: 替代 backstage-navigate-to-story DOM CustomEvent，改用 Zustand store
  const navigateHighlightStoryId = useAppStore(state => state.navigateHighlightStoryId);
  const setNavigateHighlightStoryId = useAppStore(state => state.setNavigateHighlightStoryId);
  useEffect(() => {
    if (navigateHighlightStoryId) {
      setHighlightedStoryId(navigateHighlightStoryId);
      setOpenOverviewStoryId(navigateHighlightStoryId);
      setNavigateHighlightStoryId(null); // 消费掉事件
      const timer = setTimeout(() => setHighlightedStoryId(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [navigateHighlightStoryId, setNavigateHighlightStoryId]);

  const { data: styleDnas = [] } = useQuery({
    queryKey: ['style-dnas'],
    queryFn: listStyleDnas,
    staleTime: 5 * 60 * 1000,
  });

  // 加载当前故事的混合配置
  useEffect(() => {
    if (styleDnaModalStory && blendTab === 'blend') {
      getStoryStyleBlend(styleDnaModalStory.id)
        .then(result => {
          if (result?.blend) {
            setCurrentBlend(result.blend);
          } else {
            setCurrentBlend(undefined);
          }
        })
        .catch(() => setCurrentBlend(undefined));
    }
  }, [styleDnaModalStory, blendTab]);

  const currentStory = useAppStore(s => s.currentStory);
  const setCurrentStory = useAppStore(s => s.setCurrentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);

  const handleCreate = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const form = e.currentTarget;
    const formData = new FormData(form);

    createStory.mutate(
      {
        title: formData.get('title') as string,
        description: formData.get('description') as string,
        genre: formData.get('genre') as string,
      },
      {
        onSuccess: () => {
          setIsModalOpen(false);
          form.reset();
        },
      }
    );
  };

  const handleSelectStory = (story: Story) => {
    setCurrentStory(story);
    toast.success(`已选择 "${story.title}"`);
  };

  const handleContinueStory = (story: Story) => {
    setCurrentStory(story);
    setCurrentView('scenes');
  };

  const handleEditClick = (story: Story, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingStory(story);
    setEditForm({
      title: story.title,
      description: story.description || '',
      genre: story.genre || '',
      methodology_id: story.methodology_id || '',
      methodology_step: story.methodology_step || 1,
    });
  };

  const handleEditSave = () => {
    if (!editingStory) return;

    updateStory.mutate(
      {
        id: editingStory.id,
        updates: {
          title: editForm.title,
          description: editForm.description || undefined,
          genre: editForm.genre || undefined,
          methodology_id: editForm.methodology_id || undefined,
          methodology_step: editForm.methodology_id ? editForm.methodology_step : undefined,
        },
      },
      {
        onSuccess: () => {
          setEditingStory(null);
        },
      }
    );
  };

  const handleEditCancel = () => {
    setEditingStory(null);
    setEditForm({ title: '', description: '', genre: '', methodology_id: '', methodology_step: 1 });
  };

  const handleDelete = (storyId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirm('确定要删除这个故事吗？此操作不可撤销。')) {
      deleteStory.mutate(storyId);
      if (currentStory?.id === storyId) {
        setCurrentStory(null);
      }
    }
  };

  const handleQuickCreate = async (story: Story, e: React.MouseEvent) => {
    e.stopPropagation();
    setShowAiMenu(null);
    setCreatingStoryId(story.id);
    startListening();
    try {
      const result = await runCreationWorkflow(
        story.id,
        creationMode,
        story.description || story.title
      );
      if (result.success) {
        if (creationMode === 'ai_draft_human_edit') {
          toast.success(
            `AI 初稿已完成！请在幕前编辑后继续。已完成 ${result.completed_phases.length} 个阶段`
          );
        } else if (creationMode === 'human_draft_ai_polish') {
          toast.success(`AI 润色完成！已完成 ${result.completed_phases.length} 个阶段`);
        } else {
          toast.success(`一键创作完成！已完成 ${result.completed_phases.length} 个阶段`);
        }
        // 创作完成后立即跳转到 Scene 编辑器（统一流水线：Scene 为唯一提交粒度）
        setCurrentStory(story);
        setCurrentView('scenes');
      } else {
        if (creationMode === 'ai_draft_human_edit' && result.current_phase === '写作') {
          toast.success(`AI 初稿已生成，请切换到幕前编辑`);
          setCurrentStory(story);
          setCurrentView('scenes');
        } else {
          toast.error(`创作未完成: ${result.error || '未知错误'}`);
        }
      }
    } catch (err: any) {
      toast.error(`创作失败: ${err?.message || String(err)}`);
    } finally {
      setCreatingStoryId(null);
      stopListening();
    }
  };

  const handleWizardCreate = (story: Story, e: React.MouseEvent) => {
    e.stopPropagation();
    setShowAiMenu(null);
    setWizardStory(story);
    setIsWizardOpen(true);
  };

  const handleWizardComplete = async (data: {
    worldBuilding: import('@/types/v3').WorldBuildingOption;
    characters: import('@/types/v3').CharacterProfileOption[];
    writingStyle: import('@/types/v3').WritingStyleOption;
    firstScene: import('@/types/v3').SceneProposal;
    genreInput: string;
  }) => {
    if (!wizardStory) return;
    setIsWizardCreating(true);
    try {
      const result = await createStoryWithWizard({
        title: wizardStory.title || data.writingStyle.name || '未命名作品',
        description: wizardStory.description || data.genreInput,
        genre: wizardStory.genre,
        world_building: data.worldBuilding,
        characters: data.characters,
        writing_style: data.writingStyle,
        first_scene: data.firstScene,
      });
      toast.success(`「${result.story.title}」向导创作完成！`);
      setIsWizardOpen(false);
      setWizardStory(null);
      // 刷新故事列表
      queryClient.invalidateQueries({ queryKey: ['stories'] });
      // 导航到新创建的故事
      setCurrentStory(result.story);
      setCurrentView('scenes');
    } catch (error: any) {
      toast.error(`向导创作失败: ${error?.message || String(error)}`);
    } finally {
      setIsWizardCreating(false);
    }
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <div className="loading-reel" />
      </div>
    );
  }

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">故事库</h1>
          <p className="text-gray-400">管理和创作你的故事</p>
        </div>
        <Button variant="primary" onClick={() => setIsModalOpen(true)}>
          <Plus className="w-4 h-4" />
          新建故事
        </Button>
      </div>

      {/* Current Story Indicator */}
      {currentStory && (
        <div className="p-4 rounded-xl bg-cinema-gold/10 border border-cinema-gold/30 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-cinema-gold/20 flex items-center justify-center">
              <BookOpen className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <p className="text-sm text-cinema-gold">当前编辑</p>
              <p className="font-display font-semibold text-white">{currentStory.title}</p>
            </div>
          </div>
          <Button variant="ghost" size="sm" onClick={() => setCurrentView('scenes')}>
            继续创作
            <ArrowRight className="w-4 h-4 ml-1" />
          </Button>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {stories.map(story => {
          const isHighlighted = highlightedStoryId === story.id;
          const isOverviewOpen = openOverviewStoryId === story.id;
          return (
            <Card
              key={story.id}
              hover
              className={`group cursor-pointer transition-all ${
                currentStory?.id === story.id ? 'ring-2 ring-cinema-gold/50' : ''
              } ${isHighlighted ? 'ring-2 ring-cinema-gold/70 animate-pulse' : ''}`}
              onClick={() => handleSelectStory(story)}
            >
              <CardContent className="p-6">
                {editingStory?.id === story.id ? (
                  // Edit Mode
                  <div className="space-y-3" onClick={e => e.stopPropagation()}>
                    <input
                      type="text"
                      value={editForm.title}
                      onChange={e => setEditForm({ ...editForm, title: e.target.value })}
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                      placeholder="标题"
                    />
                    <select
                      value={editForm.genre}
                      onChange={e => setEditForm({ ...editForm, genre: e.target.value })}
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                    >
                      <option value="">选择类型</option>
                      <option value="科幻">科幻</option>
                      <option value="奇幻">奇幻</option>
                      <option value="悬疑">悬疑</option>
                      <option value="言情">言情</option>
                      <option value="历史">历史</option>
                      <option value="武侠">武侠</option>
                      <option value="现代">现代</option>
                      <option value="其他">其他</option>
                    </select>
                    <select
                      value={editForm.methodology_id}
                      onChange={e => setEditForm({ ...editForm, methodology_id: e.target.value })}
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                    >
                      <option value="">选择创作方法论（可选）</option>
                      <option value="snowflake">雪花法</option>
                      <option value="scene_structure">场景节拍</option>
                      <option value="hero_journey">英雄之旅</option>
                      <option value="character_depth">人物深度</option>
                      <option value="world_building">高密度世界构建</option>
                    </select>
                    {editForm.methodology_id === 'snowflake' && (
                      <select
                        value={editForm.methodology_step}
                        onChange={e =>
                          setEditForm({ ...editForm, methodology_step: Number(e.target.value) })
                        }
                        className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                      >
                        {Array.from({ length: 10 }, (_, i) => (
                          <option key={i + 1} value={i + 1}>
                            步骤 {i + 1}:{' '}
                            {
                              [
                                '一句话概括',
                                '一段式概括',
                                '人物概述',
                                '一页纸大纲',
                                '人物详细背景',
                                '四页纸大纲',
                                '人物完整档案',
                                '场景清单',
                                '场景扩展',
                                '初稿写作',
                              ][i]
                            }
                          </option>
                        ))}
                      </select>
                    )}
                    {editForm.methodology_id === 'world_building' && (
                      <select
                        value={editForm.methodology_step}
                        onChange={e =>
                          setEditForm({ ...editForm, methodology_step: Number(e.target.value) })
                        }
                        className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                      >
                        {[
                          '1. 最小世界种子',
                          '2. 状态网扩张',
                          '3. 多线交织与回流',
                          '4. 密度迭代与克制',
                        ].map((phase, i) => (
                          <option key={i + 1} value={i + 1}>
                            阶段 {i + 1}: {phase}
                          </option>
                        ))}
                      </select>
                    )}
                    <textarea
                      value={editForm.description}
                      onChange={e => setEditForm({ ...editForm, description: e.target.value })}
                      rows={2}
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none resize-none"
                      placeholder="描述"
                    />
                    <div className="flex gap-2">
                      <Button variant="ghost" size="sm" onClick={handleEditCancel}>
                        <X className="w-4 h-4" />
                      </Button>
                      <Button variant="primary" size="sm" onClick={handleEditSave}>
                        <Check className="w-4 h-4" />
                      </Button>
                    </div>
                  </div>
                ) : (
                  // View Mode
                  <>
                    <div className="flex items-start gap-4">
                      <div className="w-12 h-12 rounded-xl bg-cinema-gold/10 flex items-center justify-center">
                        <BookOpen className="w-6 h-6 text-cinema-gold" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <h3 className="font-display text-lg font-semibold text-white truncate">
                          {story.title}
                        </h3>
                        <p className="text-sm text-gray-400 mt-1">
                          {story.genre || '未分类'} · {story.chapter_count || 0} 章
                        </p>
                        {story.description && (
                          <p className="text-sm text-gray-500 mt-2 line-clamp-2">
                            {truncateText(story.description, 100)}
                          </p>
                        )}
                        <p className="text-xs text-gray-600 mt-3">
                          更新于 {formatDate(story.updated_at)}
                        </p>
                      </div>
                    </div>

                    {/* Overview Toggle */}
                    <button
                      onClick={e => {
                        e.stopPropagation();
                        setOpenOverviewStoryId(isOverviewOpen ? null : story.id);
                      }}
                      className="mt-3 w-full flex items-center justify-center gap-1.5 py-1.5 text-xs text-gray-500 hover:text-cinema-gold transition-colors border border-dashed border-cinema-800 rounded-lg hover:border-cinema-gold/50"
                    >
                      <Eye className="w-3.5 h-3.5" />
                      {isOverviewOpen ? '收起概览' : '概览'}
                    </button>

                    {/* Overview Panel */}
                    <StoryOverview storyId={story.id} isOpen={isOverviewOpen} />

                    <div className="mt-4 pt-4 border-t border-cinema-700 flex flex-wrap gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={e => {
                          e.stopPropagation();
                          handleContinueStory(story);
                        }}
                      >
                        <FolderOpen className="w-4 h-4 mr-1" />
                        打开
                      </Button>
                      <div
                        className="relative"
                        ref={showAiMenu === story.id ? aiMenuRef : undefined}
                      >
                        <Button
                          variant="ghost"
                          size="sm"
                          disabled={creatingStoryId === story.id}
                          onClick={e => {
                            e.stopPropagation();
                            setShowAiMenu(showAiMenu === story.id ? null : story.id);
                          }}
                          title="AI 创作菜单"
                        >
                          {creatingStoryId === story.id ? (
                            <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                          ) : (
                            <Sparkles className="w-4 h-4 mr-1 text-cinema-gold" />
                          )}
                          AI 创作
                          <ChevronDown className="w-3 h-3 ml-1" />
                        </Button>

                        {showAiMenu === story.id && (
                          <div className="absolute right-0 mt-1 w-56 bg-cinema-800 border border-cinema-700 rounded-xl shadow-xl z-50 overflow-hidden">
                            <div className="p-2 space-y-1">
                              <select
                                value={creationMode}
                                onChange={e =>
                                  setCreationMode(e.target.value as typeof creationMode)
                                }
                                onClick={e => e.stopPropagation()}
                                className="w-full px-2 py-1.5 bg-cinema-900 border border-cinema-700 rounded-lg text-xs text-white focus:border-cinema-gold focus:outline-none"
                                title="选择创作模式"
                              >
                                <option value="ai_only">AI 全自动</option>
                                <option value="ai_draft_human_edit">AI 初稿 + 我精修</option>
                                <option value="human_draft_ai_polish">我初稿 + AI 润色</option>
                              </select>
                              <button
                                onClick={e => handleQuickCreate(story, e)}
                                className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-gray-300 hover:bg-cinema-700 transition-colors text-left"
                              >
                                <Sparkles className="w-4 h-4 text-cinema-gold" />
                                <div>
                                  <div className="text-white text-sm">快速创作</div>
                                  <div className="text-[10px] text-gray-500">
                                    {creationMode === 'ai_only'
                                      ? 'AI 全自动生成'
                                      : creationMode === 'ai_draft_human_edit'
                                        ? 'AI 初稿 + 我精修'
                                        : '我初稿 + AI 润色'}
                                  </div>
                                </div>
                              </button>
                              <button
                                onClick={e => handleWizardCreate(story, e)}
                                className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-gray-300 hover:bg-cinema-700 transition-colors text-left"
                              >
                                <Wand2 className="w-4 h-4 text-cinema-gold" />
                                <div>
                                  <div className="text-white text-sm">向导创作</div>
                                  <div className="text-[10px] text-gray-500">
                                    分步选择 AI 生成选项
                                  </div>
                                </div>
                              </button>
                            </div>
                          </div>
                        )}
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={e => {
                          e.stopPropagation();
                          setStyleDnaModalStory(story);
                        }}
                        title="选择写作风格 DNA"
                      >
                        <Palette className="w-4 h-4 mr-1" />
                        风格
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={e => {
                          e.stopPropagation();
                          setExportStory({ id: story.id, title: story.title });
                        }}
                      >
                        <Download className="w-4 h-4 mr-1" />
                        导出
                      </Button>
                      <Button variant="ghost" size="sm" onClick={e => handleEditClick(story, e)}>
                        <Edit3 className="w-4 h-4 mr-1" />
                        编辑
                      </Button>
                      <Button variant="danger" size="sm" onClick={e => handleDelete(story.id, e)}>
                        <Trash2 className="w-4 h-4 mr-1" />
                        删除
                      </Button>
                    </div>
                  </>
                )}
              </CardContent>
            </Card>
          );
        })}

        {stories.length === 0 && (
          <div className="col-span-full">
            <Card className="py-12">
              <CardContent className="text-center">
                <BookOpen className="w-16 h-16 text-cinema-700 mx-auto mb-4" />
                <h3 className="font-display text-xl font-semibold text-white mb-2">
                  开始你的创作之旅
                </h3>
                <p className="text-gray-500 max-w-md mx-auto mb-6">
                  你还没有创建任何故事。点击"新建故事"开始创作吧！
                </p>
                <Button variant="primary" onClick={() => setIsModalOpen(true)}>
                  <Plus className="w-4 h-4 mr-2" />
                  创建第一个故事
                </Button>
              </CardContent>
            </Card>
          </div>
        )}
      </div>

      {/* Create Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-md mx-4">
            <CardContent className="p-6">
              <h2 className="font-display text-xl font-bold text-white mb-4">新建故事</h2>

              <form onSubmit={handleCreate} className="space-y-4">
                <div>
                  <label className="block text-sm text-gray-400 mb-1">标题</label>
                  <input
                    name="title"
                    required
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  />
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">类型</label>
                  <select
                    name="genre"
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  >
                    <option value="">选择类型</option>
                    <option value="科幻">科幻</option>
                    <option value="奇幻">奇幻</option>
                    <option value="悬疑">悬疑</option>
                    <option value="言情">言情</option>
                    <option value="历史">历史</option>
                  </select>
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">描述</label>
                  <textarea
                    name="description"
                    rows={3}
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                  />
                </div>

                <div className="flex gap-3 pt-4">
                  <Button type="button" variant="ghost" onClick={() => setIsModalOpen(false)}>
                    取消
                  </Button>
                  <Button type="submit" variant="primary" isLoading={createStory.isPending}>
                    创建
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Export Dialog */}
      {exportStory && (
        <ExportDialog
          storyId={exportStory.id}
          storyTitle={exportStory.title}
          isOpen={!!exportStory}
          onClose={() => setExportStory(null)}
        />
      )}

      {/* Workflow Progress Modal */}
      {isWorkflowActive && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <Card className="w-full max-w-md mx-4">
            <CardContent className="p-6 space-y-4">
              <div className="flex items-center gap-3">
                <Loader2 className="w-6 h-6 text-cinema-gold animate-spin" />
                <div>
                  <h2 className="font-display text-lg font-bold text-white">AI 一键创作中</h2>
                  <p className="text-sm text-gray-400">{progress?.message || '准备中...'}</p>
                </div>
              </div>

              {/* Progress Bar */}
              <div className="space-y-2">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-cinema-gold font-medium">{progress?.phase || '启动'}</span>
                  <span className="text-gray-400 font-mono">
                    {Math.round((progress?.progress ?? 0) * 100)}%
                  </span>
                </div>
                <div className="h-2 bg-cinema-800 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-cinema-gold rounded-full transition-all duration-500"
                    style={{ width: `${Math.round((progress?.progress ?? 0) * 100)}%` }}
                  />
                </div>
              </div>

              {/* Phase Indicators */}
              <div className="flex items-center justify-between text-xs">
                {['构思', '大纲', '写作', '审阅', '入库'].map((phase, idx) => {
                  const thresholds = [0.0, 0.15, 0.5, 0.7, 1.0];
                  const currentProgress = progress?.progress ?? 0;
                  const isActive =
                    currentProgress >= thresholds[idx] &&
                    (idx === 4 || currentProgress < thresholds[idx + 1]);
                  const isDone = currentProgress >= thresholds[idx] + (idx < 4 ? 0.15 : 0);
                  return (
                    <div key={phase} className="flex flex-col items-center gap-1">
                      <div
                        className={`w-2 h-2 rounded-full ${
                          isDone
                            ? 'bg-cinema-gold'
                            : isActive
                              ? 'bg-cinema-gold/60 animate-pulse'
                              : 'bg-cinema-700'
                        }`}
                      />
                      <span className={isDone || isActive ? 'text-cinema-gold' : 'text-gray-600'}>
                        {phase}
                      </span>
                    </div>
                  );
                })}
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* StyleDNA Selector Modal */}
      {styleDnaModalStory && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-xl mx-4">
            <CardContent className="p-6">
              <h2 className="font-display text-xl font-bold text-white mb-2">写作风格配置</h2>
              <p className="text-sm text-gray-400 mb-4">
                为「{styleDnaModalStory.title}」配置风格，AI 创作时将按此风格进行。
              </p>

              {/* 标签页切换 */}
              <div className="flex gap-1 mb-4 p-1 bg-cinema-800 rounded-lg">
                <button
                  onClick={() => setBlendTab('single')}
                  className={`flex-1 py-1.5 px-3 rounded-md text-sm transition-colors ${
                    blendTab === 'single'
                      ? 'bg-cinema-700 text-white'
                      : 'text-gray-400 hover:text-gray-200'
                  }`}
                >
                  单一风格
                </button>
                <button
                  onClick={() => setBlendTab('blend')}
                  className={`flex-1 py-1.5 px-3 rounded-md text-sm transition-colors ${
                    blendTab === 'blend'
                      ? 'bg-cinema-700 text-white'
                      : 'text-gray-400 hover:text-gray-200'
                  }`}
                >
                  风格混合
                </button>
              </div>

              {blendTab === 'single' ? (
                <>
                  <div className="space-y-2 max-h-80 overflow-y-auto">
                    <button
                      onClick={async () => {
                        await setStoryStyleDna(styleDnaModalStory.id, null);
                        setStyleDnaModalStory(null);
                        toast.success('已清除风格设置');
                      }}
                      className={`w-full p-3 rounded-lg text-left transition-colors border ${
                        !styleDnaModalStory.style_dna_id
                          ? 'bg-cinema-gold/20 border-cinema-gold/50'
                          : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                      }`}
                    >
                      <div className="font-medium text-white">默认风格</div>
                      <div className="text-xs text-gray-400">不指定特定风格，使用通用创作风格</div>
                    </button>
                    {styleDnas.map(dna => (
                      <button
                        key={dna.id}
                        onClick={async () => {
                          await setStoryStyleDna(styleDnaModalStory.id, dna.id);
                          setStyleDnaModalStory(null);
                          toast.success(`已设置风格：${dna.name}`);
                        }}
                        className={`w-full p-3 rounded-lg text-left transition-colors border ${
                          styleDnaModalStory.style_dna_id === dna.id
                            ? 'bg-cinema-gold/20 border-cinema-gold/50'
                            : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                        }`}
                      >
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-white">{dna.name}</span>
                          {dna.is_builtin && (
                            <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold">
                              内置
                            </span>
                          )}
                        </div>
                        {dna.author && (
                          <div className="text-xs text-gray-400">作者：{dna.author}</div>
                        )}
                      </button>
                    ))}
                  </div>
                  <div className="flex justify-between mt-4">
                    <Button variant="ghost" size="sm" onClick={() => setShowStyleSampleInput(true)}>
                      <Sparkles className="w-4 h-4 mr-1 text-cinema-gold" />
                      从文本生成风格
                    </Button>
                    <Button variant="ghost" onClick={() => setStyleDnaModalStory(null)}>
                      取消
                    </Button>
                  </div>
                </>
              ) : (
                <StyleBlendPanel
                  storyId={styleDnaModalStory.id}
                  availableDnas={styleDnas}
                  initialBlend={currentBlend}
                  onSave={async blend => {
                    try {
                      const blendJson = JSON.stringify(blend);
                      await setStoryStyleBlend(styleDnaModalStory.id, blend.name, blendJson);
                      setStyleDnaModalStory(null);
                      setCurrentBlend(undefined);
                      toast.success(`风格混合「${blend.name}」已保存`);
                    } catch (err: any) {
                      toast.error(`保存失败: ${err?.message || String(err)}`);
                    }
                  }}
                  onCancel={() => {
                    setStyleDnaModalStory(null);
                    setCurrentBlend(undefined);
                  }}
                />
              )}
            </CardContent>
          </Card>
        </div>
      )}

      {/* Style Sample Input Modal */}
      {showStyleSampleInput && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-lg mx-4">
            <CardContent className="p-6 space-y-4">
              <h2 className="font-display text-xl font-bold text-white">从文本样例生成风格</h2>
              <p className="text-sm text-gray-400">
                粘贴一段你喜欢的文字（300-3000字），AI 将分析其风格特征并生成风格 DNA。
              </p>
              <textarea
                value={styleSampleText}
                onChange={e => setStyleSampleText(e.target.value)}
                rows={8}
                placeholder="在此粘贴文本样例..."
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none resize-none"
              />
              <div className="flex justify-end gap-2">
                <Button
                  variant="ghost"
                  onClick={() => {
                    setShowStyleSampleInput(false);
                    setStyleSampleText('');
                  }}
                >
                  取消
                </Button>
                <Button
                  variant="primary"
                  isLoading={isAnalyzingStyle}
                  disabled={styleSampleText.length < 30}
                  onClick={async () => {
                    setIsAnalyzingStyle(true);
                    try {
                      const result = await analyzeStyleSample(styleSampleText);
                      toast.success(`风格「${result.name}」生成成功！`);
                      queryClient.invalidateQueries({ queryKey: ['style-dnas'] });
                      setShowStyleSampleInput(false);
                      setStyleSampleText('');
                    } catch (err: any) {
                      toast.error(`风格生成失败: ${err?.message || String(err)}`);
                    } finally {
                      setIsAnalyzingStyle(false);
                    }
                  }}
                >
                  生成风格
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Novel Creation Wizard Modal */}
      {isWizardOpen && wizardStory && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in overflow-y-auto py-8">
          <Card className="w-full max-w-3xl mx-4 animate-slide-up my-auto">
            <CardContent className="p-8">
              {isWizardCreating ? (
                <div className="text-center py-12">
                  <div className="relative w-20 h-20 mx-auto mb-6">
                    <div className="absolute inset-0 border-4 border-cinema-700 rounded-full" />
                    <div className="absolute inset-0 border-4 border-cinema-gold rounded-full border-t-transparent animate-spin" />
                    <Sparkles className="absolute inset-0 m-auto w-8 h-8 text-cinema-gold" />
                  </div>
                  <h3 className="text-xl font-semibold text-white mb-2">正在创建故事...</h3>
                  <p className="text-gray-400">保存世界观、角色、文风并自动摄取知识</p>
                </div>
              ) : (
                <NovelCreationWizard
                  onComplete={handleWizardComplete}
                  onCancel={() => {
                    setIsWizardOpen(false);
                    setWizardStory(null);
                  }}
                />
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
