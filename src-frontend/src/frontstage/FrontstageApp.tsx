import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { loggedInvoke } from '@/services/tauri';
import { invoke } from '@tauri-apps/api/core';
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
import { useFrontstageStore } from './store/frontstageStore';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import { useSettings, useModels } from '@/hooks/useSettings';
import { useModelConnectionStore } from '@/stores/modelConnectionStore';
import { useGenerationStore } from '@/stores/generationStore';
import { useBootstrapStore } from '@/stores/bootstrapStore';
import { useQueryClient, useQuery } from '@tanstack/react-query';
import type { Scene } from '@/types/v3';
import type { GatewayStatus } from '@/types/llm';
import { useSubscription } from '@/hooks/useSubscription';
import { usePipelineProgress, usePipelineComplete } from '@/hooks/usePipelineProgress';
import { useBackendActivityListener } from '@/hooks/useBackendActivityListener';
import { useDbPoolStatus } from '@/hooks/useDbPoolStatus';
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

// B2: 分页初始窗口——章节加载当前章及前后各 1 章，场景加载当前及附近共 5 个
const CHAPTERS_PAGE_SIZE = 3;
const SCENES_PAGE_SIZE = 5;

// A4-1.7: 智能创作精确阶段映射（组件外常量，避免 hook deps 警告）
const PRECISE_PHASE_PATTERNS: { phase: string; patterns: string[] }[] = [
  {
    phase: '准备上下文',
    patterns: [
      '准备上下文',
      'preparing_context',
      'prepare_context',
      'loading_context',
      '加载上下文',
      '读取故事',
      '读取章节',
    ],
  },
  {
    phase: '候选生成',
    patterns: ['候选生成', 'candidate', 'candidates', 'generating_candidates', '生成候选'],
  },
  {
    phase: '内容审校',
    patterns: [
      '内容审校',
      'inspector',
      '质检',
      'inspecting',
      'inspect',
      'inspection',
      'review',
      '审校',
    ],
  },
  { phase: '改写', patterns: ['改写', '润色改写', 'rewrite', 'rewriting', 'revise', '润色'] },
  {
    phase: '最终输出',
    patterns: ['最终输出', '已完成', 'final_output', 'finalize', '最终', 'final output'],
  },
  { phase: '保存记忆', patterns: ['保存记忆', 'save_memory', 'saving_memory', 'memory', '记忆'] },
];

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
  // [DEBUG-dup] 统计创世过程中 ChapterSwitch 事件触发次数
  const chapterSwitchCountRef = useRef(0);
  // v0.23.62: 追踪 ChapterSwitch 时间戳，用于 post-ChapterSwitch 5s 静默期
  const chapterSwitchTimestampRef = useRef(0);
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
  // W2-F1: 编辑中内容与保存状态由 frontstageStore 作为唯一可写源。
  const content = useFrontstageStore(state => state.content);
  const setContent = useFrontstageStore(state => state.setContent);
  const isSaved = useFrontstageStore(state => state.isSaved);
  const setSaveStatus = useFrontstageStore(state => state.setSaveStatus);
  const setChapterInfo = useFrontstageStore(state => state.setChapterInfo);
  const setIsSaved = useCallback(
    (saved: boolean) => setSaveStatus(saved, saved ? new Date().toISOString() : null),
    [setSaveStatus]
  );
  const isSavedRef = useRef(isSaved);
  useEffect(() => {
    isSavedRef.current = isSaved;
  }, [isSaved]);
  const [generatedText, _setGeneratedText] = useState('');
  // v0.23.66: 包装 setGeneratedText，每次非空赋值时记录调用栈便于诊断重复根因
  const setGeneratedText = (text: string) => {
    if (text && text.length > 50) {
      const stack = new Error().stack?.split('\n').slice(1, 5).join(' <- ');
      frontstageLogger.info('[GEN-TEXT] setGeneratedText non-empty', {
        textLen: text.length,
        preview: text.slice(0, 80),
        stack: stack || 'unknown',
      });
    } else if (text === '') {
      frontstageLogger.info('[GEN-TEXT] setGeneratedText cleared');
    }
    _setGeneratedText(text);
  };
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
        loadStoryScenes(storyId).then(() => loadStoryWordCount(storyId));
      }
    },
    onSceneUpdated: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryScenes(storyId).then(() => loadStoryWordCount(storyId));
      }
    },
    onSceneDeleted: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryScenes(storyId).then(() => loadStoryWordCount(storyId));
      }
    },
    // v5.4.0: 监听 chapter 创建/删除（幕后增删章节后同步幕前列表）
    onChapterCreated: storyId => {
      if (currentStory && storyId === currentStory.id) {
        loadStoryChapters(storyId).then(() => loadStoryWordCount(storyId));
      }
    },
    onChapterDeleted: () => {
      if (currentStory) {
        const storyId = currentStory.id;
        loadStoryChapters(storyId).then(() => loadStoryWordCount(storyId));
      }
    },
    // v5.2.0: 监听 chapter 更新（幕后修改后同步到幕前）
    onChapterUpdated: (chapterId, title) => {
      // v0.23.52: 生成进行中时跳过后台 auto_commit 的 chapterUpdated 事件，
      // 避免后台保存与 ChapterSwitch/编辑器加载正文竞争，覆盖刚写入的正文。
      if (useGenerationStore.getState().isGenerating) {
        frontstageLogger.info('[onChapterUpdated] Skipped during generation', { chapterId });
        return;
      }
      // v0.23.62: ChapterSwitch 后 5s 静默期，auto_commit 触发的 chapterUpdated
      // 不得覆盖刚加载的正文（会造成有排版/无排版两份重复内容的根因）。
      if (Date.now() - chapterSwitchTimestampRef.current < 5000) {
        frontstageLogger.info('[onChapterUpdated] Skipped in post-ChapterSwitch quiet period', {
          chapterId,
          msSinceChapterSwitch: Date.now() - chapterSwitchTimestampRef.current,
        });
        return;
      }
      if (currentChapter && chapterId === currentChapter.id) {
        // 如果刚在 3 秒内自动保存过，忽略这次更新（避免循环）
        if (Date.now() - justSavedRef.current < 3000) {
          return;
        }
        // 静默刷新当前 chapter 内容
        // v0.23.54: 后台同步内容用 autoFormatText 格式化，否则纯文本会覆盖
        // 刚通过 ChapterSwitch 加载的 HTML 排版内容
        (async () => {
          try {
            const updated = await loggedInvoke<Chapter | null>('get_chapter', { id: chapterId });
            if (updated && updated.content !== undefined) {
              const dbContent = updated.content || '';
              // v0.23.62: 与编辑器当前内容比较，相同则跳过（避免空操作触发渲染）
              const currentEditorText = editorRef.current?.getText() || '';
              if (dbContent.trim() === currentEditorText.trim()) {
                frontstageLogger.info('[onChapterUpdated] DB content === editor, skipping');
                return;
              }
              const formatted = autoFormatText(dbContent);
              // v0.23.62: 安全包装，防止 autoFormatText 异常导致白屏
              if (!formatted || formatted.trim().length === 0) {
                frontstageLogger.warn('[onChapterUpdated] autoFormatText returned empty');
                return;
              }
              setContent(prev => {
                if (prev !== formatted) {
                  // 使用底部状态栏替代黑色 toast
                  setGenerationStatus('📝 幕后已更新本章内容');
                  setTimeout(() => {
                    setGenerationStatus(current =>
                      current === '📝 幕后已更新本章内容' ? '' : current
                    );
                  }, 2000);
                }
                return formatted;
              });
            }
          } catch (e) {
            frontstageLogger.error('Failed to refresh chapter content', { error: e });
          }
        })();
      }
    },
    // 分时架构时间线 2：AI 审计 annotation 回流，首次出现时引导
    onAnnotationCreated: () => {
      const guided = localStorage.getItem('sf_audit_guided');
      if (!guided) {
        localStorage.setItem('sf_audit_guided', '1');
        toast.success(
          'AI 已在后台完成审计，发现的问题会以标注形式出现在正文中。点击顶部债务计数器可回幕后处置。',
          {
            duration: 6000,
          }
        );
      }
    },
  });

  // B1: 高频生成状态迁移到独立 Zustand store，避免单点重渲染
  const isGenerating = useGenerationStore(s => s.isGenerating);
  const generationStatus = useGenerationStore(s => s.generationStatus);
  const orchestratorStatus = useGenerationStore(s => s.orchestratorStatus);
  const bootstrapProgress = useBootstrapStore(s => s.bootstrapProgress);
  const setIsGenerating = useGenerationStore(s => s.setIsGenerating);
  const setGenerationStatus = useGenerationStore(s => s.setGenerationStatus);
  const setOrchestratorStatus = useGenerationStore(s => s.setOrchestratorStatus);
  const setBootstrapProgress = useBootstrapStore(s => s.setBootstrapProgress);

  // v0.23.54: 安全网已在 ChapterSwitch 处理器中清空 generatedText（正文加载到
  // 编辑器的时刻）。此处不再用 isGenerating 转换检测，因为会误伤正常续写流程
  // （续写时先 setGeneratedText 再 setIsGenerating(false)，转换检测会清掉合法幽灵文本）。

  // B1: 全文字数状态；输入时基于当前章节字数增量 diff 更新，避免每次渲染全量 reduce
  const [totalWordCount, setTotalWordCount] = useState(0);
  const currentChapterPrevWordCountRef = useRef(0);

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
    // A4-1.7: 统一精确阶段映射
    if (s.includes('准备上下文')) {
      return { icon: '📂', text: '准备上下文...' };
    }
    if (s.includes('候选生成')) {
      return { icon: '✍️', text: '候选生成...' };
    }
    if (s.includes('内容审校') || s.includes('审校')) {
      return { icon: '🔍', text: '内容审校...' };
    }
    if (s.includes('改写')) {
      return { icon: '✏️', text: '改写...' };
    }
    if (s.includes('最终输出')) {
      return { icon: '📤', text: '最终输出...' };
    }
    if (s.includes('保存记忆')) {
      return { icon: '💾', text: '保存记忆...' };
    }
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
    if (s.includes('分析故事上下文') || s.includes('生成执行计划') || s.includes('context')) {
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
          // v0.13.2: smart_execute 仍在飞行中则不清空
          if (smartExecuteInFlightRef.current) return prev;
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

  // v0.13.3: 诊断卡片安全网——任何非用户取消的异常结束都兜底弹出诊断卡片
  useEffect(() => {
    if (!isGenerating && smartExecuteNeedDiagnosticRef.current) {
      smartExecuteNeedDiagnosticRef.current = false;
      if (!lastGenerationCancelledRef.current) {
        captureDiagnosticInfo('生成过程异常结束，未收到有效内容');
      }
      lastGenerationCancelledRef.current = false;
    }
  }, [isGenerating]);

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

  // v0.14.0: 通过模型网关获取所有启用模型的健康状态
  const {
    data: gatewayStatus,
    isLoading: isGatewayLoading,
    refetch: refetchGateway,
  } = useQuery<GatewayStatus>({
    queryKey: ['gateway-status'],
    queryFn: async () => {
      return await loggedInvoke<GatewayStatus>('get_gateway_status');
    },
    refetchInterval: 30000,
    staleTime: 10000,
  });

  // v0.23.20: DB 连接池状态轮询（5s），状态栏显示连接池耗尽预警
  const { data: dbPoolStatus } = useDbPoolStatus();

  // v0.11.0: 启动模型连接轮询，状态栏展示当前可用模型与连接质量
  useEffect(() => {
    const enabledIds = allModels.filter(m => m.enabled).map(m => m.id);
    if (enabledIds.length === 0) return;
    const stop = startPolling(enabledIds, 30000);
    return () => stop();
  }, [allModels, startPolling]);

  // v0.23.8: 监听后端发给 LLM 的提示词通知，再从命令读取完整 prompt，避免事件 payload 过大丢失
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<{
        request_id: string;
        model_id: string;
        model_name: string;
        provider: string;
        prompt_preview: string;
        prompt_chars: number;
        prompt_tokens: number;
      }>('llm-prompt-sent', async event => {
        lastLlmModelRef.current =
          event.payload.model_id || event.payload.model_name
            ? `${event.payload.model_name} (${event.payload.provider})`
            : '未知';
        // 先展示预览，避免命令异步读取期间诊断卡片显示“未捕获”
        lastLlmPromptRef.current = event.payload.prompt_preview
          ? `${event.payload.prompt_preview}\n...（正在读取完整提示词）`
          : '正在读取完整提示词...';
        try {
          const full = await invoke<{
            request_id: string;
            context_label: string;
            model_id: string;
            model_name: string;
            provider: string;
            prompt: string;
            prompt_chars: number;
            prompt_tokens: number;
            updated_at: string;
          } | null>('get_last_llm_prompt');
          if (full && full.prompt) {
            lastLlmPromptRef.current = full.prompt;
            lastLlmModelRef.current = `${full.model_name} (${full.provider})`;
          }
        } catch (e) {
          frontstageLogger.warn('[llm-prompt-sent] 读取完整提示词失败', e);
        }
      });
    })();
    return () => unlisten?.();
  }, []);

  // AI 学习指示器

  const editorRef = useRef<RichTextEditorRef>(null);
  // A4-1.9: 打字机效果改为 requestAnimationFrame 驱动
  const typewriterFrameRef = useRef<number | null>(null);
  // v5.2.0: 标记刚完成自动保存的时间戳，避免循环刷新
  const justSavedRef = useRef<number>(0);
  // A4-1.7/1.9: 生成任务计时器（仅记录开始时间，不启用 1s setInterval 心跳）
  const generationStartTimeRef = useRef<number | null>(null);
  // A4-1.8: 最新内容 ref，供防抖后的自动保存读取，避免在输入关键路径创建大对象
  const latestContentRef = useRef<string>('');
  // A4-1.8: notify_backstage_content_changed 节流定时器
  const notifyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // 备用机制：记录最后收到事件的时间，如果10秒内无新事件则显示提示
  const lastEventTimeRef = useRef<number | null>(null);
  const fallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // C1: 记录最近一次收到统一生成状态事件的时间，用于跳过重叠的旧事件
  const lastGenerationStatusAtRef = useRef<number>(0);
  // v0.13.1: 诊断数据收集——记录最后的原始进度事件和输入，用于超时/失败时弹窗
  const lastProgressEventRef = useRef<Record<string, any> | null>(null);
  // v0.13.2: 独立诊断计时器（不会被 stopElapsedTimer 清空）
  const diagnosticStartTimeRef = useRef<number | null>(null);
  // v0.13.2: 后端是否曾响应过
  const backendEverRespondedRef = useRef<boolean>(false);
  // v0.13.2: smart_execute 飞行中标记，防 activityStore 提前清空状态
  const smartExecuteInFlightRef = useRef<boolean>(false);
  // v0.13.3: 安全网——标记本次 smart_execute 是否仍需要诊断卡片兜底
  const smartExecuteNeedDiagnosticRef = useRef<boolean>(false);
  // v0.16.2: 主流程已收到"已完成/出错/已取消"——后续后台任务（audit/insight）的
  // progress event 不应再更新 lastEventTimeRef，否则会重置"距上次响应"计时，
  // 让前端在主流程已结束后仍 200s 后报假超时（写第二章场景实测）。
  const mainGenerationCompletedRef = useRef<boolean>(false);
  // v0.13.3: 用户主动取消时跳过安全网诊断
  const lastGenerationCancelledRef = useRef<boolean>(false);
  // v0.23.7: 记录后端最近一次实际发给 LLM 的提示词与模型，用于诊断卡片
  const lastLlmPromptRef = useRef<string>('未捕获（可能生成未进入 LLM 调用）');
  const lastLlmModelRef = useRef<string>('未知');
  const [showDiagnosticCard, setShowDiagnosticCard] = useState(false);
  const [diagnosticData, setDiagnosticData] = useState<Record<string, string>>({});

  // A4-1.7: 根据生成开始时间计算已用秒数
  const getElapsedSeconds = useCallback(() => {
    return generationStartTimeRef.current
      ? Math.floor((Date.now() - generationStartTimeRef.current) / 1000)
      : 0;
  }, []);

  // v0.13.1: 捕获诊断信息，在超时/错误时弹出诊断卡片
  const captureDiagnosticInfo = useCallback(
    (errorMsg: string) => {
      const now = Date.now();
      // v0.13.2: 优先用独立ref（不会被 stopElapsedTimer 清空）
      const diagStart = diagnosticStartTimeRef.current || generationStartTimeRef.current;
      const elapsed = diagStart ? Math.floor((now - diagStart) / 1000) : 0;
      const timeSinceLastEvent = lastEventTimeRef.current
        ? Math.floor((now - lastEventTimeRef.current) / 1000)
        : 0;
      const lastEvt = lastProgressEventRef.current;
      const backendVersion = (window as any).__STORYFORGE_VERSION__ || 'unknown';

      const feTimeout = settings?.frontend_timeout_secs ?? 200;
      const beTimeout = settings?.smart_execute_total_timeout_secs ?? 180;
      const firstChunkTimeout = settings?.llm_first_chunk_timeout_secs ?? 60;
      const connectTimeout = settings?.llm_connect_timeout_secs ?? 30;
      const promptText = lastLlmPromptRef.current;
      const promptPreview =
        promptText.length > 12000 ? `${promptText.substring(0, 12000)}\n...（已截断）` : promptText;

      const info: Record<string, string> = {
        发生时间: new Date().toISOString(),
        应用版本: backendVersion,
        AI生成模式: settings?.generation_mode ?? 'auto',
        当前模型ID: activeChatModel?.id || activeChatModelId || '未配置',
        当前模型名称: activeChatModel?.name || '未知',
        当前模型提供商: activeChatModel?.provider || '未知',
        当前模型端点: activeChatModel?.api_base || '未知',
        最后调用模型: lastLlmModelRef.current,
        '当前阶段（UI 判定）': currentToastPhaseRef.current || '未知',
        当前状态文字: generationStatus || '无',
        '已用时（秒）': String(elapsed),
        '距最后一次进度事件（秒）': String(timeSinceLastEvent),
        是否收到过后端响应: backendEverRespondedRef.current ? '是' : '否',
        最后事件类型: lastEvt?.stage || lastEvt?.step_type || '无',
        最后事件消息: lastEvt?.message || lastEvt?.detail || '无',
        最后事件详情: lastEvt ? JSON.stringify(lastEvt).substring(0, 500) : '{}',
        作品存在: currentStory?.id ? '是' : '否（currentStory 为 null）',
        前端超时秒数: String(feTimeout),
        后端smart_execute整体超时秒数: String(beTimeout),
        LLM连接超时秒数: String(connectTimeout),
        LLM首字节超时秒数: String(firstChunkTimeout),
        后端超时配置: `前端${feTimeout}s / 后端smart_execute整体${beTimeout}s / LLM连接${connectTimeout}s / LLM首字节${firstChunkTimeout}s`,
        DB连接池: dbPoolStatus
          ? `${dbPoolStatus.in_use}/${dbPoolStatus.max_size}（空闲${dbPoolStatus.idle}，超时${dbPoolStatus.connection_timeout_secs}s）`
          : '未获取',
        最后发给模型的提示词: promptPreview,
        模型建议: '若本地模型慢，可尝试减少上下文或用量',
        作品ID: currentStory?.id || '无',
        章数: chapters?.length ? String(chapters.length) : '未知',
        场数: scenes?.length ? String(scenes.length) : '未知',
        当前章节号: currentChapter?.chapter_number ? String(currentChapter.chapter_number) : '无',
        编辑器字数: editorRef.current?.getText()?.length
          ? String(editorRef.current.getText().length)
          : '未知',
        模型是否在运行: '请检查模型服务（vllm/Ollama/OpenAI）是否正常运行',
        错误信息: errorMsg,
        提示: '请复制以上信息到 GitHub Issue 或发给开发者排查',
      };
      setDiagnosticData(info);
      setShowDiagnosticCard(true);

      // v0.23.8: 如果事件链路没有拿到完整提示词，再主动通过命令读取一次，确保诊断卡片尽量有内容
      // v0.23.12: 同时读取智能创作流程日志，方便定位卡在哪一步。
      (async () => {
        try {
          const updates: Record<string, string> = {};

          if (
            lastLlmPromptRef.current.includes('未捕获') ||
            lastLlmPromptRef.current.includes('正在读取')
          ) {
            const full = await invoke<{
              request_id: string;
              context_label: string;
              model_id: string;
              model_name: string;
              provider: string;
              prompt: string;
              prompt_chars: number;
              prompt_tokens: number;
              updated_at: string;
            } | null>('get_last_llm_prompt');
            if (full && full.prompt) {
              lastLlmPromptRef.current = full.prompt;
              lastLlmModelRef.current = `${full.model_name} (${full.provider})`;
              updates['最后发给模型的提示词'] =
                full.prompt.length > 12000
                  ? `${full.prompt.substring(0, 12000)}\n...（已截断）`
                  : full.prompt;
              updates['最后调用模型'] = lastLlmModelRef.current;
            }
          }

          const logPath = await invoke<string>('get_workflow_log_path');
          const logs = await invoke<string[]>('get_workflow_logs', { count: 30 });
          updates['工作流日志路径'] = logPath;
          updates['智能创作流程最近日志'] = logs.length ? logs.join('\n') : '（暂无日志）';

          if (Object.keys(updates).length > 0) {
            setDiagnosticData(prev => ({ ...prev, ...updates }));
          }
        } catch (e) {
          frontstageLogger.warn('[captureDiagnosticInfo] 读取诊断附加信息失败', e);
        }
      })();
    },
    [
      generationStatus,
      currentStory,
      chapters,
      scenes,
      currentChapter,
      activeChatModel,
      activeChatModelId,
      settings,
    ]
  );

  // A4-1.7: 将基础文案与已用时间拼接（避免重复追加时间后缀）
  const formatStatusWithElapsed = useCallback(
    (base: string) => {
      const cleanBase = cleanStatusBase(base) || 'AI 正在处理中';
      const elapsed = getElapsedSeconds();
      return elapsed > 0 ? `${cleanBase} (${elapsed}s)` : cleanBase;
    },
    [cleanStatusBase, getElapsedSeconds]
  );

  const mapPrecisePhase = useCallback((raw: string | undefined): string | null => {
    if (!raw) return null;
    const s = raw.toLowerCase();
    for (const { phase, patterns } of PRECISE_PHASE_PATTERNS) {
      if (patterns.some(p => s.includes(p.toLowerCase()))) {
        return phase;
      }
    }
    return null;
  }, []);

  // C1: 判断旧版重叠事件是否应被跳过（统一事件已覆盖）
  const shouldSkipOverlappingEvent = useCallback(() => {
    return Date.now() - lastGenerationStatusAtRef.current < 1000;
  }, []);

  // v0.13.2: 前端自救计时器——当后端心跳停止时，前端自主更新已用时
  // 每次调用先清除旧定时器，10秒后检查；若仍无新事件则更新状态并再次调度
  const scheduleFallbackPrompt = useCallback(() => {
    if (fallbackTimerRef.current) clearTimeout(fallbackTimerRef.current);
    const tick = () => {
      // v0.16.2: 主流程已结束则停止 tick（不再显示 "AI 正在深度思考中..."）
      if (mainGenerationCompletedRef.current) return;
      if (!lastEventTimeRef.current || !generationStartTimeRef.current) return;
      const sinceLastEvent = Math.floor((Date.now() - lastEventTimeRef.current) / 1000);
      const totalElapsed = Math.floor((Date.now() - generationStartTimeRef.current) / 1000);
      if (sinceLastEvent > 10) {
        setGenerationStatus(prev => {
          // 如果已经有模型生成中的提示，不要覆盖
          if (prev.includes('正在生成中') || prev.includes('等待响应')) return prev;
          return formatStatusWithElapsed(
            `AI 正在深度思考中...（已用时 ${totalElapsed} 秒，距上次响应 ${sinceLastEvent} 秒）`
          );
        });
      }
      // v0.13.2: 自我重调度——每 10 秒更新一次，直到 stopElapsedTimer 清除
      fallbackTimerRef.current = setTimeout(tick, 10000);
    };
    fallbackTimerRef.current = setTimeout(tick, 10000);
  }, [formatStatusWithElapsed]);

  // 辅助函数：启动运行时长计时器
  const startElapsedTimer = useCallback(() => {
    generationStartTimeRef.current = Date.now();
    lastEventTimeRef.current = Date.now();
    // v0.13.3: 不要在启动时就认为后端已响应，应等真正收到事件后再标记
    diagnosticStartTimeRef.current = Date.now(); // v0.13.2
    // v0.16.2: 新一轮生成开始，重置主流程完成标记
    mainGenerationCompletedRef.current = false;
    scheduleFallbackPrompt();
  }, [scheduleFallbackPrompt]);

  // 辅助函数：停止运行时长计时器
  const stopElapsedTimer = useCallback(() => {
    if (fallbackTimerRef.current) {
      clearTimeout(fallbackTimerRef.current);
      fallbackTimerRef.current = null;
    }
    generationStartTimeRef.current = null;
  }, []);

  // 辅助函数：更新最后收到事件的时间
  const updateLastEventTime = useCallback(() => {
    // v0.16.2: 主流程已结束后忽略后台 audit/insight 的 progress event，
    // 避免重置 sinceLastEvent 导致 200s 假超时。
    if (mainGenerationCompletedRef.current) return;
    lastEventTimeRef.current = Date.now();
    backendEverRespondedRef.current = true; // v0.13.2
    scheduleFallbackPrompt();
  }, [scheduleFallbackPrompt]);

  // C1: 处理统一生成状态事件，更新底部状态栏与 orchestratorStatus
  const handleGenerationStatus = useCallback(
    (p: {
      phase: string;
      progress: number;
      message: string;
      elapsed_ms: number;
      task_id: string;
      request_id?: string | null;
    }) => {
      lastProgressEventRef.current = { stage: p.phase, message: p.message, step: p.progress };
      lastGenerationStatusAtRef.current = Date.now();
      // v0.16.2: 主流程结束阶段（已完成/出错/已取消）后，标记 ref 让后续后台
      // progress event 不再触发 sinceLastEvent 重置。
      const isTerminal = p.phase === '已完成' || p.phase === '出错' || p.phase === '已取消';
      if (isTerminal) {
        mainGenerationCompletedRef.current = true;
      }
      updateLastEventTime();
      const precise = mapPrecisePhase(p.phase) || mapPrecisePhase(p.message);
      // v0.23.43: 状态文案追加模型名称，提高识别度（用名称而非 ID）
      const modelName = lastLlmModelRef.current;
      const message = precise || p.message;
      const messageWithModel =
        modelName && modelName !== '未知' ? `${message} · ${modelName}` : message;
      setGenerationStatus(formatStatusWithElapsed(messageWithModel));
      setOrchestratorStatus({
        stepType: p.phase,
        loopIdx: undefined,
        score: p.progress,
        message: messageWithModel,
        detail: p.request_id || undefined,
      });
      updateGenerationPhase(message);
    },
    [formatStatusWithElapsed, mapPrecisePhase, updateGenerationPhase, updateLastEventTime]
  );

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
      if (typewriterFrameRef.current) {
        cancelAnimationFrame(typewriterFrameRef.current);
        typewriterFrameRef.current = null;
      }
      if (notifyTimeoutRef.current) {
        clearTimeout(notifyTimeoutRef.current);
        notifyTimeoutRef.current = null;
      }
    };
  }, []);

  // Setup Tauri event listeners
  const setupEventListeners = async (unlisteners: (() => void)[]) => {
    try {
      // C1: 监听统一生成状态事件（主要消费通道）
      const unlistenGenerationStatus = await listen<{
        phase: string;
        progress: number;
        message: string;
        elapsed_ms: number;
        task_id: string;
        request_id?: string | null;
      }>('generation-status', event => {
        handleGenerationStatus(event.payload);
      });
      unlisteners.push(unlistenGenerationStatus);

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
              // v0.23.64: 去重守卫——检查 formatted 尾部是否已存在于 prev 中，
              // 避免同一正文被 AppendContent 事件拼接两次（根因：auto_write 循环
              // 可能重发相同内容，或前端重复接收事件）
              setContent(prev => {
                const prevText = prev.replace(/<[^>]*>/g, '').trim();
                const formattedText = formatted.replace(/<[^>]*>/g, '').trim();
                if (formattedText && prevText.endsWith(formattedText.slice(-200))) {
                  frontstageLogger.warn('[AppendContent] Skipped duplicate append', {
                    formattedLen: formattedText.length,
                  });
                  return prev;
                }
                return prev + formatted;
              });
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
              // v0.23.55: 正文通过 ChapterSwitch 加载到编辑器时，必须清空 generatedText。
              // 根因：创世/续写成功后 generatedText 可能仍持有正文（纯文本幽灵段落），
              // 与编辑器正文（HTML排版）并存 → "有排版版+无排版版"两份重复。
              // 在 ChapterSwitch（正文加载点）清空是最可靠的时机。
              // v0.23.66: 无条件清空——即使 stale closure 中 generatedText 为空，
              // 实际渲染时可能已有内容。setGeneratedText('') 是幂等操作，安全。
              setGeneratedText('');
              // [DEBUG-dup] 追踪 ChapterSwitch 事件触发次数与内容
              chapterSwitchCountRef.current += 1;
              chapterSwitchTimestampRef.current = Date.now(); // v0.23.62: post-ChapterSwitch quiet period
              frontstageLogger.info('[DEBUG-dup] ChapterSwitch event received', {
                count: chapterSwitchCountRef.current,
                story_id: payload.story_id,
                chapter_id: payload.chapter_id,
                has_content: !!payload.content,
                content_length: payload.content?.length || 0,
                content_preview: payload.content?.slice(0, 60) ?? 'EMPTY',
                current_chapter_id: currentChapterRef.current?.id ?? 'null',
                is_saved: useFrontstageStore.getState().isSaved,
                store_content_length: useFrontstageStore.getState().content.length,
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
        setGenerationStatus(formatStatusWithElapsed(p.message));
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
        const precise = mapPrecisePhase(p.stage) || mapPrecisePhase(p.message) || p.message;
        setGenerationStatus(formatStatusWithElapsed(precise));
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
        lastProgressEventRef.current = event.payload as any;
        updateLastEventTime();
        const precise = mapPrecisePhase(p.stage) || mapPrecisePhase(p.message) || p.message;
        setGenerationStatus(formatStatusWithElapsed(precise));
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
        lastProgressEventRef.current = event.payload as any;
        updateLastEventTime();
        frontstageLogger.debug('[plan-executor-step]', {
          status: p.status,
          message: p.message,
          progress: `${p.steps_completed}/${p.total_steps}`,
        });
        if (p.step_id === '__complete__') {
          setGenerationStatus(p.message);
        } else if (p.status === 'running') {
          const precise = mapPrecisePhase(p.message) || p.message;
          setGenerationStatus(
            formatStatusWithElapsed(`${precise} (${p.steps_completed + 1}/${p.total_steps})`)
          );
          updateGenerationPhase(precise);
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
        lastProgressEventRef.current = event.payload as any;
        // C1: 统一事件已覆盖 orchestrator 进度，跳过重叠更新
        if (shouldSkipOverlappingEvent()) return;
        const p = event.payload;
        updateLastEventTime();
        // A4-1.7: 映射到统一精确阶段文案
        const precise = mapPrecisePhase(p.step_type) || mapPrecisePhase(p.detail);
        const stepNames: Record<string, string> = {
          生成: '候选生成',
          质检: 'Inspector 审校',
          改写: '改写',
        };
        let message = precise || p.detail || stepNames[p.step_type] || p.step_type;
        if (p.step_type === '改写' && typeof p.loop_idx === 'number' && !p.detail) {
          message = `改写（第 ${p.loop_idx + 1} 轮）`;
        }
        if (p.step_type === '质检' && typeof p.score === 'number' && !p.detail) {
          message = `Inspector 审校（评分 ${p.score}%）`;
        }
        setGenerationStatus(formatStatusWithElapsed(message));
        setOrchestratorStatus({
          stepType: p.step_type,
          loopIdx: p.loop_idx,
          score: p.score,
          message,
          detail: p.detail,
        });
        updateGenerationPhase(message);
      });
      unlisteners.push(unlistenOrchestrator);

      // 监听 agent-stage-update 事件 — Agent内部阶段
      const unlisten7 = await listen<{
        agent_type: string;
        stage: string;
        message: string;
        progress: number;
      }>('agent-stage-update', event => {
        // C1: 统一事件已覆盖 Agent 创作阶段，跳过重叠更新
        if (shouldSkipOverlappingEvent()) return;
        const p = event.payload;
        updateLastEventTime();
        frontstageLogger.debug('[agent-stage-update]', {
          stage: p.stage,
          agent_type: p.agent_type,
          message: p.message,
        });
        // A4-1.7: 优先映射到统一精确阶段文案
        const precise = mapPrecisePhase(p.stage) || mapPrecisePhase(p.message);
        const displayMessage = precise || `${p.agent_type}: ${p.message}`;
        // v0.11.2: Agent 内部阶段不应覆盖更具体的进度（如候选生成）。
        // 如果当前状态包含具体进度，仅记录阶段；否则显示 Agent 阶段。
        setGenerationStatus(prev => {
          const base = cleanStatusBase(prev);
          const hasSpecificProgress = /候选|第\s*\d+\s*轮|评分|匹配度|降级|失败|准备中/.test(base);
          if (hasSpecificProgress) return formatStatusWithElapsed(base);
          return formatStatusWithElapsed(displayMessage);
        });
        updateGenerationPhase(displayMessage);
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
        // v0.14.4: 心跳事件必须依赖 isGenerating，避免后台静默调用误触发状态栏
        if (!isGenerating && !smartExecuteInFlightRef.current) {
          return;
        }
        updateLastEventTime();
        lastProgressEventRef.current = {
          stage: p.stage,
          message: p.message,
          elapsed: p.elapsed_seconds,
          source: 'heartbeat',
        }; // v0.13.2
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

        // C1: 统一事件已覆盖 LLM 创作进度，跳过重叠的状态栏更新
        if (shouldSkipOverlappingEvent()) return;

        // v0.11.2: LLM 心跳不应覆盖更具体的阶段进度（如"生成候选 1/2"）。
        // A4-1.7: 优先映射到精确阶段文案，并基于前端计时器显示已用时间。
        const precise = mapPrecisePhase(p.stage) || mapPrecisePhase(p.message) || p.message;
        // v0.23.43: 状态文案追加模型名称，提高识别度
        const modelName = lastLlmModelRef.current;
        const preciseWithModel =
          modelName && modelName !== '未知' ? `${precise} · ${modelName}` : precise;
        setGenerationStatus(prev => {
          const base = cleanStatusBase(prev);
          const hasSpecificProgress = /候选|第\s*\d+\s*轮|评分|匹配度|降级|失败|准备中/.test(base);
          if (hasSpecificProgress) {
            return formatStatusWithElapsed(base);
          }
          return formatStatusWithElapsed(preciseWithModel);
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

  // B2: 刷新当前故事的 scenes 列表；传入 page 则使用分页接口并合并到本地状态
  const loadStoryScenes = async (storyId: string, page?: number) => {
    try {
      const result =
        page && page > 0
          ? await loggedInvoke<Scene[]>('get_story_scenes_paged', {
              story_id: storyId,
              limit: SCENES_PAGE_SIZE,
              offset: (page - 1) * SCENES_PAGE_SIZE,
            })
          : await loggedInvoke<Scene[]>('get_story_scenes', { story_id: storyId });
      if (page && page > 0) {
        setScenes(prev => {
          const map = new Map(prev.map(s => [s.id, s]));
          result.forEach(s => map.set(s.id, s));
          return Array.from(map.values()).sort((a, b) => a.sequence_number - b.sequence_number);
        });
      } else {
        setScenes(result);
      }
    } catch (e) {
      frontstageLogger.error('Failed to load scenes', { error: e });
    }
  };

  // B2: 刷新当前故事的 chapters 列表；传入 page 则使用分页接口并合并到本地状态
  const loadStoryChapters = async (storyId: string, page?: number) => {
    try {
      const result =
        page && page > 0
          ? await loggedInvoke<Chapter[]>('get_story_chapters_paged', {
              story_id: storyId,
              limit: CHAPTERS_PAGE_SIZE,
              offset: (page - 1) * CHAPTERS_PAGE_SIZE,
            })
          : await loggedInvoke<Chapter[]>('get_story_chapters', { story_id: storyId });
      if (page && page > 0) {
        setChapters(prev => {
          const map = new Map(prev.map(c => [c.id, c]));
          result.forEach(c => map.set(c.id, c));
          return Array.from(map.values()).sort((a, b) => a.chapter_number - b.chapter_number);
        });
      } else {
        setChapters(result);
      }
    } catch (e) {
      frontstageLogger.error('Failed to load chapters', { error: e });
    }
  };

  // B2: 从后端聚合获取全文字数，避免全量 chapters content 上传统计
  const loadStoryWordCount = useCallback(async (storyId: string) => {
    try {
      const result = await loggedInvoke<{ total_chars: number }>('get_story_word_count', {
        story_id: storyId,
      });
      setTotalWordCount(result.total_chars);
    } catch (e) {
      frontstageLogger.error('Failed to load story word count', { error: e });
    }
  }, []);

  const selectStory = async (story: Story) => {
    setCurrentStory(story);
    try {
      // B2: 初始仅加载当前章附近内容，降低大 story 的 IPC payload
      const [result, scenesResult] = await Promise.all([
        loggedInvoke<Chapter[]>('get_story_chapters_paged', {
          story_id: story.id,
          limit: CHAPTERS_PAGE_SIZE,
          offset: 0,
        }),
        loggedInvoke<Scene[]>('get_story_scenes_paged', {
          story_id: story.id,
          limit: SCENES_PAGE_SIZE,
          offset: 0,
        }),
      ]);
      setChapters(result);
      setScenes(scenesResult);
      loadStoryWordCount(story.id);
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
    // [DEBUG-dup] 诊断日志保留，guard 回滚 —— v0.23.37 的同章同内容跳过 guard
    // 可能与竞态交互导致状态不一致。先回到 v0.23.37 之前的行为。
    frontstageLogger.info('[DEBUG-dup] selectChapter called', {
      chapter_id: chapter.id,
      content_length: chapter.content?.length ?? 0,
      content_preview: chapter.content?.slice(0, 50) ?? 'EMPTY',
    });

    // B2: 分页列表不返回 content（序列化为 null），若选中章节缺少正文则按需加载完整章节
    if ((chapter.content === undefined || chapter.content === null) && chapter.id) {
      (async () => {
        try {
          const full = await loggedInvoke<Chapter | null>('get_chapter', { id: chapter.id });
          if (full) {
            frontstageLogger.info('[selectChapter] Lazy-loaded full chapter', {
              chapter_id: full.id,
              content_length: full.content?.length ?? 0,
            });
            selectChapter(full);
          }
        } catch (e) {
          frontstageLogger.error('Failed to lazy-load chapter content', { error: e });
        }
      })();
      return;
    }

    // B2: 若本地尚未加载该章节（跨章切换），加载其所在分页
    const chapterIndex = chapters.findIndex(c => c.id === chapter.id);
    if (chapterIndex === -1 && currentStory) {
      (async () => {
        try {
          const full = await loggedInvoke<Chapter | null>('get_chapter', { id: chapter.id });
          if (full) {
            setChapters(prev => {
              const map = new Map(prev.map(c => [c.id, c]));
              map.set(full.id, full);
              return Array.from(map.values()).sort((a, b) => a.chapter_number - b.chapter_number);
            });
            selectChapter(full);
          }
        } catch (e) {
          frontstageLogger.error('Failed to load missing chapter', { error: e });
        }
      })();
      return;
    }

    // B2: 接近已加载章节末尾时预加载下一页
    if (
      chapterIndex >= 0 &&
      chapters.length > 0 &&
      chapterIndex >= chapters.length - 1 &&
      currentStory
    ) {
      const nextPage = Math.floor(chapters.length / CHAPTERS_PAGE_SIZE) + 1;
      loadStoryChapters(currentStory.id, nextPage);
    }

    cancelAutoSave();
    setCurrentChapter(chapter);
    const formattedContent = autoFormatText(chapter.content || '');
    setContent(formattedContent);
    // v0.23.23: 同步 latestContentRef，使 handleContentChange 的内容比较基准正确
    latestContentRef.current = formattedContent;
    setIsSaved(true);
    setChapterInfo(chapter.id, chapter.title || '', currentStory?.title);

    // Sync currentScene if chapter has associated scene
    if (chapter.scene_id) {
      const associatedScene = scenes.find(s => s.id === chapter.scene_id);
      if (!associatedScene) {
        (async () => {
          try {
            const fullScene = await loggedInvoke<Scene | null>('get_scene', {
              scene_id: chapter.scene_id,
            });
            if (fullScene) {
              setScenes(prev => {
                const map = new Map(prev.map(s => [s.id, s]));
                map.set(fullScene.id, fullScene);
                return Array.from(map.values()).sort(
                  (a, b) => a.sequence_number - b.sequence_number
                );
              });
              setCurrentScene(fullScene);
            }
          } catch (e) {
            frontstageLogger.error('Failed to lazy-load associated scene', { error: e });
          }
        })();
      } else {
        setCurrentScene(associatedScene);
      }
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

  // A4-1.8: 单次遍历计算中文字数 + 英文词数，避免两次正则 match
  const computeWordCount = useCallback((html: string): number => {
    const text = html.replace(/<[^>]*>/g, '');
    let chinese = 0;
    let english = 0;
    let inWord = false;
    for (const char of text) {
      if (char >= '\u4e00' && char <= '\u9fa5') {
        chinese++;
        inWord = false;
      } else if ((char >= 'a' && char <= 'z') || (char >= 'A' && char <= 'Z')) {
        if (!inWord) {
          english++;
          inWord = true;
        }
      } else {
        inWord = false;
      }
    }
    return chinese + english;
  }, []);

  const handleContentChange = useCallback(
    async (newContent: string) => {
      // v0.23.23: 内容未变化时跳过，避免伪"保存中"
      if (newContent === latestContentRef.current) return;
      setContent(newContent);
      setIsSaved(false);
      // A4-1.8: 使用 ref 保存最新内容，避免在输入关键路径立即创建保存任务对象
      latestContentRef.current = newContent;

      if (currentChapter) {
        // B1: 基于当前章节字数增量 diff 更新全文字数，避免每次输入都全量 reduce
        const newWordCount = computeWordCount(newContent);
        const delta = newWordCount - currentChapterPrevWordCountRef.current;
        if (delta !== 0) {
          setTotalWordCount(prev => prev + delta);
        }
        currentChapterPrevWordCountRef.current = newWordCount;
        // W4-F7: 自动保存非阻塞化 — 使用 requestIdleCallback + startTransition
        scheduleAutoSave(
          () => ({
            chapterId: currentChapter.id,
            title: currentChapter.title,
            content: latestContentRef.current,
            wordCount: computeWordCount(latestContentRef.current),
          }),
          async payload => {
            try {
              await loggedInvoke<unknown>('update_chapter', {
                id: payload.chapterId,
                title: payload.title,
                content: payload.content,
                word_count: payload.wordCount,
              });
              setWordCount(payload.wordCount);
              setIsSaved(true);
              justSavedRef.current = Date.now();
            } catch (e) {
              frontstageLogger.error('Auto-save failed', { error: e });
            }
          },
          2000
        );

        // A4-1.8: notify_backstage_content_changed IPC 节流至 350ms
        if (notifyTimeoutRef.current) {
          clearTimeout(notifyTimeoutRef.current);
        }
        notifyTimeoutRef.current = setTimeout(() => {
          notifyTimeoutRef.current = null;
          loggedInvoke<unknown>('notify_backstage_content_changed', {
            text: latestContentRef.current,
            chapter_id: currentChapter.id,
          }).catch(e => frontstageLogger.error('Failed to notify content change', { error: e }));
        }, 350);
      }
    },
    [currentChapter, computeWordCount]
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

      if (typewriterFrameRef.current) {
        cancelAnimationFrame(typewriterFrameRef.current);
        typewriterFrameRef.current = null;
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

      // v0.14.0: 前端超时从 330 秒降至 200 秒，后端 smart_execute 整体超时为 180 秒。
      // v0.23.7: 改为从 settings 读取，避免用户已调整超时后文案仍显示 200/180。
      const feTimeoutSeconds = settings?.frontend_timeout_secs ?? 200;
      const beTimeoutSeconds = settings?.smart_execute_total_timeout_secs ?? 180;
      const feTimeoutMs = feTimeoutSeconds * 1000;
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          useBackendActivityStore
            .getState()
            .failAllRunning(`前端超时：模型未在 ${feTimeoutSeconds} 秒内响应`);
          // v0.14.0: 前端超时时通知后端取消所有进行中的 LLM 生成（best-effort）
          invoke('llm_cancel_all_generations').catch(() => {
            /* best-effort，忽略取消失败 */
          });
          reject(
            new Error(
              `前端超时：模型响应超过${feTimeoutSeconds}秒。后端 smart_execute 整体超时为${beTimeoutSeconds}秒，正常情况下应由后端先返回错误。请检查模型服务（vllm/Ollama/OpenAI）是否正常运行。`
            )
          );
        }, feTimeoutMs);
      });
      cancelGenerationRef.current = () => {
        if (timeoutId) clearTimeout(timeoutId);
      };

      try {
        smartExecuteInFlightRef.current = true;
        smartExecuteNeedDiagnosticRef.current = true; // v0.13.3
        const result = await Promise.race([
          smartExecute({
            user_input: context || '续写',
            current_content: editorRef.current?.getText(),
            style_weight: 50,
          }),
          timeoutPromise,
        ]);
        if (timeoutId) clearTimeout(timeoutId);
        smartExecuteInFlightRef.current = false;

        setGenerationStatus('质检通过，生成完成');
        setOrchestratorStatus({ stepType: '完成', message: '质检通过，生成完成' });

        // v5.1.0: Bootstrap 完成后自动加载新故事并切换到第一章
        const storyCreatedMsg = result.messages?.find((m: string) =>
          m.startsWith('story_created:')
        );
        const isBackgroundBootstrap = result.messages?.some(
          (m: string) =>
            m === 'novel_bootstrap_background_started' ||
            m === 'novel_bootstrap_first_chapter_ready'
        );
        const isFirstChapterReady = result.messages?.some(
          (m: string) => m === 'novel_bootstrap_first_chapter_ready'
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

        // v0.23.15: 第一章已就绪时，正文已通过 ChapterSwitch 事件加载到编辑器，
        // 不再走"后台生成中"的空内容路径。仅旧版 background_started 才走后台路径。
        if (isBackgroundBootstrap && !isFirstChapterReady) {
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

        // v0.23.15: 第一章已就绪——正文已通过 ChapterSwitch 加载，直接完成
        if (isFirstChapterReady) {
          frontstageLogger.info('[Bootstrap] Story created, first chapter ready');
          toast.success('小说已创建！第一章已生成，您可以开始写作了', { duration: 4000 });
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
          smartExecuteInFlightRef.current = false;
          smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
          captureDiagnosticInfo('AI 返回了空内容（final_content 为空）');
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
        // A4-1.9: 使用 requestAnimationFrame 替代 16ms setInterval 打字机效果
        let index = 0;
        const typeFrame = () => {
          index += 3;
          if (index >= displayText.length) {
            typewriterFrameRef.current = null;
            smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
            setGeneratedText(displayText);
            stopElapsedTimer();
            setIsGenerating(false);
            setOrchestratorStatus(null);
          } else {
            setGeneratedText(displayText.slice(0, index));
            typewriterFrameRef.current = requestAnimationFrame(typeFrame);
          }
        };
        typewriterFrameRef.current = requestAnimationFrame(typeFrame);
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
        // v0.13.1: 弹出诊断卡片，方便用户复制报错信息
        smartExecuteInFlightRef.current = false;
        smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
        captureDiagnosticInfo(msg);
        setIsGenerating(false);
        setGenerationStatus('');
        setOrchestratorStatus(null);
      }
    },
    [isGenerating, settings]
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
    lastGenerationCancelledRef.current = true; // v0.13.3
    smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
    if (cancelGenerationRef.current) {
      cancelGenerationRef.current();
      cancelGenerationRef.current = null;
    }
    // A4-1.9: 立即停止前端打字机动画，避免取消后仍有文本输出
    if (typewriterFrameRef.current) {
      cancelAnimationFrame(typewriterFrameRef.current);
      typewriterFrameRef.current = null;
    }
    // A4-1.7: 立即清理前端运行状态，即使后端 join_all 尚未完成也能立刻反馈
    stopElapsedTimer();
    setIsGenerating(false);
    setGenerationStatus('✓ 已取消生成');
    // v0.11.2: 清理所有残留的后台活动，避免取消后状态栏仍显示"系统正在处理中"
    useBackendActivityStore.getState().failAllRunning('用户已取消');
    // v0.11.5: 真正通知后端取消所有 Agent / LLM 任务，而不仅是清理前端状态
    // A4-1.7: 即使后端 join_all 阻塞，前端状态也已先完成清理；设置 5s 超时避免挂起
    try {
      await Promise.race([
        loggedInvoke<unknown>('agent_cancel_all_tasks', {}),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('取消命令超时')), 5000)
        ),
      ]);
    } catch (e) {
      frontstageLogger.error('Failed to cancel agent tasks', { error: e });
    }
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
    }
  }, [stopElapsedTimer]);

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
      // v0.14.0: 前端超时统一降至 200 秒，后端 smart_execute 整体超时为 180 秒。
      // v0.15.5/v0.23.7: 从设置读取，避免用户已调整超时后文案仍显示 200/180。
      const timeoutSeconds = settings?.frontend_timeout_secs ?? 200;
      const beTimeoutSeconds = settings?.smart_execute_total_timeout_secs ?? 180;
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
          useBackendActivityStore
            .getState()
            .failAllRunning(`前端超时：模型未在 ${timeoutSeconds} 秒内响应`);
          // v0.14.0: 前端超时时通知后端取消所有进行中的 LLM 生成（best-effort）
          invoke('llm_cancel_all_generations').catch(() => {
            /* best-effort，忽略取消失败 */
          });
          reject(
            new Error(
              isBootstrap
                ? `前端超时：模型响应超过${timeoutSeconds}秒。创建新小说需要多次LLM调用，后端 smart_execute 整体超时为${beTimeoutSeconds}秒，正常情况下应由后端先返回错误。请检查模型服务（vllm/Ollama/OpenAI）是否正常运行。`
                : `前端超时：模型响应超过${timeoutSeconds}秒。后端 smart_execute 整体超时为${beTimeoutSeconds}秒，正常情况下应由后端先返回错误。请检查模型服务（vllm/Ollama/OpenAI）是否正常运行。`
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
        smartExecuteInFlightRef.current = true;
        smartExecuteNeedDiagnosticRef.current = true; // v0.13.3
        const result = await Promise.race([
          smartExecute({
            user_input: userInput,
            current_content: editorRef.current?.getText(),
            style_weight: 50,
          }),
          timeoutPromise,
        ]);

        if (timeoutId) clearTimeout(timeoutId);
        smartExecuteInFlightRef.current = false;
        if (aborted) {
          stopElapsedTimer();
          setIsGenerating(false);
          setGenerationStatus('');
          return;
        }

        currentToastPhaseRef.current = null;

        // v0.23.15: 区分三种 Bootstrap 状态
        // - first_chapter_ready: 第一章已生成（v0.23.14+），正文已通过 ChapterSwitch 加载
        // - background_started: 旧版后台生成中（final_content 为空）
        const isFirstChapterReady = result.messages.some(
          m => m === 'novel_bootstrap_first_chapter_ready'
        );
        const isBackgroundBootstrap = result.messages.some(
          m => m === 'novel_bootstrap_background_started'
        );
        const isBootstrapCompleted =
          isFirstChapterReady ||
          (!isBackgroundBootstrap && result.messages.some(m => m.includes('novel_bootstrap')));

        // [DEBUG-dup] 诊断创世正文重复：记录 smartExecute 返回的关键字段
        frontstageLogger.info('[DEBUG-dup] smartExecute returned', {
          success: result.success,
          messages: result.messages,
          isFirstChapterReady,
          isBackgroundBootstrap,
          isBootstrapCompleted,
          final_content_length: result.final_content?.length ?? 0,
          final_content_is_summary:
            !!result.final_content && result.final_content.includes('已创建，第一章正文已生成'),
          final_content_preview: result.final_content?.slice(0, 80) ?? 'EMPTY',
          generatedText_before: generatedText,
        });

        if (isFirstChapterReady) {
          frontstageLogger.info('[SmartGeneration] Story created, first chapter ready');
          // v0.23.50: 清空 stale generatedText。根因：若上一次续写/生成在
          // generatedText 留下了正文（纯文本幽灵段落），创世成功后 ChapterSwitch
          // 把正文加载进编辑器（HTML 排版版），stale generatedText 仍显示为幽灵
          // 段落 → 两份重复（一排版一纯文本）。正文已通过 ChapterSwitch 加载，
          // 幽灵文本必须清空。v0.23.37 曾清空后回滚，但日志确认根因正是 stale 文本。
          setGeneratedText('');
        } else if (isBackgroundBootstrap) {
          frontstageLogger.info('[SmartGeneration] Story created, first chapter in background');
          toast.success('故事已创建，第一章正在后台生成，完成后会自动加载', {
            duration: 5000,
          });
        }

        // 关键修复：空字符串在JS中是falsy，必须显式检查trim后的长度
        const hasContent = result.final_content && result.final_content.trim().length > 0;
        frontstageLogger.info('[DEBUG-dup] hasContent check', {
          hasContent,
          isBootstrapCompleted,
        });
        if (hasContent) {
          // v5.3.1 修复：Bootstrap 完成时内容已通过 ChapterSwitch 加载到编辑器，
          // 不要设置 generatedText，否则会出现正文+幽灵文本两份内容
          smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
          if (isBootstrapCompleted) {
            frontstageLogger.info(
              '[DEBUG-dup] isBootstrapCompleted=true, NOT setting generatedText'
            );
            toast.success('小说已创建！第一章已生成，您可以开始写作了');
          } else {
            // v5.4.0: 去除与当前编辑器内容重复的部分，防止 LLM 返回完整文本导致"重复输出"
            let finalContent = result.final_content!;
            const currentText = editorRef.current?.getText() || '';
            frontstageLogger.info('[DEBUG-dup] !! NON-bootstrap path, WILL setGeneratedText', {
              finalContent_length: finalContent.length,
              currentText_length: currentText.length,
              startsWith: currentText ? finalContent.startsWith(currentText) : false,
            });
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
          smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
        } else if (!result.success) {
          // 后端返回了失败
          smartExecuteInFlightRef.current = false;
          smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
          captureDiagnosticInfo('后端返回创作失败（success=false）');
          toast.error('创作失败：AI 未能生成内容，请检查模型配置或稍后重试');
        } else {
          // 后端返回了成功但没有内容 — 显示明确的错误提示（修复"没有提示地停止"）
          smartExecuteInFlightRef.current = false;
          smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
          captureDiagnosticInfo('后端返回成功但 final_content 为空');
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
                  // v0.23.66: 在 selectChapter 前强制清空 generatedText。
                  // 根因：line 2462 的 setGeneratedText('') 与 selectChapter 之间有
                  // async loadStories 间隙，期间可能被其他事件重新设置幽灵文本。
                  // 在内容加载点前再次清空是最可靠的时机。
                  setGeneratedText('');
                  // [DEBUG-dup] 回滚 v0.23.37 跳过 selectChapter —— 跳过会导致 currentChapter
                  // 未设置，后续渲染依赖可能崩溃（页面空白）。先用诊断日志定位根因再修。
                  frontstageLogger.info('[DEBUG-dup] story_created calling selectChapter', {
                    chapter_id: storyChapters[0].id,
                    content_length: storyChapters[0].content?.length ?? 0,
                    content_preview: storyChapters[0].content?.slice(0, 50) ?? 'EMPTY',
                    is_first_chapter_ready: isFirstChapterReady,
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
                    setGeneratedText(''); // v0.23.64: 清空幽灵文本，避免格式版+无格式版重复
                    setContent(autoFormatText(result.final_content));
                  }
                } else if (result.final_content && result.final_content.trim().length > 0) {
                  // v5.4.1 fix: 极端情况——DB 中没有章节但 result.final_content 有内容，直接显示
                  frontstageLogger.warn(
                    '[SmartGeneration] No chapters in DB but final_content exists, displaying content directly'
                  );
                  setGeneratedText(''); // v0.23.64: 清空幽灵文本，避免格式版+无格式版重复
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
        // v0.13.1: 弹出诊断卡片
        smartExecuteInFlightRef.current = false;
        smartExecuteNeedDiagnosticRef.current = false; // v0.13.3
        captureDiagnosticInfo(msg);
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
    [isGenerating, settings]
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

  // B2: 当前章节变化时更新 diff 基准（用于输入时增量更新全文字数）
  useEffect(() => {
    currentChapterPrevWordCountRef.current = currentChapter
      ? computeWordCount(currentChapter.content || '')
      : 0;
  }, [currentChapter, computeWordCount]);

  // B2: 全文字数由后端 SQL 聚合返回，避免将全量 chapters content 传到前端再 reduce
  useEffect(() => {
    if (!currentStory?.id) {
      setTotalWordCount(0);
      return;
    }
    let cancelled = false;
    loggedInvoke<{ total_chars: number }>('get_story_word_count', {
      story_id: currentStory.id,
    })
      .then(result => {
        if (!cancelled) setTotalWordCount(result.total_chars);
      })
      .catch(e => frontstageLogger.error('Failed to load story word count', { error: e }));
    return () => {
      cancelled = true;
    };
  }, [currentStory?.id]);

  // v0.13.1: 诊断信息转换为可复制的纯文本
  const diagnosticText = useMemo(() => {
    return Object.entries(diagnosticData)
      .map(([key, value]) => `${key}: ${value}`)
      .join('\n');
  }, [diagnosticData]);

  return (
    <>
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
          dbPoolStatus={dbPoolStatus ?? null}
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
                    setOrchestratorStatus(current =>
                      current?.message === message ? null : current
                    );
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
              isGenesis={!currentStory}
              inputValue={inputValue}
              ghostHint={ghostHint}
              hintSource={hintSource}
              gatewayModels={gatewayStatus?.models || []}
              allModels={allModels}
              isGatewayLoading={isGatewayLoading}
              onRefreshGateway={() => refetchGateway()}
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

      {/* v0.13.1: 智能创作异常中断诊断卡片 */}
      {showDiagnosticCard && (
        <div
          className="fixed inset-0 z-[9999] flex items-center justify-center"
          style={{ backgroundColor: 'rgba(0,0,0,0.55)' }}
          onClick={() => setShowDiagnosticCard(false)}
        >
          <div
            className="rounded-2xl p-6 max-w-lg w-full mx-4 shadow-2xl"
            style={{
              background: 'var(--parchment, #f5f0e1)',
              border: '1px solid var(--warm-sand, #d4c5a9)',
              color: 'var(--charcoal, #2c2c2c)',
            }}
            onClick={e => e.stopPropagation()}
          >
            <h3 className="text-lg font-bold mb-3" style={{ color: 'var(--charcoal, #2c2c2c)' }}>
              🐛 智能创作诊断报告
            </h3>
            <p className="text-sm mb-3" style={{ opacity: 0.7 }}>
              生成过程被中断。请复制下方信息发给开发者帮助排查。
            </p>
            <textarea
              readOnly
              className="w-full h-56 rounded-lg p-3 text-xs font-mono resize-none mb-3 bg-white/70 border"
              style={{
                borderColor: 'var(--warm-sand, #d4c5a9)',
                color: 'var(--charcoal, #2c2c2c)',
              }}
              value={diagnosticText}
              onClick={e => (e.target as HTMLTextAreaElement).select()}
            />
            <div className="flex gap-2 justify-end">
              <button
                className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                style={{
                  background: 'var(--warm-sand, #d4c5a9)',
                  color: 'var(--charcoal, #2c2c2c)',
                }}
                onClick={() => setShowDiagnosticCard(false)}
              >
                关闭
              </button>
              <button
                className="px-4 py-2 rounded-lg text-sm font-medium transition-colors text-white"
                style={{ background: 'var(--accent, #5b8c5a)' }}
                onClick={() => {
                  navigator.clipboard
                    .writeText(diagnosticText)
                    .then(() => {
                      toast.success('诊断信息已复制到剪贴板，请粘贴给开发者');
                    })
                    .catch(() => {
                      toast.error('复制失败，请手动选择文本框内容');
                    });
                }}
              >
                📋 复制诊断信息
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
};

export default FrontstageApp;
