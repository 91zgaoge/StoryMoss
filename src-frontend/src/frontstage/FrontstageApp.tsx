import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { loggedInvoke } from '@/services/tauri';
import { listen } from '@tauri-apps/api/event';
import { X } from 'lucide-react';
import {
  recordFeedback,
  smartExecute,
  checkPreflight,
  autoCreateMissingContracts,
  getInputHint,
  runRefine,
  runReview,
  runFinalize,
  getPipelineActiveDraft,
} from '@/services/tauri';
import { parseStructuredError } from '@/utils/errorHandler';
import { modelService } from '@/services/modelService';
import { autoFormatText } from '@/utils/format';
import { scheduleAutoSave, cancelAutoSave } from './autoSave';
import RichTextEditor, { RichTextEditorRef } from './components/RichTextEditor';
import { SmartHintSystem } from './ai-perception';
import { useCharacters } from '@/hooks/useCharacters';
import { useSyncStore } from '@/hooks/useSyncStore';
import { useAppStore } from '@/stores/appStore';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import { useSettings, useModels } from '@/hooks/useSettings';
import { useModelConnectionStore } from '@/stores/modelConnectionStore';
import { useQueryClient } from '@tanstack/react-query';
import type { Scene } from '@/types/v3';
import { useSubscription } from '@/hooks/useSubscription';
import { usePipelineProgress, usePipelineComplete } from '@/hooks/usePipelineProgress';
import { useBackendActivityListener } from '@/hooks/useBackendActivityListener';
// import { useIntent } from '@/hooks/useIntent'; // Removed — model-driven orchestration eliminates frontend intent parsing
import { loadEditorConfig } from '@/components/EditorSettings';
import { UpgradePanel } from './components/UpgradePanel';
import { WenSiPanel } from './components/WenSiPanel';

import { createLogger } from '@/utils/logger';

const frontstageLogger = createLogger('ui:FrontstageApp');

import FrontstageHeader from './components/FrontstageHeader';
import FrontstageBottomBar from './components/FrontstageBottomBar';

interface Story {
  id: string;
  title: string;
  description?: string;
}

interface Chapter {
  id: string;
  story_id: string;
  title?: string;
  chapter_number: number;
  content?: string;
  scene_id?: string;
}

interface FrontstageEvent {
  type: string;
  payload?: {
    text?: string;
    chapter_id?: string;
    story_id?: string;
    title?: string;
    content?: string;
    hint?: string;
    position?: { line: number; column: number };
    duration_ms?: number;
    saved?: boolean;
    timestamp?: string;
    entity?: string;
  };
}

type WensiMode = 'off' | 'passive' | 'active';

const FrontstageApp: React.FC = () => {
  const [stories, setStories] = useState<Story[]>([]);
  const [currentStory, setCurrentStory] = useState<Story | null>(null);
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [scenes, setScenes] = useState<Scene[]>([]);
  const [currentChapter, setCurrentChapter] = useState<Chapter | null>(null);

  // v5.4.1: 使用 ref 跟踪最新状态，避免 event listener 中的 stale closure
  const currentStoryRef = useRef(currentStory);
  const chaptersRef = useRef(chapters);
  const currentChapterRef = useRef(currentChapter);
  useEffect(() => {
    currentStoryRef.current = currentStory;
  }, [currentStory]);
  useEffect(() => {
    chaptersRef.current = chapters;
  }, [chapters]);
  useEffect(() => {
    currentChapterRef.current = currentChapter;
  }, [currentChapter]);
  const [currentScene, setCurrentScene] = useState<Scene | null>(null);
  // W2-F1 TODO: `content` 和 `isSaved` 应迁移到 frontstageStore（唯一可写源）。
  // 当前已通过事件入口保护（ContentUpdate/ChapterSwitch 在 isSaved===false 时不覆盖）
  // 和 RichTextEditor 焦点保护满足验收标准。
  const [content, setContent] = useState('');
  const [isSaved, setIsSaved] = useState(true);
  const isSavedRef = useRef(isSaved);
  useEffect(() => {
    isSavedRef.current = isSaved;
  }, [isSaved]);
  const [generatedText, setGeneratedText] = useState('');
  const [wordCount, setWordCount] = useState(0);
  const [fontSize, setFontSize] = useState(() => loadEditorConfig().fontSize);
  const [isZenMode, setIsZenMode] = useState(false);

  // 文思三态：关闭 / 被动提示 / 主动辅助
  const [wensiMode, setWensiMode] = useState<WensiMode>('passive');

  const [smartGhostText, setSmartGhostText] = useState('');
  const [inlineSuggestion, setInlineSuggestion] = useState<{
    instruction: string;
    targetText: string;
    category: string;
    targetParagraphIndex: number;
  } | null>(null);
  const [showUpgradePanel, setShowUpgradePanel] = useState(false);
  const [upgradeTrigger, setUpgradeTrigger] = useState('');
  const subscription = useSubscription();

  // const { parseIntent, executeIntent } = useIntent(); // Removed — all AI routing is now backend-driven
  // 统一实时状态同步中心：幕前监听后台数据变更，自动刷新本地状态
  // useSyncStore 内部已自动 invalidate TanStack Query 缓存，useCharacters/useScenes 等 hook 会自动重新获取
  useSyncStore({
    onStoryCreated: (storyId, title) => {
      toast.success(`故事「${title || '新故事'}」已创建`);
      loadStories();
    },
    onStoryDeleted: () => {
      loadStories();
    },
    // v5.4.0: 监听 scene 变更（幕后修改后同步到幕前 scenes 列表）
    onSceneCreated: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryScenes(storyId);
      }
    },
    onSceneUpdated: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryScenes(storyId);
      }
    },
    onSceneDeleted: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryScenes(storyId);
      }
    },
    // v5.4.0: 监听 chapter 创建/删除（幕后增删章节后同步幕前列表）
    onChapterCreated: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryChapters(storyId);
      }
    },
    onChapterDeleted: () => {
      if (currentStory) {
        loadStoryChapters(currentStory.id);
      }
    },
    // v5.2.0: 监听 chapter 更新（幕后修改后同步到幕前）
    onChapterUpdated: (chapterId, title) => {
      if (currentChapter && chapterId === currentChapter.id) {
        // 如果刚在 3 秒内自动保存过，忽略这次更新（避免循环）
        if (Date.now() - justSavedRef.current < 3000) {
          return;
        }
        // 静默刷新当前 chapter 内容
        (async () => {
          try {
            const updated = await loggedInvoke<Chapter | null>('get_chapter', { id: chapterId });
            if (updated && updated.content !== undefined) {
              setContent(prev => {
                if (prev !== updated.content) {
                  // 使用底部状态栏替代黑色 toast
                  setGenerationStatus('📝 幕后已更新本章内容');
                  setTimeout(() => {
                    setGenerationStatus(current =>
                      current === '📝 幕后已更新本章内容' ? '' : current
                    );
                  }, 2000);
                }
                return updated.content || '';
              });
            }
          } catch (e) {
            frontstageLogger.error('Failed to refresh chapter content', { error: e });
          }
        })();
      }
    },
  });

  const [isGenerating, setIsGenerating] = useState(false);
  const [generationStatus, setGenerationStatus] = useState('');
  const [orchestratorStatus, setOrchestratorStatus] = useState<{
    stepType: string;
    loopIdx?: number;
    score?: number;
    message: string;
    detail?: string;
  } | null>(null);

  // Bootstrap 进度
  const [bootstrapProgress, setBootstrapProgress] = useState<{
    stepName: string;
    stepNumber: number;
    totalSteps: number;
    message: string;
    status: string;
  } | null>(null);

  // v5.3.0: 大阶段实时提示 — 保存当前大阶段，避免底部状态栏闪烁
  const currentToastPhaseRef = useRef<string | null>(null);

  // v0.11.1: 统一状态提示 — 用顶部状态栏替代黑色 toast
  const showTransientStatus = useCallback((message: string, durationMs = 3000) => {
    setOrchestratorStatus({ stepType: 'info', message });
    setTimeout(() => {
      setOrchestratorStatus(current => (current?.message === message ? null : current));
    }, durationMs);
  }, []);

  const toast = useMemo<{
    (message: string, _opts?: unknown): string;
    success: (message: string, _opts?: unknown) => void;
    error: (message: string, _opts?: unknown) => void;
    loading: (message: string, _opts?: unknown) => string;
  }>(
    () =>
      Object.assign(
        (message: string, _opts?: unknown) => {
          showTransientStatus(message);
          return '';
        },
        {
          success: (message: string, _opts?: unknown) => showTransientStatus(`✓ ${message}`),
          error: (message: string, _opts?: unknown) => showTransientStatus(`✗ ${message}`),
          loading: (message: string, _opts?: unknown) => {
            showTransientStatus(
              message,
              _opts &&
                typeof _opts === 'object' &&
                'duration' in _opts &&
                (_opts as { duration: unknown }).duration === Infinity
                ? 60000
                : 3000
            );
            return '';
          },
        }
      ),
    [showTransientStatus]
  );

  // v0.11.2: 清理状态文案中的时间后缀与兜底提示，避免重复追加
  const cleanStatusBase = useCallback((prev: string): string => {
    return prev
      .replace(/（系统仍在处理中\.\.\.）/g, '')
      .replace(/\s*\(\d+s\)\s*$/g, '')
      .trim();
  }, []);

  /** 将细粒度步骤名映射为大阶段提示文案 */
  const getMajorPhase = useCallback((stepName: string): { icon: string; text: string } | null => {
    const s = stepName.toLowerCase();
    if (
      s.includes('构思') ||
      s.includes('概念') ||
      s.includes('创意') ||
      s.includes('conception')
    ) {
      return { icon: '🎨', text: '正在构思故事概念...' };
    }
    if (
      s.includes('开篇') ||
      s.includes('正文') ||
      s.includes('第一章') ||
      s.includes('first chapter') ||
      s.includes('撰写')
    ) {
      return { icon: '✍️', text: '正在撰写第一章...' };
    }
    if (s.includes('世界') || s.includes('世界观') || s.includes('world')) {
      return { icon: '🌍', text: '正在构建世界观...' };
    }
    if (s.includes('大纲') || s.includes('outline') || s.includes('结构')) {
      return { icon: '📋', text: '正在生成故事大纲...' };
    }
    if (s.includes('角色') || s.includes('character') || s.includes('人物')) {
      return { icon: '👤', text: '正在塑造角色...' };
    }
    if (s.includes('场景') || s.includes('scene') || s.includes('情节')) {
      return { icon: '🎬', text: '正在铺设场景...' };
    }
    if (s.includes('伏笔') || s.includes('foreshadow') || s.includes('铺垫')) {
      return { icon: '🔮', text: '正在埋设伏笔...' };
    }
    if (s.includes('图谱') || s.includes('kg') || s.includes('knowledge') || s.includes('graph')) {
      return { icon: '🕸️', text: '正在构建知识图谱...' };
    }
    if (s.includes('后台') || s.includes('background') || s.includes('完善')) {
      return { icon: '⏳', text: '后台正在完善小说世界...' };
    }
    if (s.includes('质检') || s.includes('检查') || s.includes('inspect')) {
      return { icon: '🔍', text: '正在质检生成内容...' };
    }
    if (s.includes('改写') || s.includes('润色') || s.includes('revise')) {
      return { icon: '✏️', text: '正在润色改写...' };
    }
    // v0.9.4: 智能执行与计划生成的大阶段映射，避免 Toast 长时间卡在初始提示
    if (s.includes('加载上下文') || s.includes('读取故事') || s.includes('读取章节')) {
      return { icon: '📂', text: '正在加载故事上下文...' };
    }
    if (
      s.includes('分析故事上下文') ||
      s.includes('生成执行计划') ||
      s.includes('planning') ||
      s.includes('context')
    ) {
      return { icon: '🧠', text: '正在规划创作步骤...' };
    }
    if (s.includes('执行创作计划') || s.includes('executing')) {
      return { icon: '⚙️', text: '正在执行创作计划...' };
    }
    if (s.includes('生成') || s.includes('续写') || s.includes('writing') || s.includes('draft')) {
      return { icon: '✍️', text: '正在生成续写内容...' };
    }
    if (s.includes('完成') || s.includes('completed')) {
      return { icon: '✅', text: '创作计划执行完成...' };
    }
    return null;
  }, []);

  /** 更新底部状态栏的大阶段提示（仅在阶段变化时更新，避免闪烁） */
  const updateGenerationPhase = useCallback(
    (stepName: string) => {
      const phase = getMajorPhase(stepName);
      if (!phase) return;
      const phaseKey = phase.text;
      // 只有大阶段变化时才更新
      if (currentToastPhaseRef.current === phaseKey) return;
      currentToastPhaseRef.current = phaseKey;
      setGenerationStatus(prev => {
        const base = cleanStatusBase(prev);
        // v0.11.2: 如果当前状态包含更具体的进度信息（如候选、第N轮、评分等），
        // 保留具体进度而不是用大阶段文案覆盖，让用户知道后台到底在做什么。
        const hasSpecificProgress = /候选|第\s*\d+\s*轮|评分|匹配度|降级|失败|准备中/.test(base);
        if (hasSpecificProgress) return prev;
        return `${phase.icon} ${phase.text}`;
      });
    },
    [getMajorPhase, cleanStatusBase]
  );

  // v0.7.7: 统一后台活动监听器 — 聚合所有进度事件到 backendActivityStore
  useBackendActivityListener();

  // v0.8.0: 将本地 isGenerating 与 backendActivityStore 对齐，避免状态分裂
  useEffect(() => {
    const unsub = useBackendActivityStore.subscribe(state => {
      const isAnyActive = state.getIsAnyActive();
      setIsGenerating(prev => {
        if (prev && !isAnyActive) {
          stopElapsedTimer();
          setGenerationStatus('');
          return false;
        }
        if (!prev && isAnyActive) {
          startElapsedTimer();
          return true;
        }
        return prev;
      });
    });
    return unsub;
  }, []);

  // v5.3.0: 统一 Pipeline 进度监听（同时更新 bootstrapProgress + 顶部 Toast 大阶段）
  const { progress: pipelineProgress } = usePipelineProgress({ pipelineType: 'genesis' });
  const lastPipelineComplete = usePipelineComplete();
  useEffect(() => {
    if (pipelineProgress) {
      setBootstrapProgress({
        stepName: pipelineProgress.stepName,
        stepNumber: pipelineProgress.stepNumber,
        totalSteps: pipelineProgress.totalSteps,
        message: pipelineProgress.message,
        status: pipelineProgress.status,
      });
      setGenerationStatus(pipelineProgress.message);
      updateGenerationPhase(pipelineProgress.stepName);
    }
  }, [pipelineProgress, updateGenerationPhase]);

  // WenSi 浮动面板
  const [showWenSiPanel, setShowWenSiPanel] = useState(false);
  const [wenSiTab, setWenSiTab] = useState<'write' | 'revise' | 'dialog'>('write');

  // F1 帮助面板
  const [showHelpPanel, setShowHelpPanel] = useState(false);

  // 底部输入栏
  const [inputValue, setInputValue] = useState('');

  // 输入栏智能提示系统
  const [ghostHint, setGhostHint] = useState(''); // 灰色提示内容
  const [hintSource, setHintSource] = useState<'llm' | 'history'>('llm');
  const [inputHistory, setInputHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1); // -1=LLM建议, 0+=历史
  // v0.7.8: 使用新版模型管理系统丰富底部栏 tooltip
  const { data: settings } = useSettings();
  const { data: allModels = [] } = useModels();
  const connectionStates = useModelConnectionStore(state => state.states);
  const startPolling = useModelConnectionStore(state => state.startPolling);
  const activeChatModelId = settings?.active_models?.chat;
  const activeChatModel = allModels.find(m => m.id === activeChatModelId);
  const chatConnectionState = activeChatModelId ? connectionStates[activeChatModelId] : undefined;
  const queryClient = useQueryClient();

  // v0.11.0: 状态栏模型状态从 store 派生
  const modelStatus: 'connected' | 'disconnected' | 'connecting' = chatConnectionState
    ? chatConnectionState.isChecking
      ? 'connecting'
      : chatConnectionState.result.success
        ? 'connected'
        : 'disconnected'
    : 'connecting';
  const modelName = activeChatModel?.name || activeChatModelId || '未配置';

  // v0.11.0: 启动模型连接轮询，状态栏展示当前可用模型与连接质量
  useEffect(() => {
    const enabledIds = allModels.filter(m => m.enabled).map(m => m.id);
    if (enabledIds.length === 0) return;
    const stop = startPolling(enabledIds, 30000);
    return () => stop();
  }, [allModels, startPolling]);

  // AI 学习指示器

  const editorRef = useRef<RichTextEditorRef>(null);
  const typewriterIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // v5.2.0: 标记刚完成自动保存的时间戳，避免循环刷新
  const justSavedRef = useRef<number>(0);
  // 生成任务计时器：记录开始时间 + 定时更新运行时长显示
  const generationStartTimeRef = useRef<number | null>(null);
  const elapsedTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // 备用机制：记录最后收到事件的时间，如果10秒内无新事件则显示提示
  const lastEventTimeRef = useRef<number>(Date.now());
  const fallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 辅助函数：启动运行时长计时器
  const startElapsedTimer = useCallback(() => {
    generationStartTimeRef.current = Date.now();
    lastEventTimeRef.current = Date.now();
    if (elapsedTimerRef.current) clearInterval(elapsedTimerRef.current);
    elapsedTimerRef.current = setInterval(() => {
      const elapsed = generationStartTimeRef.current
        ? Math.floor((Date.now() - generationStartTimeRef.current) / 1000)
        : 0;
      setGenerationStatus(prev => {
        // 保留原有的状态前缀，只更新后面的时间部分
        const base = cleanStatusBase(prev) || 'AI 正在处理中';
        return `${base} (${elapsed}s)`;
      });
    }, 1000);
    // 启动备用提示定时器：每10秒检查一次是否有新事件
    if (fallbackTimerRef.current) clearInterval(fallbackTimerRef.current);
    fallbackTimerRef.current = setInterval(() => {
      const sinceLastEvent = Date.now() - lastEventTimeRef.current;
      if (sinceLastEvent > 10000) {
        setGenerationStatus(prev => {
          // 如果已经有模型生成中的提示，不要覆盖
          if (prev.includes('正在生成中') || prev.includes('等待响应')) return prev;
          const base = cleanStatusBase(prev) || 'AI 正在处理中';
          const elapsed = generationStartTimeRef.current
            ? Math.floor((Date.now() - generationStartTimeRef.current) / 1000)
            : 0;
          return `${base}（系统仍在处理中...） (${elapsed}s)`;
        });
      }
    }, 10000);
  }, [cleanStatusBase]);

  // 辅助函数：停止运行时长计时器
  const stopElapsedTimer = useCallback(() => {
    if (elapsedTimerRef.current) {
      clearInterval(elapsedTimerRef.current);
      elapsedTimerRef.current = null;
    }
    if (fallbackTimerRef.current) {
      clearInterval(fallbackTimerRef.current);
      fallbackTimerRef.current = null;
    }
    generationStartTimeRef.current = null;
  }, []);

  // 辅助函数：更新最后收到事件的时间
  const updateLastEventTime = useCallback(() => {
    lastEventTimeRef.current = Date.now();
  }, []);

  // W2-F2: 监听编辑器配置变化（同步幕后设置到幕前），替代 editor-config-changed DOM CustomEvent
  const editorConfig = useAppStore(state => state.editorConfig);
  useEffect(() => {
    if (editorConfig?.fontSize) {
      setFontSize(editorConfig.fontSize);
    }
  }, [editorConfig]);

  // 加载当前故事的角色
  const { data: characters = [] } = useCharacters(currentStory?.id || null);

  // Load stories on mount
  useEffect(() => {
    const unlisteners: (() => void)[] = [];
    loadStories();
    setupEventListeners(unlisteners);
    return () => {
      unlisteners.forEach(u => u());
      if (typewriterIntervalRef.current) {
        clearInterval(typewriterIntervalRef.current);
        typewriterIntervalRef.current = null;
      }
    };
  }, []);

  // Setup Tauri event listeners
  const setupEventListeners = async (unlisteners: (() => void)[]) => {
    try {
      // 监听 frontstage-update 事件
      const unlisten1 = await listen<FrontstageEvent>('frontstage-update', event => {
        const { type, payload } = event.payload;

        switch (type) {
          case 'ContentUpdate':
            // W2-F1: 有未保存更改时不覆盖，避免同步事件导致内容回滚
            if (payload?.text !== undefined && isSavedRef.current) {
              setContent(autoFormatText(payload.text));
            }
            break;
          case 'AppendContent':
            if (payload?.text !== undefined) {
              const formatted = autoFormatText(payload.text);
              setContent(prev => prev + formatted);
            }
            break;
          case 'DataRefresh':
            // v0.11.2: 模型配置变更时刷新 settings/models，让幕前立即感知新活跃模型
            if (payload?.entity === 'model_config') {
              queryClient.invalidateQueries({ queryKey: ['settings'] });
              queryClient.invalidateQueries({ queryKey: ['models'] });
            }
            loadStories();
            // W2-F2: characters-refreshed DOM CustomEvent 已废弃，数据刷新由 useSyncStore 统一处理
            break;
          case 'SaveStatus':
            setIsSaved(payload?.saved ?? true);
            break;
          case 'ChapterSwitch':
            if (payload?.chapter_id) {
              frontstageLogger.info('[ChapterSwitch] Received event', {
                story_id: payload.story_id,
                chapter_id: payload.chapter_id,
                has_content: !!payload.content,
                content_length: payload.content?.length || 0,
              });
              // v5.4.1 fix: 如果事件中直接包含内容，直接使用（绕过 DB 查询竞态）
              // W2-F1: 切换不同章节时无条件加载；同一章节且有未保存更改时不覆盖，避免内容回滚
              if (payload?.content && payload.content.trim().length > 0) {
                const isSameChapter = payload.chapter_id === currentChapterRef.current?.id;
                if (!isSameChapter || isSaved) {
                  frontstageLogger.info('[ChapterSwitch] Using content from event directly');
                  setContent(autoFormatText(payload.content));
                } else {
                  frontstageLogger.info(
                    '[ChapterSwitch] Skipping content overwrite (unsaved changes)'
                  );
                }
              }
              // v5.4.1: 使用 ref 获取最新状态，避免 stale closure
              if (payload?.story_id && payload.story_id !== currentStoryRef.current?.id) {
                (async () => {
                  try {
                    const allStories = await loggedInvoke<Story[]>('list_stories');
                    const targetStory = allStories.find(s => s.id === payload.story_id);
                    frontstageLogger.info('[ChapterSwitch] Target story lookup', {
                      found: !!targetStory,
                      story_count: allStories.length,
                    });
                    if (targetStory) {
                      const storyChapters = await loggedInvoke<Chapter[]>('get_story_chapters', {
                        story_id: targetStory.id,
                      });
                      const storyScenes = await loggedInvoke<Scene[]>('get_story_scenes', {
                        story_id: targetStory.id,
                      });
                      frontstageLogger.info('[ChapterSwitch] Loaded chapters', {
                        count: storyChapters.length,
                        chapter_ids: storyChapters.map(c => c.id),
                      });
                      setCurrentStory(targetStory);
                      setChapters(storyChapters);
                      setScenes(storyScenes);
                      let targetChapter = storyChapters.find(c => c.id === payload.chapter_id);
                      // v5.4.0 fallback: 如果找不到目标 chapter，尝试加载第一个 chapter
                      if (!targetChapter && storyChapters.length > 0) {
                        targetChapter = storyChapters[0];
                        frontstageLogger.warn(
                          '[ChapterSwitch] Target chapter not found by ID, falling back to first chapter',
                          {
                            expected_id: payload.chapter_id,
                            fallback_id: targetChapter.id,
                            has_content: !!targetChapter.content,
                          }
                        );
                      }
                      if (targetChapter) {
                        frontstageLogger.info('[ChapterSwitch] Selecting chapter', {
                          chapter_id: targetChapter.id,
                          content_length: targetChapter.content?.length || 0,
                        });
                        selectChapter(targetChapter);
                      } else {
                        frontstageLogger.error(
                          '[ChapterSwitch] No chapters available for new story'
                        );
                      }
                      // v5.0.0 修复：通知 backstage 刷新故事列表，确保幕后也能看到新故事
                      try {
                        await loggedInvoke<unknown>('notify_backstage_content_changed', {
                          text: targetChapter?.content || '',
                          chapter_id: targetChapter?.id || '',
                        });
                      } catch (e) {
                        // ignore
                      }
                    } else {
                      frontstageLogger.error(
                        '[ChapterSwitch] Target story not found in list_stories',
                        { story_id: payload.story_id }
                      );
                    }
                  } catch (e) {
                    frontstageLogger.error('Failed to switch to new story', { error: e });
                  }
                })();
              } else {
                // v5.4.1: 使用 ref 获取最新 chapters，避免 stale closure
                const chapter = chaptersRef.current.find(c => c.id === payload.chapter_id);
                if (chapter) {
                  frontstageLogger.info('[ChapterSwitch] Selecting chapter (same story)', {
                    chapter_id: chapter.id,
                    content_length: chapter.content?.length || 0,
                  });
                  selectChapter(chapter);
                } else {
                  // v5.4.1 fix: chaptersRef 可能为空（Bootstrap 竞态：storyCreated→loadStories→selectStory 在 ChapterSwitch 之前设置了空 chapters）
                  // 此时必须重新查询数据库获取最新章节
                  frontstageLogger.warn(
                    '[ChapterSwitch] Chapter not found in current story, re-fetching from DB',
                    { chapter_id: payload.chapter_id, story_id: payload.story_id }
                  );
                  (async () => {
                    try {
                      const freshChapters = await loggedInvoke<Chapter[]>('get_story_chapters', {
                        story_id: payload.story_id,
                      });
                      const freshChapter = freshChapters.find(c => c.id === payload.chapter_id);
                      if (freshChapter) {
                        frontstageLogger.info('[ChapterSwitch] Found chapter after re-fetch', {
                          chapter_id: freshChapter.id,
                          content_length: freshChapter.content?.length || 0,
                        });
                        setChapters(freshChapters);
                        selectChapter(freshChapter);
                      } else if (freshChapters.length > 0) {
                        frontstageLogger.warn(
                          '[ChapterSwitch] Target chapter not found after re-fetch, falling back to first',
                          { expected_id: payload.chapter_id, fallback_id: freshChapters[0].id }
                        );
                        setChapters(freshChapters);
                        selectChapter(freshChapters[0]);
                      } else {
                        frontstageLogger.error(
                          '[ChapterSwitch] No chapters available after re-fetch'
                        );
                      }
                    } catch (e) {
                      frontstageLogger.error('[ChapterSwitch] Failed to re-fetch chapters', {
                        error: e,
                      });
                    }
                  })();
                }
              }
            } else {
              frontstageLogger.warn('[ChapterSwitch] Missing chapter_id in payload');
            }
            break;
        }
      });
      unlisteners.push(unlisten1);

      // 监听 novel-bootstrap-error 事件（后台阶段错误可见化）
      const unlisten2 = await listen<{
        step: string;
        story_id: string;
        error: string;
      }>('novel-bootstrap-error', event => {
        const p = event.payload;
        frontstageLogger.error('[novel-bootstrap-error]', { step: p.step, error: p.error });
        toast.error(`后台完善失败（${p.step}）: ${p.error}`, { duration: 5000 });
        setGenerationStatus('');
        setBootstrapProgress(null);
        currentToastPhaseRef.current = null;
      });
      unlisteners.push(unlisten2);

      // 监听 novel-bootstrap-progress 事件
      const unlisten3 = await listen<{
        session_id: string;
        step_name: string;
        step_number: number;
        total_steps: number;
        message: string;
        status: string;
      }>('novel-bootstrap-progress', event => {
        const p = event.payload;
        updateLastEventTime();
        setBootstrapProgress({
          stepName: p.step_name,
          stepNumber: p.step_number,
          totalSteps: p.total_steps,
          message: p.message,
          status: p.status || 'running',
        });
        setGenerationStatus(p.message);
        updateGenerationPhase(p.step_name);
        // v5.2.2 / v5.4.0: 区分即时阶段完成和后台阶段完成
        // GenesisPipeline 即时阶段 total_steps=2，后台阶段 total_steps=6
        if (p.status === 'failed') {
          // 步骤失败：显示错误提示并清理进度
          toast.error(`创世失败: ${p.message}`, { duration: 8000 });
          currentToastPhaseRef.current = null;
          setTimeout(() => {
            setBootstrapProgress(null);
            setGenerationStatus('');
          }, 8000);
        } else if (p.total_steps === 2 && p.step_number >= p.total_steps) {
          // 即时阶段完成：正文已生成，用户可开始写作
          setTimeout(() => {
            setBootstrapProgress(null);
            setGenerationStatus('⏳ 后台正在完善小说世界...');
            currentToastPhaseRef.current = '⏳ 后台正在完善小说世界...';
          }, 2000);
        } else if (p.total_steps === 6 && p.step_number >= p.total_steps) {
          // 后台阶段全部完成（GenesisPipeline 最后一步：知识图谱生成）
          toast.success('创世完成！世界观、角色、场景、伏笔已全部生成');
          currentToastPhaseRef.current = null;
          setTimeout(() => {
            setBootstrapProgress(null);
            setGenerationStatus('');
          }, 3000);
        }
      });
      unlisteners.push(unlisten3);

      // 监听 plan-generator-progress 事件 — 方案C：流式进度反馈
      const unlisten4 = await listen<{
        stage: string;
        message: string;
      }>('plan-generator-progress', event => {
        const p = event.payload;
        updateLastEventTime();
        setGenerationStatus(p.message);
        updateGenerationPhase(p.stage);
      });
      unlisteners.push(unlisten4);

      // 监听 smart-execute-progress 事件 — 整体执行进度
      const unlisten5 = await listen<{
        stage: string;
        message: string;
        step_number: number;
        total_steps: number;
      }>('smart-execute-progress', event => {
        const p = event.payload;
        updateLastEventTime();
        setGenerationStatus(p.message);
        updateGenerationPhase(p.stage);
      });
      unlisteners.push(unlisten5);

      // 监听 plan-executor-step 事件 — 步骤级进度
      const unlisten6 = await listen<{
        step_id: string;
        capability_id: string;
        status: string;
        message: string;
        steps_completed: number;
        total_steps: number;
      }>('plan-executor-step', event => {
        const p = event.payload;
        updateLastEventTime();
        frontstageLogger.debug('[plan-executor-step]', {
          status: p.status,
          message: p.message,
          progress: `${p.steps_completed}/${p.total_steps}`,
        });
        if (p.step_id === '__complete__') {
          setGenerationStatus(p.message);
        } else if (p.status === 'running') {
          setGenerationStatus(`${p.message} (${p.steps_completed + 1}/${p.total_steps})`);
          updateGenerationPhase(p.message);
        } else if (p.status === 'completed') {
          setGenerationStatus(p.message);
        } else if (p.status === 'failed') {
          setGenerationStatus(p.message);
        }
      });
      unlisteners.push(unlisten6);

      // v0.9.4: 监听 orchestrator-step 事件 — Writer / Inspector / Rewrite 质量闭环
      // 之前只在 handleRequestGeneration 中局部监听，导致智能输入栏（smart generation）无法看到细粒度进度
      const unlistenOrchestrator = await listen<{
        task_id: string;
        step_type: string;
        loop_idx?: number;
        score?: number;
        detail?: string;
      }>('orchestrator-step', event => {
        const p = event.payload;
        updateLastEventTime();
        const stepNames: Record<string, string> = {
          生成: '生成中...',
          质检: '质检中...',
          改写: '改写中...',
        };
        let message = p.detail || stepNames[p.step_type] || p.step_type;
        if (p.step_type === '改写' && typeof p.loop_idx === 'number' && !p.detail) {
          message = `第 ${p.loop_idx + 1} 轮优化中...`;
        }
        if (p.step_type === '质检' && typeof p.score === 'number' && !p.detail) {
          message = `质检中... 评分 ${p.score}%`;
        }
        setGenerationStatus(message);
        setOrchestratorStatus({
          stepType: p.step_type,
          loopIdx: p.loop_idx,
          score: p.score,
          message,
          detail: p.detail,
        });
        updateGenerationPhase(p.step_type);
      });
      unlisteners.push(unlistenOrchestrator);

      // 监听 agent-stage-update 事件 — Agent内部阶段
      const unlisten7 = await listen<{
        agent_type: string;
        stage: string;
        message: string;
        progress: number;
      }>('agent-stage-update', event => {
        const p = event.payload;
        updateLastEventTime();
        frontstageLogger.debug('[agent-stage-update]', {
          stage: p.stage,
          agent_type: p.agent_type,
          message: p.message,
        });
        // v0.11.2: Agent 内部阶段不应覆盖更具体的进度（如候选生成）。
        // 如果当前状态包含具体进度，仅记录阶段；否则显示 Agent 阶段。
        setGenerationStatus(prev => {
          const base = cleanStatusBase(prev);
          const hasSpecificProgress = /候选|第\s*\d+\s*轮|评分|匹配度|降级|失败|准备中/.test(base);
          if (hasSpecificProgress) return prev;
          return `${p.agent_type}: ${p.message}`;
        });
        updateGenerationPhase(p.stage);
      });
      unlisteners.push(unlisten7);

      // 监听 llm-generating-progress 事件 — LLM模型生成心跳
      const unlisten8 = await listen<{
        stage: string;
        message: string;
        elapsed_seconds: number;
        model: string;
        pipeline_context?: {
          step_name: string;
          step_number: number;
          total_steps: number;
          action: string;
        };
      }>('llm-generating-progress', event => {
        const p = event.payload;
        updateLastEventTime();
        frontstageLogger.debug('[llm-generating-progress]', {
          stage: p.stage,
          message: p.message,
          pipeline_context: p.pipeline_context,
        });

        // v5.2.4: 如果携带Pipeline步骤上下文，同步更新bootstrapProgress
        if (p.pipeline_context) {
          setBootstrapProgress({
            stepName: p.pipeline_context.step_name,
            stepNumber: p.pipeline_context.step_number,
            totalSteps: p.pipeline_context.total_steps,
            message: p.message,
            status: 'running',
          });
          updateGenerationPhase(p.pipeline_context.step_name);
        }

        // v0.11.2: LLM 心跳不应覆盖更具体的阶段进度（如"生成候选 1/2"）。
        // 如果当前状态包含具体进度，仅更新时间；否则显示 LLM 心跳消息。
        setGenerationStatus(prev => {
          const base = cleanStatusBase(prev);
          const hasSpecificProgress = /候选|第\s*\d+\s*轮|评分|匹配度|降级|失败|准备中/.test(base);
          if (hasSpecificProgress) {
            return `${base} (${p.elapsed_seconds}s)`;
          }
          return p.message;
        });
      });
      unlisteners.push(unlisten8);

      // v5.2.0: 监听上下文降级事件
      const unlisten9 = await listen<{
        story_id: string;
        reason: string;
        fallback: string;
      }>('context-degraded', event => {
        const p = event.payload;
        frontstageLogger.warn('[context-degraded]', { reason: p.reason });
        // 使用底部状态栏替代黑色 toast
        setGenerationStatus('⚡ 正在使用简化上下文生成内容...');
      });
      unlisteners.push(unlisten9);
    } catch (e) {
      frontstageLogger.error('Failed to setup event listeners', { error: e });
    }
  };

  const loadStories = async () => {
    try {
      const result = await loggedInvoke<Story[]>('list_stories');
      setStories(result);
      // v5.4.1 fix: Bootstrap 期间（isGenerating=true）不要自动选择 story，
      // 避免 FirstChapterGenerationStep 尚未完成时 selectStory 拿到空 chapters 导致编辑器被清空
      if (result.length > 0 && !currentStory && !isGenerating) {
        await selectStory(result[0]);
      }
    } catch (e) {
      frontstageLogger.error('Failed to load stories', { error: e });
    }
  };

  // v5.4.0: 刷新当前故事的 scenes 列表（用于 sync-event 回调）
  const loadStoryScenes = async (storyId: string) => {
    try {
      const result = await loggedInvoke<Scene[]>('get_story_scenes', { story_id: storyId });
      setScenes(result);
    } catch (e) {
      frontstageLogger.error('Failed to load scenes', { error: e });
    }
  };

  // v5.4.0: 刷新当前故事的 chapters 列表（用于 sync-event 回调）
  const loadStoryChapters = async (storyId: string) => {
    try {
      const result = await loggedInvoke<Chapter[]>('get_story_chapters', { story_id: storyId });
      setChapters(result);
    } catch (e) {
      frontstageLogger.error('Failed to load chapters', { error: e });
    }
  };

  const selectStory = async (story: Story) => {
    setCurrentStory(story);
    try {
      const [result, scenesResult] = await Promise.all([
        loggedInvoke<Chapter[]>('get_story_chapters', { story_id: story.id }),
        loggedInvoke<Scene[]>('get_story_scenes', { story_id: story.id }),
      ]);
      setChapters(result);
      setScenes(scenesResult);
      if (result.length > 0) {
        selectChapter(result[0]);
      } else {
        setCurrentChapter(null);
        setCurrentScene(null);
        setContent('');
      }
    } catch (e) {
      console.error('Failed to load chapters:', e);
    }
  };

  const selectChapter = (chapter: Chapter) => {
    frontstageLogger.info('[selectChapter] Selecting chapter', {
      chapter_id: chapter.id,
      content_length: chapter.content?.length ?? 0,
      content_preview: chapter.content?.slice(0, 50) ?? 'EMPTY',
    });
    cancelAutoSave();
    setCurrentChapter(chapter);
    setContent(autoFormatText(chapter.content || ''));
    setIsSaved(true);

    // Sync currentScene if chapter has associated scene
    if (chapter.scene_id) {
      const associatedScene = scenes.find(s => s.id === chapter.scene_id);
      setCurrentScene(associatedScene || null);
    } else {
      setCurrentScene(null);
    }
  };

  // v0.9.5: Genesis 后台阶段完成后自动刷新当前故事内容
  useEffect(() => {
    if (!lastPipelineComplete || lastPipelineComplete.pipelineType !== 'genesis') return;
    if (!lastPipelineComplete.success) {
      toast.error('第一章后台生成失败，请检查模型配置或稍后重试');
      return;
    }
    if (!currentStory?.id) return;
    (async () => {
      try {
        const storyChapters = await loggedInvoke<Chapter[]>('get_story_chapters', {
          story_id: currentStory.id,
        });
        const storyScenes = await loggedInvoke<Scene[]>('get_story_scenes', {
          story_id: currentStory.id,
        });
        setChapters(storyChapters);
        setScenes(storyScenes);
        const activeChapter =
          storyChapters.find(c => c.id === currentChapter?.id) || storyChapters[0];
        if (activeChapter) {
          selectChapter(activeChapter);
          toast.success('第一章生成完成，已开始写作');
        }
      } catch (e) {
        frontstageLogger.error('[PipelineComplete] Failed to refresh story after genesis', {
          error: e,
        });
      }
    })();
  }, [lastPipelineComplete, currentStory?.id, currentChapter?.id, selectChapter]);

  const handleContentChange = useCallback(
    async (newContent: string) => {
      setContent(newContent);
      setIsSaved(false);

      const text = newContent.replace(/<[^>]*>/g, '');
      const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
      const englishWords = (text.match(/[a-zA-Z]+/g) || []).length;
      setWordCount(chineseChars + englishWords);

      const computedWordCount = chineseChars + englishWords;

      if (currentChapter) {
        // W4-F7: 自动保存非阻塞化 — 使用 requestIdleCallback + startTransition
        scheduleAutoSave(
          {
            chapterId: currentChapter.id,
            title: currentChapter.title,
            content: newContent,
            wordCount: computedWordCount,
          },
          async payload => {
            try {
              await loggedInvoke<unknown>('update_chapter', {
                id: payload.chapterId,
                title: payload.title,
                content: payload.content,
                word_count: payload.wordCount,
              });
              setIsSaved(true);
              justSavedRef.current = Date.now();
            } catch (e) {
              frontstageLogger.error('Auto-save failed', { error: e });
            }
          },
          2000
        );

        loggedInvoke<unknown>('notify_backstage_content_changed', {
          text: newContent,
          chapter_id: currentChapter.id,
        }).catch(e => frontstageLogger.error('Failed to notify content change', { error: e }));
      }
    },
    [currentChapter]
  );

  const openBackstage = async () => {
    try {
      await loggedInvoke<unknown>('show_backstage', { story_id: currentStory?.id || null });
    } catch (e) {
      frontstageLogger.error('Failed to open backstage', { error: e });
      const isTauri = !!(window as any).__TAURI__;
      if (!isTauri) {
        window.open('http://127.0.0.1:5173/index.html', '_blank');
      }
    }
  };

  // 文思三态循环切换
  const cycleWensiMode = useCallback(() => {
    setWensiMode(prev => {
      if (prev === 'off') return 'passive';
      if (prev === 'passive') return 'active';
      return 'off';
    });
  }, []);

  // Request AI generation -- now routes through backend smart_execute
  const handleRequestGeneration = useCallback(
    async (context?: string) => {
      if (isGenerating) {
        // 使用顶部状态栏替代黑色 toast
        setOrchestratorStatus({ stepType: 'busy', message: 'AI 正在生成中，请稍候...' });
        setTimeout(() => {
          setOrchestratorStatus(current =>
            current?.message === 'AI 正在生成中，请稍候...' ? null : current
          );
        }, 2000);
        return;
      }

      if (typewriterIntervalRef.current) {
        clearInterval(typewriterIntervalRef.current);
        typewriterIntervalRef.current = null;
      }

      // v0.7.5: 写作前预检，提前发现阻塞性问题；缺少合同/大纲时自动补齐
      if (currentStory?.id && currentChapter?.chapter_number !== undefined) {
        try {
          const preflight = await checkPreflight(currentStory.id, currentChapter.chapter_number);
          if (!preflight.ready) {
            const isMissingContracts = preflight.missing_contracts.length > 0;
            const isMissingOutline = preflight.blocking_issues.some(
              (i: string) => i.includes('大纲') || i.includes('outline')
            );
            if (isMissingContracts || isMissingOutline) {
              setIsGenerating(true);
              // 生成明确的提示文案
              const missingItems: string[] = [];
              if (isMissingContracts) {
                if (preflight.missing_contracts.includes('MASTER_SETTING'))
                  missingItems.push('世界观合同');
                if (preflight.missing_contracts.some((c: string) => c.startsWith('CHAPTER_')))
                  missingItems.push('章节合同');
              }
              if (isMissingOutline) missingItems.push('场景大纲');
              const hintMsg = `检测到缺少 ${missingItems.join('、')}，系统正在自动补齐，请稍候...`;
              const loadingToastId = toast.loading(hintMsg, { duration: Infinity });
              setGenerationStatus(hintMsg);
              let progressUnlisten: (() => void) | null = null;
              try {
                progressUnlisten = await listen('contract-auto-progress', event => {
                  const p = event.payload as any;
                  setGenerationStatus(p.message);
                  const pct = Math.round((p.progress || 0) * 100);
                  toast.loading(`${p.message} (${pct}%)`, { id: loadingToastId });
                });
                const targetSceneId = currentScene?.id || currentChapter.scene_id;
                const result = await autoCreateMissingContracts(
                  currentStory.id,
                  currentChapter.chapter_number,
                  targetSceneId
                );
                if (
                  !result.created_master_setting &&
                  !result.created_chapter_contract &&
                  !result.created_outline
                ) {
                  toast.error(`自动补齐未成功（${missingItems.join('、')}），请手动创建`, {
                    id: loadingToastId,
                  });
                  setIsGenerating(false);
                  setGenerationStatus('');
                  return;
                }
                toast.success(`补齐完成（${missingItems.join('、')}），继续生成...`, {
                  id: loadingToastId,
                });
                // 补齐成功，继续执行后续生成逻辑
              } catch (e) {
                frontstageLogger.error('Auto creation failed', { error: e });
                toast.error('自动补齐失败，请手动创建', { id: loadingToastId });
                setIsGenerating(false);
                setGenerationStatus('');
                return;
              } finally {
                if (progressUnlisten) progressUnlisten();
              }
            } else {
              const issues =
                preflight.blocking_issues.length > 0
                  ? preflight.blocking_issues
                  : preflight.missing_contracts;
              const firstIssue = issues[0] || '写作前检查未通过';
              toast.error(`写作前检查未通过：${firstIssue}`, { duration: 6000 });
              setIsGenerating(false);
              return;
            }
          }
        } catch (e) {
          frontstageLogger.warn('Preflight check failed silently', { error: e });
          // 预检调用失败不阻断，让后端做最终检查
        }
      }

      setGeneratedText('');
      setIsGenerating(true);
      setGenerationStatus('正在续写...');
      setOrchestratorStatus(null);
      startElapsedTimer();

      // v0.11.5: 前端超时从 600 秒缩短到 300 秒，与后端总超时（本地约 150s / 远程约 270s）
      // 保持一定余量。超时后主动清理 backendActivityStore，避免状态栏卡死。
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          useBackendActivityStore.getState().failAllRunning('前端超时：模型未在 300 秒内响应');
          reject(
            new Error(
              '前端超时：模型响应超过300秒（5分钟）。本地模型生成长文本较慢，请检查模型是否仍在运行，或尝试使用更快的模型。'
            )
          );
        }, 300000);
      });
      cancelGenerationRef.current = () => {
        if (timeoutId) clearTimeout(timeoutId);
      };

      try {
        const result = await Promise.race([
          smartExecute({
            user_input: context || '续写',
            current_content: editorRef.current?.getText(),
            style_weight: 50,
          }),
          timeoutPromise,
        ]);
        if (timeoutId) clearTimeout(timeoutId);

        setGenerationStatus('质检通过，生成完成');
        setOrchestratorStatus({ stepType: '完成', message: '质检通过，生成完成' });

        // v5.1.0: Bootstrap 完成后自动加载新故事并切换到第一章
        const storyCreatedMsg = result.messages?.find((m: string) =>
          m.startsWith('story_created:')
        );
        const isBackgroundBootstrap = result.messages?.some(
          (m: string) => m === 'novel_bootstrap_background_started'
        );

        if (storyCreatedMsg) {
          const newStoryId = storyCreatedMsg.replace('story_created:', '');
          (async () => {
            try {
              const allStories = await loggedInvoke<Story[]>('list_stories');
              const targetStory = allStories.find(s => s.id === newStoryId);
              if (targetStory) {
                const storyChapters = await loggedInvoke<Chapter[]>('get_story_chapters', {
                  story_id: targetStory.id,
                });
                const storyScenes = await loggedInvoke<Scene[]>('get_story_scenes', {
                  story_id: targetStory.id,
                });
                setCurrentStory(targetStory);
                setChapters(storyChapters);
                setScenes(storyScenes);
                if (storyChapters.length > 0) {
                  selectChapter(storyChapters[0]);
                }
              }
            } catch (e) {
              frontstageLogger.error('[Bootstrap] Failed to auto-load new story', { error: e });
            }
          })();
        }

        // v0.9.5: 新故事创建后，第一章在后台生成，此时 final_content 为空是正常的
        if (isBackgroundBootstrap) {
          frontstageLogger.info(
            '[Bootstrap] Story created, first chapter generating in background'
          );
          toast.success('故事已创建，第一章正在后台生成，完成后会自动加载', {
            duration: 5000,
          });
          stopElapsedTimer();
          setIsGenerating(false);
          setGenerationStatus('');
          setOrchestratorStatus(null);
          return;
        }

        const text = result.final_content || '';
        if (!text.trim()) {
          frontstageLogger.error('[Generation] Backend returned empty content');
          toast.error('AI 返回了空内容，请检查模型配置或重试', { duration: 5000 });
          stopElapsedTimer();
          setIsGenerating(false);
          setGenerationStatus('');
          setOrchestratorStatus(null);
          return;
        }
        // v5.6.4 fix: 去除与当前编辑器内容重复的前缀，防止 LLM 返回完整文本导致"重复输出"
        let displayText = text;
        const currentText = editorRef.current?.getText() || '';
        if (currentText && displayText.startsWith(currentText)) {
          displayText = displayText.slice(currentText.length).trimStart();
          frontstageLogger.info(
            '[RequestGeneration] Removed duplicate prefix from generated text',
            {
              prefix_len: currentText.length,
              remaining_len: displayText.length,
            }
          );
        }
        // 如果去重后为空，说明 LLM 返回的内容与已有内容完全相同
        if (!displayText.trim()) {
          stopElapsedTimer();
          setIsGenerating(false);
          setGenerationStatus('');
          // 使用顶部状态栏替代黑色 toast
          setOrchestratorStatus({
            stepType: 'info',
            message: 'AI 续写内容与当前文本相同，无需添加',
          });
          setTimeout(() => {
            setOrchestratorStatus(current =>
              current?.message === 'AI 续写内容与当前文本相同，无需添加' ? null : current
            );
          }, 3000);
          return;
        }
        let index = 0;
        typewriterIntervalRef.current = setInterval(() => {
          index += 3;
          if (index >= displayText.length) {
            if (typewriterIntervalRef.current) {
              clearInterval(typewriterIntervalRef.current);
              typewriterIntervalRef.current = null;
            }
            setGeneratedText(displayText);
            stopElapsedTimer();
            setIsGenerating(false);
            setOrchestratorStatus(null);
          } else {
            setGeneratedText(displayText.slice(0, index));
          }
        }, 16);
      } catch (error) {
        if (timeoutId) clearTimeout(timeoutId);
        stopElapsedTimer();
        // v0.11.5: 任何失败/超时都要清理 backendActivityStore，避免状态栏残留
        useBackendActivityStore
          .getState()
          .failAllRunning(
            error instanceof Error &&
              (error.message.includes('超时') || error.message.includes('timed out'))
              ? '模型响应超时'
              : '生成失败'
          );
        frontstageLogger.error('Generation request failed', { error });
        const structured = parseStructuredError(error);
        const msg = error instanceof Error ? error.message : String(error);
        if (structured?.code === 'PREFLIGHT_FAILED') {
          const issues = (structured.data?.issues as string[]) || [];
          const firstIssue = issues[0] || structured.message || '写作前检查未通过';
          const isMissingContracts = issues.some(
            (i: string) =>
              i.includes('合同') || i.includes('MASTER_SETTING') || i.includes('章节合同')
          );
          if (
            isMissingContracts &&
            currentStory?.id &&
            currentChapter?.chapter_number !== undefined
          ) {
            toast.error(`写作前检查未通过：${firstIssue}。正在尝试自动补齐合同，请重试...`, {
              duration: 6000,
            });
          } else {
            toast.error(`写作前检查未通过：${firstIssue}`, { duration: 6000 });
          }
        } else if (msg.includes('超时') || msg.includes('timed out') || msg.includes('timeout')) {
          toast.error(`模型响应超时：${msg}\n请检查模型服务是否正常运行`, { duration: 6000 });
        } else {
          toast.error(`生成失败: ${msg}`);
        }
        setIsGenerating(false);
        setGenerationStatus('');
        setOrchestratorStatus(null);
      }
    },
    [isGenerating]
  );

  // Accept AI generation
  const handleAcceptGeneration = useCallback(() => {
    if (generatedText && editorRef.current) {
      // v0.7.4: 续写内容自动排版（智能分段 + 引号规范化）
      // v0.9.2-fix: 续写内容接受后始终追加到正文最后，避免插入光标处导致段落混乱
      const formatted = autoFormatText(generatedText);
      editorRef.current.appendText(formatted);
      if (currentStory?.id) {
        recordFeedback({
          story_id: currentStory.id,
          chapter_id: currentChapter?.id,
          feedback_type: 'accept',
          agent_type: 'writer',
          original_ai_text: generatedText,
        })
          .then(() => {
            toast.success('已记录接受偏好，系统将学习此方向');
          })
          .catch(e => frontstageLogger.error('Feedback record failed', { error: e }));
      } else {
        toast.success('已记录接受偏好');
      }
      setGeneratedText('');
    }
  }, [generatedText, currentStory, currentChapter]);

  // Reject AI generation
  const handleRejectGeneration = useCallback(() => {
    if (generatedText && currentStory?.id) {
      recordFeedback({
        story_id: currentStory.id,
        chapter_id: currentChapter?.id,
        feedback_type: 'reject',
        agent_type: 'writer',
        original_ai_text: generatedText,
      })
        .then(() => {
          toast.success('已记录拒绝偏好，系统将调整生成策略');
        })
        .catch(e => console.error('Feedback record failed:', e));
    } else {
      toast.success('已记录拒绝偏好');
    }
    setGeneratedText('');
  }, [generatedText, currentStory, currentChapter]);

  // 处理内联修改建议
  const handleInlineSuggestion = useCallback((suggestion: any, targetText: string) => {
    setInlineSuggestion({
      instruction: suggestion.instruction || '润色这段文字',
      targetText,
      category: suggestion.category,
      targetParagraphIndex: suggestion.targetParagraphIndex ?? -1,
    });
  }, []);

  // 取消生成引用
  const cancelGenerationRef = useRef<(() => void) | null>(null);
  // v5.4.0: 保存 GenesisPipeline session_id，用于取消后台任务
  const sessionIdRef = useRef<string | null>(null);

  // 取消当前生成
  const handleCancelGeneration = useCallback(async () => {
    if (cancelGenerationRef.current) {
      cancelGenerationRef.current();
      cancelGenerationRef.current = null;
    }
    // v0.11.5: 真正通知后端取消所有 Agent / LLM 任务，而不仅是清理前端状态
    try {
      await loggedInvoke<unknown>('agent_cancel_all_tasks', {});
    } catch (e) {
      frontstageLogger.error('Failed to cancel agent tasks', { error: e });
    }
    // v0.11.2: 清理所有残留的后台活动，避免取消后状态栏仍显示"系统正在处理中"
    useBackendActivityStore.getState().failAllRunning('用户已取消');
    // v5.4.0: 如果有 session_id，调用后端取消 GenesisPipeline
    if (sessionIdRef.current) {
      try {
        await loggedInvoke<unknown>('cancel_genesis_pipeline', {
          session_id: sessionIdRef.current,
        });
        // 使用底部状态栏替代黑色 toast
        setGenerationStatus('✓ 已取消生成并通知后端停止后台任务');
      } catch (e) {
        frontstageLogger.error('Failed to cancel genesis pipeline', { error: e });
        setGenerationStatus('✓ 已取消生成');
      }
      sessionIdRef.current = null;
    } else {
      setGenerationStatus('✓ 已取消生成');
    }
    setIsGenerating(false);
  }, []);

  // 检测用户输入是否是"创建新小说"意图（需要更长的超时）
  // v5.4.0: 增强检测，区分"创建新小说"和"续写当前故事"
  const isNovelCreationIntent = (input: string): boolean => {
    const txt = input.toLowerCase();
    // 明确的创建新小说意图词（必须包含至少一个）
    const creationSignals = [
      '写一部',
      '写一本',
      '写一篇',
      '写个',
      '创作一部',
      '创作一本',
      '创作一篇',
      '创作个',
      '生成一部',
      '生成一本',
      '生成一篇',
      '新建',
      '创建',
      '新开',
      'novel',
      'story',
      'book',
    ];
    const hasCreationSignal = creationSignals.some(kw => txt.includes(kw));
    if (!hasCreationSignal) return false;
    // 排除明确的续写意图词
    const continuationSignals = ['续写', '接着写', '往下写', '后面', '接下来', '继续', '后续'];
    const hasContinuationSignal = continuationSignals.some(kw => txt.includes(kw));
    // 如果同时包含创建信号和续写信号，优先判断为续写（用户说"续写一部小说"）
    if (hasContinuationSignal) return false;
    return true;
  };

  // v0.9.4: 检测用户输入是否明确为"续写/继续写"意图，用于显示更准确的初始提示
  const isContinuationIntent = (input: string): boolean => {
    const txt = input.toLowerCase();
    const continuationSignals = [
      '续写',
      '接着写',
      '往下写',
      '继续写',
      '继续',
      '后续',
      '后面',
      '接下来',
      '写下去',
      '往下续',
    ];
    return continuationSignals.some(kw => txt.includes(kw));
  };

  // 智能生成入口 -- 简化为直接调用后端 smart_execute
  const handleSmartGeneration = useCallback(
    async (userInput: string) => {
      if (isGenerating) {
        // 使用顶部状态栏替代黑色 toast
        setOrchestratorStatus({ stepType: 'busy', message: 'AI 正在生成中，请稍候...' });
        setTimeout(() => {
          setOrchestratorStatus(current =>
            current?.message === 'AI 正在生成中，请稍候...' ? null : current
          );
        }, 2000);
        return;
      }

      // 创建新小说涉及多步LLM调用（概念→正文→世界观→大纲→角色→场景→伏笔），本地模型可能需要5-10分钟
      // v5.4.0: 移除 stories.length === 0 限制，用户输入明确的创建意图时始终创建新小说
      const isBootstrap = isNovelCreationIntent(userInput);
      // v0.11.5: 前端超时统一缩短到 300 秒，避免用户空等 10 分钟。
      const timeoutSeconds = 300;
      const timeoutMs = timeoutSeconds * 1000;

      // v0.7.5: 非 Bootstrap 请求先执行预检；缺少合同/大纲时自动补齐
      if (!isBootstrap && currentStory?.id && currentChapter?.chapter_number !== undefined) {
        try {
          const preflight = await checkPreflight(currentStory.id, currentChapter.chapter_number);
          if (!preflight.ready) {
            const isMissingContracts = preflight.missing_contracts.length > 0;
            const isMissingOutline = preflight.blocking_issues.some(
              (i: string) => i.includes('大纲') || i.includes('outline')
            );
            if (isMissingContracts || isMissingOutline) {
              setIsGenerating(true);
              const missingItems: string[] = [];
              if (isMissingContracts) {
                if (preflight.missing_contracts.includes('MASTER_SETTING'))
                  missingItems.push('世界观合同');
                if (preflight.missing_contracts.some((c: string) => c.startsWith('CHAPTER_')))
                  missingItems.push('章节合同');
              }
              if (isMissingOutline) missingItems.push('场景大纲');
              const hintMsg = `检测到缺少 ${missingItems.join('、')}，系统正在自动补齐，请稍候...`;
              const loadingToastId = toast.loading(hintMsg, { duration: Infinity });
              setGenerationStatus(hintMsg);
              let progressUnlisten: (() => void) | null = null;
              try {
                progressUnlisten = await listen('contract-auto-progress', event => {
                  const p = event.payload as any;
                  setGenerationStatus(p.message);
                  const pct = Math.round((p.progress || 0) * 100);
                  toast.loading(`${p.message} (${pct}%)`, { id: loadingToastId });
                });
                const targetSceneId = currentScene?.id || currentChapter.scene_id;
                const result = await autoCreateMissingContracts(
                  currentStory.id,
                  currentChapter.chapter_number,
                  targetSceneId
                );
                if (
                  !result.created_master_setting &&
                  !result.created_chapter_contract &&
                  !result.created_outline
                ) {
                  toast.error(`自动补齐未成功（${missingItems.join('、')}），请手动创建`, {
                    id: loadingToastId,
                  });
                  setIsGenerating(false);
                  setGenerationStatus('');
                  return;
                }
                toast.success(`补齐完成（${missingItems.join('、')}），继续生成...`, {
                  id: loadingToastId,
                });
              } catch (e) {
                frontstageLogger.error('Auto creation failed', { error: e });
                toast.error('自动补齐失败，请手动创建', { id: loadingToastId });
                setIsGenerating(false);
                setGenerationStatus('');
                return;
              } finally {
                if (progressUnlisten) progressUnlisten();
              }
            } else {
              const issues =
                preflight.blocking_issues.length > 0
                  ? preflight.blocking_issues
                  : preflight.missing_contracts;
              const firstIssue = issues[0] || '写作前检查未通过';
              toast.error(`写作前检查未通过：${firstIssue}`, { duration: 6000 });
              return;
            }
          }
        } catch (e) {
          frontstageLogger.warn('Preflight check failed silently', { error: e });
        }
      }

      setIsGenerating(true);
      const isContinuation = isContinuationIntent(userInput);
      const initialStatusMsg = isBootstrap
        ? '🎨 正在构思故事概念...'
        : isContinuation
          ? '📝 正在续写...'
          : '💭 正在理解创作意图并执行...';
      setGenerationStatus(initialStatusMsg);
      startElapsedTimer();
      // 状态统一在底部 AI 编排器状态栏展示
      currentToastPhaseRef.current = initialStatusMsg;

      // 方案A：前端动态超时 + 取消支持
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      let aborted = false;

      const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          aborted = true;
          useBackendActivityStore.getState().failAllRunning('前端超时：模型未在 300 秒内响应');
          reject(
            new Error(
              isBootstrap
                ? `前端超时：模型响应超过${timeoutSeconds / 60}分钟。创建新小说需要多次LLM调用，本地模型可能较慢。请检查模型服务是否正常运行，或尝试简化输入。`
                : `前端超时：模型响应超过${timeoutSeconds}秒（5分钟）。本地模型生成长文本可能耗时较长，请检查模型是否仍在运行。`
            )
          );
        }, timeoutMs);
      });

      // 暴露取消函数
      cancelGenerationRef.current = () => {
        aborted = true;
        if (timeoutId) clearTimeout(timeoutId);
      };

      try {
        const result = await Promise.race([
          smartExecute({
            user_input: userInput,
            current_content: editorRef.current?.getText(),
            style_weight: 50,
          }),
          timeoutPromise,
        ]);

        if (timeoutId) clearTimeout(timeoutId);
        if (aborted) {
          stopElapsedTimer();
          setIsGenerating(false);
          setGenerationStatus('');
          return;
        }

        currentToastPhaseRef.current = null;

        // v0.9.5: 区分后台生成中的 Bootstrap 与已完成的 Bootstrap
        const isBackgroundBootstrap = result.messages.some(
          m => m === 'novel_bootstrap_background_started'
        );
        const isBootstrapCompleted =
          !isBackgroundBootstrap && result.messages.some(m => m.includes('novel_bootstrap'));

        if (isBackgroundBootstrap) {
          frontstageLogger.info('[SmartGeneration] Story created, first chapter in background');
          toast.success('故事已创建，第一章正在后台生成，完成后会自动加载', {
            duration: 5000,
          });
        }

        // 关键修复：空字符串在JS中是falsy，必须显式检查trim后的长度
        const hasContent = result.final_content && result.final_content.trim().length > 0;
        if (hasContent) {
          // v5.3.1 修复：Bootstrap 完成时内容已通过 ChapterSwitch 加载到编辑器，
          // 不要设置 generatedText，否则会出现正文+幽灵文本两份内容
          if (isBootstrapCompleted) {
            toast.success('小说已创建！第一章已生成，您可以开始写作了');
          } else {
            // v5.4.0: 去除与当前编辑器内容重复的部分，防止 LLM 返回完整文本导致"重复输出"
            let finalContent = result.final_content!;
            const currentText = editorRef.current?.getText() || '';
            if (currentText && finalContent.startsWith(currentText)) {
              finalContent = finalContent.slice(currentText.length).trimStart();
              frontstageLogger.info(
                '[SmartGeneration] Removed duplicate prefix from generated text',
                {
                  prefix_len: currentText.length,
                  remaining_len: finalContent.length,
                }
              );
            }
            setGeneratedText(finalContent);
            toast.success('创作完成！');
          }
        } else if (isBackgroundBootstrap) {
          // 后台生成中，final_content 为空是预期行为，已在上文提示用户
        } else if (!result.success) {
          // 后端返回了失败
          toast.error('创作失败：AI 未能生成内容，请检查模型配置或稍后重试');
        } else {
          // 后端返回了成功但没有内容 — 显示明确的错误提示（修复"没有提示地停止"）
          toast.error('AI 返回了空内容，请检查模型配置或稍后重试', { duration: 5000 });
          frontstageLogger.error(
            '[SmartGeneration] Backend returned success=true but empty final_content',
            { result }
          );
        }

        // v5.4.1: Bootstrap 完成后直接加载新故事内容，不再完全依赖 ChapterSwitch 事件
        const storyCreatedMsg = result.messages.find(m => m.startsWith('story_created:'));
        if (storyCreatedMsg) {
          const storyId = storyCreatedMsg.replace('story_created:', '');
          frontstageLogger.info('[SmartGeneration] New story created, fetching content directly', {
            story_id: storyId,
          });
          // 直接加载新创建的故事和章节
          (async () => {
            try {
              const allStories = await loggedInvoke<Story[]>('list_stories');
              const targetStory = allStories.find(s => s.id === storyId);
              if (targetStory) {
                const storyChapters = await loggedInvoke<Chapter[]>('get_story_chapters', {
                  story_id: storyId,
                });
                const storyScenes = await loggedInvoke<Scene[]>('get_story_scenes', {
                  story_id: storyId,
                });
                const firstChapter = storyChapters[0];
                frontstageLogger.info('[SmartGeneration] Loaded new story', {
                  story_id: storyId,
                  chapter_count: storyChapters.length,
                  first_chapter_id: firstChapter?.id,
                  first_chapter_content_length: firstChapter?.content?.length ?? 0,
                  first_chapter_content_preview: firstChapter?.content?.slice(0, 100) ?? 'EMPTY',
                });
                setCurrentStory(targetStory);
                setChapters(storyChapters);
                setScenes(storyScenes);
                if (storyChapters.length > 0) {
                  frontstageLogger.info('[SmartGeneration] Calling selectChapter', {
                    chapter_id: storyChapters[0].id,
                    content_length: storyChapters[0].content?.length ?? 0,
                  });
                  selectChapter(storyChapters[0]);
                  // v5.4.1 fix: 双重保险——如果 DB 返回的 content 为空但 result.final_content 有内容，直接使用 final_content
                  if (
                    (!firstChapter?.content || firstChapter.content.trim().length === 0) &&
                    result.final_content &&
                    result.final_content.trim().length > 0
                  ) {
                    frontstageLogger.warn(
                      '[SmartGeneration] DB chapter content is empty but final_content exists, using final_content as fallback'
                    );
                    setContent(autoFormatText(result.final_content));
                  }
                } else if (result.final_content && result.final_content.trim().length > 0) {
                  // v5.4.1 fix: 极端情况——DB 中没有章节但 result.final_content 有内容，直接显示
                  frontstageLogger.warn(
                    '[SmartGeneration] No chapters in DB but final_content exists, displaying content directly'
                  );
                  setContent(autoFormatText(result.final_content));
                }
              } else {
                frontstageLogger.error('[SmartGeneration] New story not found in list_stories', {
                  story_id: storyId,
                });
              }
            } catch (e) {
              frontstageLogger.error('[SmartGeneration] Failed to load new story', { error: e });
            }
          })();
        }
        // v5.4.0: 保存 session_id 用于取消后台任务
        const sessionIdMsg = result.messages.find(m => m.startsWith('session_id:'));
        if (sessionIdMsg) {
          sessionIdRef.current = sessionIdMsg.replace('session_id:', '');
        }
      } catch (e: any) {
        if (timeoutId) clearTimeout(timeoutId);
        currentToastPhaseRef.current = null;
        // v0.11.5: 异常时清理 backendActivityStore，避免状态栏卡死
        useBackendActivityStore
          .getState()
          .failAllRunning(
            e?.message?.includes('超时') ||
              e?.message?.includes('timed out') ||
              e?.message?.includes('timeout')
              ? '模型响应超时'
              : '执行失败'
          );
        frontstageLogger.error('Smart execution failed', { error: e });
        const structured = parseStructuredError(e);
        const msg = e?.message || String(e);
        if (structured?.code === 'PREFLIGHT_FAILED') {
          const issues = (structured.data?.issues as string[]) || [];
          const firstIssue = issues[0] || structured.message || '写作前检查未通过';
          toast.error(
            `写作前检查未通过：${firstIssue}。请在「幕后 → StorySystem → 合同」中创建世界观合同和章节合同`,
            { duration: 6000 }
          );
        } else if (msg.includes('超时') || msg.includes('timed out') || msg.includes('timeout')) {
          toast.error(`模型响应超时：${msg}\n请检查模型服务是否正常运行`, { duration: 6000 });
        } else {
          toast.error(`执行失败: ${msg}`);
        }
      } finally {
        stopElapsedTimer();
        cancelGenerationRef.current = null;
        currentToastPhaseRef.current = null;
        setIsGenerating(false);
        setOrchestratorStatus(null);
        // v5.4.1 修复：Bootstrap 场景下保留后台状态提示，不要直接清空
        // 后台阶段完成/失败时会通过 novel-bootstrap-progress / novel-bootstrap-error 事件自动清空
        if (isBootstrap) {
          setGenerationStatus('后台正在完善小说世界...');
        } else {
          setGenerationStatus('');
        }
      }
    },
    [isGenerating]
  );

  // 底部输入栏提交
  const handleInputSubmit = useCallback(() => {
    const text = inputValue.trim();
    if (!text) return;
    // 保存到历史
    setInputHistory(prev => {
      const filtered = prev.filter(h => h !== text);
      return [text, ...filtered].slice(0, 20);
    });
    setGhostHint('');
    setHistoryIndex(-1);
    handleSmartGeneration(text);
    setInputValue('');
  }, [inputValue, handleSmartGeneration]);

  // 获取LLM智能输入建议
  const fetchSmartHint = useCallback(async () => {
    if (!currentChapter) return;
    try {
      const hint = await getInputHint(editorRef.current?.getText());
      if (hint && !inputValue) {
        setGhostHint(hint);
        setHintSource('llm');
        setHistoryIndex(-1);
      }
    } catch (e) {
      frontstageLogger.error('Failed to fetch input hint', { error: e });
    }
  }, [currentChapter]);

  // v0.11.0: 模型状态统一由 modelConnectionStore 轮询驱动，状态栏直接展示当前可用模型
  // 保留一次旧版兜底检测，兼容后端未返回新模型配置时的降级
  useEffect(() => {
    const runLegacyCheck = async () => {
      try {
        await modelService.checkModelStatus();
      } catch (e) {
        frontstageLogger.warn('Legacy model status check failed', { error: e });
      }
    };
    runLegacyCheck();
  }, []);

  // 输入栏获得焦点时获取智能建议
  const handleInputFocus = useCallback(() => {
    if (!inputValue && !ghostHint) {
      fetchSmartHint();
    }
  }, [inputValue, ghostHint, fetchSmartHint]);

  const handleInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter 发送
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleInputSubmit();
        return;
      }
      // ↑ 键：切换显示 ghost hint（LLM建议 → 历史记录）
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (hintSource === 'llm' && inputHistory.length > 0) {
          // 切换到第一条历史
          setHintSource('history');
          setHistoryIndex(0);
          setGhostHint(inputHistory[0]);
        } else if (hintSource === 'history' && historyIndex < inputHistory.length - 1) {
          // 下一条历史
          const nextIdx = historyIndex + 1;
          setHistoryIndex(nextIdx);
          setGhostHint(inputHistory[nextIdx]);
        } else if (hintSource === 'history') {
          // 循环回到 LLM 建议
          setHintSource('llm');
          setHistoryIndex(-1);
          fetchSmartHint();
        } else {
          // 当前是LLM建议但没有历史，重新获取
          fetchSmartHint();
        }
        return;
      }
      // ↓ 键：从历史回到 LLM 建议
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (hintSource === 'history') {
          if (historyIndex > 0) {
            const prevIdx = historyIndex - 1;
            setHistoryIndex(prevIdx);
            setGhostHint(inputHistory[prevIdx]);
          } else {
            setHintSource('llm');
            setHistoryIndex(-1);
            fetchSmartHint();
          }
        }
        return;
      }
      // → 键：确认填充 ghost hint
      if (e.key === 'ArrowRight' && ghostHint && !inputValue) {
        e.preventDefault();
        setInputValue(ghostHint);
        setGhostHint('');
        setHistoryIndex(-1);
        setHintSource('llm');
        return;
      }
      // 任意键输入时清除 ghost hint
      if (e.key.length === 1 && ghostHint) {
        setGhostHint('');
        setHistoryIndex(-1);
        setHintSource('llm');
      }
    },
    [
      handleInputSubmit,
      ghostHint,
      inputValue,
      hintSource,
      historyIndex,
      inputHistory,
      fetchSmartHint,
    ]
  );

  // Pipeline 命令处理
  const handlePipelineRefine = useCallback(async () => {
    if (!currentStory?.id || !currentChapter) {
      toast.error('请先选择故事和章节');
      return;
    }
    try {
      toast.loading('正在执行 AI 修稿...', { id: 'pipeline-refine' });
      const draft = await getPipelineActiveDraft(currentStory.id, currentChapter.chapter_number);
      if (!draft) {
        toast.error('当前章节没有活跃草稿', { id: 'pipeline-refine' });
        return;
      }
      const result = await runRefine(currentStory.id, draft.id, undefined);
      toast.success(`修稿完成：${result.change_summary || '已生成修订版本'}`, {
        id: 'pipeline-refine',
      });
      // 刷新编辑器内容
      if (result.refined_content) {
        editorRef.current?.setContent(result.refined_content);
      }
    } catch (e: any) {
      toast.error('修稿失败: ' + (e.message || String(e)), { id: 'pipeline-refine' });
    }
  }, [currentStory, currentChapter]);

  const handlePipelineReview = useCallback(async () => {
    if (!currentStory?.id || !currentChapter) {
      toast.error('请先选择故事和章节');
      return;
    }
    try {
      toast.loading('正在执行 AI 审稿...', { id: 'pipeline-review' });
      const draft = await getPipelineActiveDraft(currentStory.id, currentChapter.chapter_number);
      if (!draft) {
        toast.error('当前章节没有活跃草稿', { id: 'pipeline-review' });
        return;
      }
      const result = await runReview(currentStory.id, draft.id, undefined);
      toast.success(`审稿完成：综合评分 ${result.overall_score}分`, { id: 'pipeline-review' });
    } catch (e: any) {
      toast.error('审稿失败: ' + (e.message || String(e)), { id: 'pipeline-review' });
    }
  }, [currentStory, currentChapter]);

  const handlePipelineFinalize = useCallback(async () => {
    if (!currentStory?.id || !currentChapter) {
      toast.error('请先选择故事和章节');
      return;
    }
    try {
      toast.loading('正在定稿...', { id: 'pipeline-finalize' });
      const draft = await getPipelineActiveDraft(currentStory.id, currentChapter.chapter_number);
      if (!draft) {
        toast.error('当前章节没有活跃草稿', { id: 'pipeline-finalize' });
        return;
      }
      await runFinalize(
        currentStory.id,
        draft.id,
        currentChapter.chapter_number,
        currentChapter.title
      );
      toast.success('定稿完成，后处理已启动', { id: 'pipeline-finalize' });
    } catch (e: any) {
      toast.error('定稿失败: ' + (e.message || String(e)), { id: 'pipeline-finalize' });
    }
  }, [currentStory, currentChapter]);

  // 处理编辑器 Slash 命令
  const handleSlashCommand = useCallback((commandId: string) => {
    if (commandId === 'auto_write') {
      setWenSiTab('write');
      setShowWenSiPanel(true);
    } else if (commandId === 'auto_revise') {
      setWenSiTab('revise');
      setShowWenSiPanel(true);
    } else if (commandId === 'dialog') {
      setWenSiTab('dialog');
      setShowWenSiPanel(true);
    } else if (commandId === 'pipeline_refine') {
      handlePipelineRefine();
    } else if (commandId === 'pipeline_review') {
      handlePipelineReview();
    } else if (commandId === 'pipeline_finalize') {
      handlePipelineFinalize();
    }
  }, []);

  // 全局快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // F11 禅模式
      if (e.key === 'F11') {
        e.preventDefault();
        setIsZenMode(prev => !prev);
        return;
      }
      // Ctrl+Enter / Cmd+Enter 续写（智能判断：非active时自动切换）
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !isZenMode) {
        e.preventDefault();
        handleRequestGeneration('');
        return;
      }
      // Ctrl+Shift+B 快速切换到幕后工作室并定位当前故事
      if (e.key === 'B' && e.ctrlKey && e.shiftKey && !isZenMode) {
        e.preventDefault();
        openBackstage();
        return;
      }
    };

    // F1 帮助面板
    const handleF1 = (e: KeyboardEvent) => {
      if (e.key === 'F1') {
        e.preventDefault();
        setShowHelpPanel(prev => !prev);
      }
    };
    window.addEventListener('keydown', handleF1);

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keydown', handleF1);
    };
  }, [wensiMode, isZenMode, handleRequestGeneration]);

  // Calculate total story word count
  const totalWordCount = chapters.reduce((sum, c) => {
    const text = c.content || '';
    const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
    const englishWords = (text.match(/[a-zA-Z]+/g) || []).length;
    return sum + chineseChars + englishWords;
  }, 0);

  return (
    <div className={`frontstage-container ${isZenMode ? 'zen-mode' : ''}`}>
      <FrontstageHeader
        currentStory={currentStory}
        currentChapter={currentChapter}
        wordCount={wordCount}
        totalWordCount={totalWordCount}
        fontSize={fontSize}
        isSaved={isSaved}
        isZenMode={isZenMode}
        wensiMode={wensiMode}
        orchestratorStatus={orchestratorStatus}
        bootstrapProgress={bootstrapProgress}
        onOpenBackstage={openBackstage}
        onCycleWensiMode={cycleWensiMode}
        onToggleZenMode={() => setIsZenMode(prev => !prev)}
      />

      {/* Main Content */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        {/* Editor + Bottom Input */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          <main className="frontstage-main" style={{ flex: 1, minHeight: 0 }}>
            {currentChapter && (
              <div className="chapter-header">
                <h1 className="chapter-title">
                  {currentChapter.title || `第${currentChapter.chapter_number}章`}
                </h1>
              </div>
            )}

            <RichTextEditor
              ref={editorRef}
              content={content}
              onChange={handleContentChange}
              wensiMode={wensiMode}
              generatedText={generatedText}
              isGenerating={isGenerating}
              onAcceptGeneration={handleAcceptGeneration}
              onRejectGeneration={handleRejectGeneration}
              onRequestGeneration={handleRequestGeneration}
              onSmartGeneration={handleSmartGeneration}
              onSlashCommand={handleSlashCommand}
              onShowStatus={message => {
                setOrchestratorStatus({ stepType: 'editor', message });
                setTimeout(() => {
                  setOrchestratorStatus(current => (current?.message === message ? null : current));
                }, 3000);
              }}
              placeholder="开始写作..."
              characters={characters}
              fontSize={fontSize}
              onFontSizeChange={setFontSize}
              isZenMode={isZenMode}
              onZenModeChange={setIsZenMode}
              storyId={currentStory?.id}
              chapterId={currentChapter?.id}
              chapterNumber={currentChapter?.chapter_number}
              smartGhostText={smartGhostText}
              inlineSuggestion={subscription.isPro ? inlineSuggestion : null}
              onClearInlineSuggestion={() => setInlineSuggestion(null)}
              subscription={subscription}
            />
          </main>

          <FrontstageBottomBar
            isZenMode={isZenMode}
            isGenerating={isGenerating}
            generationStatus={generationStatus}
            inputValue={inputValue}
            ghostHint={ghostHint}
            hintSource={hintSource}
            modelStatus={modelStatus}
            modelName={modelName}
            modelProvider={activeChatModel?.provider}
            modelApiBase={activeChatModel?.api_base}
            modelLatency={chatConnectionState?.result?.latency}
            lastCheckedAt={chatConnectionState?.lastCheckedAt}
            onGoToSettings={openBackstage}
            onInputChange={setInputValue}
            onInputSubmit={handleInputSubmit}
            onCancelGeneration={handleCancelGeneration}
            onInputFocus={handleInputFocus}
            onInputKeyDown={handleInputKeyDown}
          />
        </div>
      </div>

      {/* Floating WenSi Panel */}
      {showWenSiPanel && (
        <div className="fixed bottom-6 right-6 w-[420px] max-w-[calc(100vw-3rem)] z-40">
          <div className="bg-[var(--parchment-dark)] border border-[var(--warm-sand)] rounded-xl shadow-2xl overflow-hidden">
            <div className="flex items-center justify-between px-4 py-2.5 border-b border-[var(--warm-sand)]">
              <span className="text-sm font-medium text-[var(--charcoal)]">文思泉涌</span>
              <button
                onClick={() => setShowWenSiPanel(false)}
                className="text-[var(--stone-gray)] hover:text-[var(--charcoal)] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-3">
              <WenSiPanel
                storyId={currentStory?.id}
                chapterId={currentChapter?.id}
                isPro={subscription?.isPro ?? false}
                onShowUpgrade={trigger => {
                  setUpgradeTrigger(trigger);
                  setShowUpgradePanel(true);
                }}
                hasAutoWriteQuota={subscription?.hasAutoWriteQuota || (async () => true)}
                hasAutoReviseQuota={subscription?.hasAutoReviseQuota || (async () => true)}
                editorContent={editorRef.current?.getText()}
                selectedText={editorRef.current?.getSelectedText()}
                onShowStatus={message => {
                  setOrchestratorStatus({ stepType: 'wensi', message });
                  setTimeout(() => {
                    setOrchestratorStatus(current =>
                      current?.message === message ? null : current
                    );
                  }, 3000);
                }}
                onReviseResult={text => {
                  if (editorRef.current) {
                    // v0.7.4: 修稿结果自动排版（智能分段 + 引号规范化）
                    const html = autoFormatText(text);
                    editorRef.current.insertText(html);
                    toast.success('修改内容已应用到编辑器');
                  }
                }}
                onFreePrompt={prompt => {
                  handleSmartGeneration(prompt);
                  setShowWenSiPanel(false);
                }}
              />
            </div>
          </div>
        </div>
      )}

      {/* F1 帮助面板 */}
      {showHelpPanel && !isZenMode && (
        <div className="fixed top-16 left-1/2 -translate-x-1/2 z-50">
          <div className="frontstage-help-panel">
            <div className="frontstage-help-header">
              <span className="text-sm font-medium">快捷键指南</span>
              <button
                onClick={() => setShowHelpPanel(false)}
                className="text-[var(--stone-gray)] hover:text-[var(--charcoal)] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="frontstage-help-body">
              <div className="frontstage-help-section">
                <h4>写作</h4>
                <div className="frontstage-help-row">
                  <kbd>Ctrl</kbd>+<kbd>Enter</kbd>
                  <span>AI 续写</span>
                </div>
                <div className="frontstage-help-row">
                  <kbd>/</kbd>
                  <span>输入任意指令</span>
                </div>
                <div className="frontstage-help-row">
                  <kbd>Tab</kbd>
                  <span>接受 AI 建议</span>
                </div>
                <div className="frontstage-help-row">
                  <kbd>Esc</kbd>
                  <span>拒绝 AI 建议</span>
                </div>
              </div>
              <div className="frontstage-help-section">
                <h4>模式</h4>
                <div className="frontstage-help-row">
                  <kbd>Ctrl</kbd>+<kbd>Space</kbd>
                  <span>循环文思模式</span>
                </div>
                <div className="frontstage-help-row">
                  <kbd>F11</kbd>
                  <span>禅模式</span>
                </div>
                <div className="frontstage-help-row">
                  <kbd>F1</kbd>
                  <span>本帮助面板</span>
                </div>
              </div>
              <div className="frontstage-help-section">
                <h4>操作</h4>
                <div className="frontstage-help-row">
                  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>B</kbd>
                  <span>回幕后工作室</span>
                </div>
                <div className="frontstage-help-row">
                  <span className="no-kbd">点击标题</span>
                  <span>回幕后工作室</span>
                </div>
                <div className="frontstage-help-row">
                  <span className="no-kbd">修 / 批 / 幕</span>
                  <span>侧边栏快捷按钮</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 智能文思 — 统一提示系统 */}
      <SmartHintSystem
        htmlContent={content}
        isEnabled={!isZenMode && wensiMode !== 'off'}
        isZenMode={isZenMode}
        onGhostSuggestion={setSmartGhostText}
        onInlineSuggestion={subscription.isPro ? handleInlineSuggestion : undefined}
        subscription={subscription}
      />

      {/* 付费引导面板 */}
      <UpgradePanel
        isOpen={showUpgradePanel}
        onClose={() => setShowUpgradePanel(false)}
        trigger={upgradeTrigger}
        onUpgraded={() => subscription.fetchStatus()}
      />

      {/* 禅模式退出提示 */}
      {isZenMode && (
        <button onClick={() => setIsZenMode(false)} className="zen-mode-exit">
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3M3 16h3a2 2 0 0 1 2 2v3" />
          </svg>
          退出禅模式 (F11)
        </button>
      )}

      {/* 故事上下文信息已整合至编辑器侧边栏与幕后工作室 */}
    </div>
  );
};

export default FrontstageApp;
