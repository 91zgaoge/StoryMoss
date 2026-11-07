import { useEffect, useRef, useState } from 'react';
import {
  BookOpen,
  Users,
  FileText,
  Sparkles,
  ArrowRight,
  Clock,
  FolderOpen,
  Type,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { Panel } from '@/components/ui/Panel';
import { StudioNavRail } from '@/components/ui/StudioNavRail';
import { useAppStore } from '@/stores/appStore';
import { useStories, useCreateStory, useUpdateStory } from '@/hooks/useStories';
import { createStoryWithWizard, loggedInvoke } from '@/services/tauri';
import { NovelCreationWizard } from '@/components/NovelCreationWizard';
import { AgencyRunsPanel } from '@/components/AgencyRunsPanel';
import { CreationPathGuide } from '@/components/CreationPathGuide';
import {
  StoryFormatFields,
  productionConstraintsJson,
  type StoryFormat,
} from '@/components/StoryFormatFields';
import { formatNumber, formatDate } from '@/utils/format';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const dashboardLogger = createLogger('ui:Dashboard');

export function Dashboard() {
  const stories = useAppStore(s => s.stories);
  const setStories = useAppStore(s => s.setStories);
  const setCurrentUser = useAppStore(s => s.setCurrentUser);
  const setCurrentStory = useAppStore(s => s.setCurrentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);
  const isLoading = useAppStore(s => s.isLoading);
  const hasHydrated = useRef(false);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [createFormat, setCreateFormat] = useState<StoryFormat>('novel');
  const [isWizardOpen, setIsWizardOpen] = useState(false);
  const [isCreating, setIsCreating] = useState(false);

  const { data: fetchedStories = [], isLoading: isStoriesLoading } = useStories();
  const createStory = useCreateStory();
  const updateStory = useUpdateStory();

  // Sync fetched stories to store
  useEffect(() => {
    if (fetchedStories.length > 0 && stories.length === 0) {
      setStories(fetchedStories);
    }
  }, [fetchedStories, stories.length, setStories]);

  // Initialize current user for collaboration - only once
  useEffect(() => {
    if (hasHydrated.current) return;
    hasHydrated.current = true;

    const userId = localStorage.getItem('user_id') || `user_${Date.now()}`;
    const userName = localStorage.getItem('user_name') || '创作者';
    localStorage.setItem('user_id', userId);
    localStorage.setItem('user_name', userName);
    setCurrentUser({ id: userId, name: userName });
  }, [setCurrentUser]);

  // Prefer fetched stories for stats to avoid store lag
  const statsSource = fetchedStories.length > 0 ? fetchedStories : stories;

  // Calculate totals across all stories
  const totalCharacters = statsSource.reduce((sum, s) => sum + (s.character_count || 0), 0);
  const totalScenes = statsSource.reduce((sum, s) => sum + (s.scene_count || 0), 0);
  const totalWordCount = statsSource.reduce((sum, s) => sum + (s.word_count || 0), 0);

  const stats = [
    {
      label: '故事',
      value: statsSource.length,
      icon: BookOpen,
      color: 'text-cinema-gold',
      view: 'stories' as const,
    },
    {
      label: '角色',
      value: totalCharacters,
      icon: Users,
      color: 'text-cinema-velvet',
      view: 'characters' as const,
    },
    {
      label: '场景',
      value: totalScenes,
      icon: FileText,
      color: 'text-cinema-gold',
      view: 'scenes' as const,
    },
    {
      label: '字数',
      value: totalWordCount,
      icon: Type,
      color: 'text-status-success',
      view: 'scenes' as const,
    },
  ];

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
        onSuccess: newStory => {
          if (createFormat === 'short_drama') {
            updateStory.mutate({
              id: newStory.id,
              updates: {
                story_format: 'short_drama',
                production_constraints: productionConstraintsJson(formData),
              },
            });
          }
          setIsModalOpen(false);
          setCreateFormat('novel');
          form.reset();
          // Auto-select the new story and navigate to chapters
          setCurrentStory(newStory);
          setCurrentView('scenes');
          toast.success(`故事 "${newStory.title}" 创建成功！`);
        },
      }
    );
  };

  const handleWizardComplete = async (data: {
    worldBuilding: import('@/types/v3').WorldBuildingOption;
    characters: import('@/types/v3').CharacterProfileOption[];
    writingStyle: import('@/types/v3').WritingStyleOption;
    firstScene: import('@/types/v3').SceneProposal;
    genreInput: string;
    selectedStrategy?: import('@/types/index').SelectedStrategy;
  }) => {
    setIsCreating(true);
    try {
      const result = await createStoryWithWizard({
        title: data.writingStyle.name || '未命名作品',
        description: data.genreInput,
        genre: data.genreInput,
        style_dna_id: data.selectedStrategy?.style_dna_ids[0],
        genre_profile_id: data.selectedStrategy?.genre_profile_id,
        methodology_id: data.selectedStrategy?.methodology_id,
        world_building: data.worldBuilding,
        characters: data.characters,
        writing_style: data.writingStyle,
        first_scene: data.firstScene,
      });

      setIsWizardOpen(false);
      setStories([...stories, result.story]);
      setCurrentStory(result.story);
      setCurrentView('scenes');
      toast.success(
        `「${result.story.title}」创建成功！已自动摄取 ${result.ingested_entities} 个实体、${result.ingested_relations} 条关系到知识图谱。`
      );
    } catch (error) {
      dashboardLogger.error('Wizard creation failed', { error });
      toast.error('创建失败，请重试');
    } finally {
      setIsCreating(false);
    }
  };

  const handleContinueStory = (story: (typeof stories)[0]) => {
    setCurrentStory(story);
    setCurrentView('scenes');
  };

  const handleOpenFrontstage = async () => {
    try {
      await loggedInvoke<unknown>('show_frontstage');
      toast.success('幕前写作界面已打开');
    } catch (error) {
      dashboardLogger.error('Failed to open frontstage', { error });
      toast.error('打开幕前失败');
    }
  };

  const recentStories = [...statsSource]
    .sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime())
    .slice(0, 3);

  return (
    <div className="flex h-full min-h-0 bg-cinema-950 font-body text-ai-ink">
      {/* 左侧导航轨 */}
      <StudioNavRail activeView="dashboard" />

      {/* 主区 */}
      <main className="flex min-w-0 min-h-0 flex-1 flex-col">
        <header className="flex h-16 shrink-0 items-center justify-between gap-4 border-b border-borderSubtle bg-cinema-900/70 px-5 backdrop-blur-sm sm:px-6">
          <h1 className="font-body text-base font-semibold text-ai-ink">工作室</h1>
          <div className="flex items-center gap-3">
            <span className="hidden font-mono text-xs text-ai-ink-3 sm:inline">MODEL: ONLINE</span>
            <Button variant="primary" size="sm" onClick={() => setIsModalOpen(true)}>
              新建
            </Button>
          </div>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6" data-testid="dashboard-scroll">
          <div className="mx-auto grid max-w-[1440px] grid-cols-1 gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(320px,360px)]">
            {/* 作品概览 */}
            <div className="min-w-0 space-y-5">
              <Panel title="作品概览">
                <div className="space-y-6">
                  {/* Hero */}
                  <div className="relative overflow-hidden rounded-xl border border-borderSubtle bg-cinema-900/55 p-6 sm:p-8">
                    <div className="pointer-events-none absolute right-0 top-0 h-64 w-64 translate-x-1/2 -translate-y-1/2 rounded-full bg-cinema-gold/5 blur-3xl" />

                    <div className="relative z-10">
                      <h2 className="mb-2 font-body text-2xl font-semibold tracking-tight text-ai-ink sm:text-3xl">
                        欢迎回到创作工作室
                      </h2>
                      <p className="text-ai-ink-2 text-base font-body italic max-w-2xl">
                        "每一个伟大的故事，都始于一个勇敢的开始。"
                      </p>
                      <div className="mt-4 flex flex-wrap gap-3">
                        <Button variant="primary" className="gap-2" onClick={handleOpenFrontstage}>
                          <Sparkles className="w-4 h-4" />
                          AI 创建故事
                        </Button>
                        <Button variant="secondary" onClick={() => setIsModalOpen(true)}>
                          手动创建
                        </Button>
                        {recentStories.length > 0 && (
                          <Button variant="ghost" onClick={() => setCurrentView('stories')}>
                            <FolderOpen className="w-4 h-4 mr-2" />
                            打开故事库
                          </Button>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* Stats */}
                  <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
                    {stats.map(stat => {
                      const Icon = stat.icon;
                      return (
                        <button
                          key={stat.label}
                          type="button"
                          onClick={() => setCurrentView(stat.view)}
                          className={cn(
                            'rounded-xl border transition-all duration-300 text-left cursor-pointer',
                            'bg-cinema-850/50 backdrop-blur-xl border-cinema-700/50',
                            'hover:border-cinema-gold/30 hover:shadow-lg hover:shadow-cinema-gold/5',
                            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50'
                          )}
                        >
                          <div className="flex items-center gap-3 p-4">
                            <div className={cn('p-2.5 rounded-xl bg-cinema-800', stat.color)}>
                              <Icon className="w-5 h-5" />
                            </div>
                            <div>
                              <p className="text-2xl font-display font-bold text-ai-ink">
                                {formatNumber(stat.value)}
                              </p>
                              <p className="text-xs text-ai-ink-2">{stat.label}</p>
                            </div>
                          </div>
                        </button>
                      );
                    })}
                  </div>

                  {/* Recent Stories */}
                  {!isStoriesLoading && recentStories.length > 0 && (
                    <div className="space-y-3">
                      <div className="flex items-center justify-between">
                        <h3 className="font-display text-sm font-semibold text-ai-ink flex items-center gap-2">
                          <Clock className="w-4 h-4 text-cinema-gold" />
                          最近编辑
                        </h3>
                        <Button variant="ghost" size="sm" onClick={() => setCurrentView('stories')}>
                          查看全部
                          <ArrowRight className="w-4 h-4 ml-1" />
                        </Button>
                      </div>

                      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3">
                        {recentStories.map(story => (
                          <button
                            key={story.id}
                            type="button"
                            onClick={() => handleContinueStory(story)}
                            className={cn(
                              'rounded-xl border transition-all duration-300 text-left cursor-pointer group',
                              'bg-cinema-850/50 backdrop-blur-xl border-cinema-700/50',
                              'hover:border-cinema-gold/30 hover:shadow-lg hover:shadow-cinema-gold/5',
                              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50'
                            )}
                          >
                            <div className="p-4">
                              <div className="flex items-start gap-3">
                                <div className="w-10 h-10 rounded-lg bg-cinema-gold/10 flex items-center justify-center flex-shrink-0">
                                  <BookOpen className="w-5 h-5 text-cinema-gold" />
                                </div>
                                <div className="flex-1 min-w-0">
                                  <h4 className="font-display font-semibold text-ai-ink truncate group-hover:text-cinema-gold transition-colors">
                                    {story.title}
                                  </h4>
                                  <p className="text-xs text-cinema-400 mt-1">
                                    {story.genre || '未分类'} · {story.chapter_count || 0} 章
                                    {story.word_count ? ` · ${story.word_count} 字` : ''}
                                  </p>
                                  <p className="mt-2 text-xs text-ai-ink-3">
                                    更新于 {formatDate(story.updated_at)}
                                  </p>
                                </div>
                              </div>
                            </div>
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Empty State */}
                  {!isStoriesLoading && stories.length === 0 && (
                    <Card className="py-10">
                      <CardContent className="text-center">
                        <BookOpen className="w-14 h-14 text-cinema-700 mx-auto mb-4" />
                        <h3 className="font-display text-lg font-semibold text-ai-ink mb-2">
                          开始你的创作之旅
                        </h3>
                        <p className="text-cinema-400 max-w-md mx-auto mb-6 text-sm">
                          使用 AI
                          向导创建一个新故事，或者导入已有的创作。草苔将帮助你管理角色、章节，并提供
                          AI 辅助写作。
                        </p>
                        <div className="flex justify-center gap-4">
                          <Button variant="primary" onClick={handleOpenFrontstage}>
                            <Sparkles className="w-4 h-4 mr-2" />
                            AI 创建第一个故事
                          </Button>
                          <Button variant="secondary" onClick={() => setIsModalOpen(true)}>
                            手动创建
                          </Button>
                        </div>
                      </CardContent>
                    </Card>
                  )}

                  {/* Loading State */}
                  {(isLoading || isStoriesLoading) && (
                    <div className="flex items-center justify-center py-12">
                      <div className="loading-reel" />
                    </div>
                  )}
                </div>
              </Panel>
            </div>

            {/* 创作路径 */}
            <div className="min-w-0">
              <Panel title="创作路径">
                <CreationPathGuide
                  onFrontstage={handleOpenFrontstage}
                  onWizard={() => setIsWizardOpen(true)}
                  onQuick={() => {
                    useAppStore.getState().setPendingQuickCreate(true);
                    setCurrentView('stories');
                  }}
                />
              </Panel>
            </div>

            {/* 代理运行（v0.86.0：读 agency_runs，替代废弃 genesis_runs 面板） */}
            <div className="min-w-0 xl:col-span-2">
              <Panel title="代理运行" collapsible defaultOpen>
                <AgencyRunsPanel />
              </Panel>
            </div>
          </div>
        </div>
      </main>

      {/* Create Modal */}
      {isModalOpen && (
        <Modal open onClose={() => setIsModalOpen(false)} title="新建故事" size="sm">
          <form onSubmit={handleCreate} className="space-y-4">
            <div>
              <label className="block text-sm text-ai-ink-2 mb-1">标题 *</label>
              <input
                name="title"
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-ai-ink focus:border-cinema-gold focus:outline-none"
                placeholder="给你的故事起个名字"
              />
            </div>

            <div>
              <label className="block text-sm text-ai-ink-2 mb-1">类型</label>
              <select
                name="genre"
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-ai-ink focus:border-cinema-gold focus:outline-none"
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
            </div>

            <div>
              <label className="block text-sm text-ai-ink-2 mb-1">描述</label>
              <textarea
                name="description"
                rows={3}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-ai-ink focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="简要描述一下你的故事..."
              />
            </div>

            <StoryFormatFields format={createFormat} onFormatChange={setCreateFormat} />

            <div className="flex gap-3 pt-4">
              <Button type="button" variant="ghost" onClick={() => setIsModalOpen(false)}>
                取消
              </Button>
              <Button
                type="submit"
                variant="primary"
                isLoading={createStory.isPending}
                className="flex-1"
              >
                创建
              </Button>
            </div>
          </form>
        </Modal>
      )}

      {/* Wizard Modal */}
      {isWizardOpen && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in overflow-y-auto py-8">
          <Card className="w-full max-w-3xl mx-4 max-h-[85dvh] overflow-y-auto animate-slide-up my-auto">
            <CardContent className="p-8">
              {isCreating ? (
                <div className="text-center py-12">
                  <div className="relative w-20 h-20 mx-auto mb-6">
                    <div className="absolute inset-0 border-4 border-cinema-700 rounded-full" />
                    <div className="absolute inset-0 border-4 border-cinema-gold rounded-full border-t-transparent animate-spin" />
                    <Sparkles className="absolute inset-0 m-auto w-8 h-8 text-cinema-gold" />
                  </div>
                  <h3 className="text-xl font-semibold text-ai-ink mb-2">正在创建故事...</h3>
                </div>
              ) : (
                <NovelCreationWizard
                  onComplete={handleWizardComplete}
                  onCancel={() => setIsWizardOpen(false)}
                />
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}

function cn(...classes: (string | boolean | undefined)[]) {
  return classes.filter(Boolean).join(' ');
}
