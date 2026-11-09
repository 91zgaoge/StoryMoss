import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { loggedInvoke } from '@/services/tauri';
import { confirmDialog } from '@/components/ui/confirm';
import { createLogger } from '@/utils/logger';
import { useQueryClient } from '@tanstack/react-query';
import { Sidebar } from '@/components/Sidebar';
import { Dashboard } from '@/pages/Dashboard';
import { Stories } from '@/pages/Stories';
import { Characters } from '@/pages/Characters';
import { Scenes } from '@/pages/Scenes';
import Corpus from '@/pages/Corpus';
import { WorldBuilding } from '@/pages/WorldBuilding';
import { KnowledgeGraph } from '@/pages/KnowledgeGraph';
import { Skills } from '@/pages/Skills';
import { Settings } from '@/pages/Settings';
import { BookDeconstruction } from '@/pages/BookDeconstruction';
import { Tasks } from '@/pages/Tasks';
import { CascadeCenter } from '@/pages/CascadeCenter';
import { Maintenance } from '@/pages/Maintenance';
import { Foreshadowing } from '@/pages/Foreshadowing';
import { NarrativeAnalysis } from '@/pages/NarrativeAnalysis';
import { StorySystem } from '@/pages/StorySystem';
import { Insights } from '@/pages/Insights';
import { IntentionGraphDiagnostics } from '@/pages/IntentionGraphDiagnostics';
import AgencyEval from '@/pages/AgencyEval';
import TrainingLab from '@/pages/TrainingLab';
import AgencyLearning from '@/pages/AgencyLearning';
import AgencyStudio from '@/pages/AgencyStudio';
import { Logs } from '@/pages/Logs';
import { TracingPanel } from '@/pages/TracingPanel';
import { DataLoader } from '@/components/DataLoader';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { UpdateNotification } from '@/components/updater';
import { useUpdater } from '@/hooks/useUpdater';
import { useSyncStore } from '@/hooks/useSyncStore';
import { useWorkflowNodes } from '@/hooks/useWorkflowNodes';
import { useWebViewRedrawFix } from '@/hooks/useWebViewRedrawFix';
import { useBackstageTheme } from '@/hooks/useBackstageTheme';
import { LoginModal } from '@/pages/Login';
import { ConfirmPortal } from '@/components/ui/confirm';
import { useAppStore } from '@/stores/appStore';
import {
  useAgencyActivityStore,
  type AgentActivityEvent,
  type AgentProgressEvent,
} from '@/stores/agencyActivityStore';
import type { ViewType, Story } from '@/types';
import toast from 'react-hot-toast';

const FULL_HEIGHT_VIEWS = new Set<ViewType>([
  'dashboard',
  'settings',
  'scenes',
  'corpus',
  'knowledge-graph',
  'book-deconstruction',
  'guidebook-distillation',
  'story-system',
  'agency-studio',
  'logs',
  'tracing',
]);

function App() {
  const queryClient = useQueryClient();

  // v5.1.0: Zustand↔TanStack Query 状态同步 — currentStory 变化时自动刷新关联数据
  const currentStory = useAppStore(state => state.currentStory);
  useEffect(() => {
    if (currentStory?.id) {
      // Cancel stale requests before invalidating to prevent race conditions
      queryClient.cancelQueries({ queryKey: ['characters', currentStory.id] });
      queryClient.cancelQueries({ queryKey: ['scenes', currentStory.id] });
      queryClient.cancelQueries({ queryKey: ['chapters', currentStory.id] });
      // Batch invalidate related queries to reduce IPC waterfall
      queryClient.invalidateQueries({ queryKey: ['characters', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['scenes', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['chapters', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['world_building', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['foreshadowings', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['story-outline', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['knowledge-graph', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['character-relationships', currentStory.id] });
      // v0.74.0: 语料 / 剧组对话 / 任务现场随故事切换
      queryClient.invalidateQueries({ queryKey: ['corpus', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['corpus-detail'] });
      queryClient.invalidateQueries({ queryKey: ['agency-chat-sessions', currentStory.id] });
      queryClient.invalidateQueries({ queryKey: ['agency-chat'] });
      queryClient.invalidateQueries({ queryKey: ['agency-crew', currentStory.id] });
    }
  }, [currentStory?.id, queryClient]);

  // v5.4.0: 监听 Workflow 节点级执行事件（日志/调试用途）
  useWorkflowNodes();

  // 统一实时状态同步中心：监听后端数据变更事件，自动刷新缓存
  useSyncStore({
    onStoryCreated: storyId => {
      toast.success('新故事已创建');
    },
    onStoryDeleted: () => {
      toast('故事已删除', { icon: '🗑️' });
    },
  });

  // 导航统一走 appStore，避免页面 setCurrentView 与壳层 local state 失步
  const currentView = useAppStore(state => state.currentView);
  const setCurrentView = useAppStore(state => state.setCurrentView);
  // W2-F2: 从 local state 迁移到 Zustand store，替代 DOM CustomEvent 通信
  const isLoginOpen = useAppStore(state => state.isLoginModalOpen);
  // W2-F3: WebView 重绘修复 — 封装 DOM hack 为可复用 hook
  const { mainRef, forceRedraw } = useWebViewRedrawFix();

  // 自动更新检测
  const {
    currentVersion,
    hasUpdate,
    latestVersion,
    updateInfo,
    isInstalling,
    downloadProgress,
    error,
    checkUpdate,
    installUpdate,
    dismissUpdate,
  } = useUpdater(true);

  // W2-F2: show-login-modal 已废弃，改用 Zustand store (isLoginModalOpen / setLoginModalOpen)
  // 原 window.addEventListener('show-login-modal', ...) 已移除

  // 幕后也监听 genesis-warnings（非致命错误 toast + 刷新 Genesis 列表）
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      try {
        unlisten = await listen<{
          session_id: string;
          story_id: string;
          count: number;
          has_error: boolean;
          message: string;
        }>('genesis-warnings', event => {
          const p = event.payload;
          if (p.has_error) {
            toast.error(p.message, { duration: 6000 });
          } else {
            toast(p.message, { icon: '⚠️', duration: 5000 });
          }
          queryClient.invalidateQueries({ queryKey: ['genesis-runs'] });
          window.dispatchEvent(new CustomEvent('genesis-warnings-received'));
        });
      } catch (e) {
        createLogger('ui:App').error('Failed to setup genesis-warnings listener', { error: e });
      }
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [queryClient]);

  // v0.93.3: 后端自动提议上传日志包——仅幕后窗口响应，confirmDialog 同意才
  // 上传（后端不静默上传）；拒绝记当天，当日不再提示。
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const today = () => new Date().toISOString().slice(0, 10);
    const setup = async () => {
      try {
        unlisten = await listen<{
          filename: string;
          file_path: string;
          sha256: string;
          size_bytes: number;
          file_count: number;
          error_count: number;
        }>('log-upload-requested', async event => {
          // 双窗口共用 App 壳：仅幕后窗口弹窗（浏览器/mock 环境默认响应）
          let isBackstage = true;
          try {
            isBackstage = getCurrentWindow().label === 'backstage';
          } catch {
            isBackstage = true;
          }
          if (!isBackstage) return;
          if (localStorage.getItem('log-upload-declined-date') === today()) return;
          const p = event.payload;
          const ok = await confirmDialog({
            title: '上传日志包协助诊断？',
            description: `检测到 ${p.error_count} 条新错误日志，已打包 ${p.filename}（${(p.size_bytes / 1024 / 1024).toFixed(1)} MB，${p.file_count} 个文件）。上传后服务端仅保留 72 小时，用于定位崩溃与错误。`,
            confirmText: '同意上传',
            cancelText: '今天不上传',
          });
          if (!ok) {
            localStorage.setItem('log-upload-declined-date', today());
            toast('已跳过，今天不再提示日志上传', { icon: '📤' });
            return;
          }
          try {
            const res = await loggedInvoke<{ accepted: boolean; duplicate: boolean }>(
              'upload_log_report',
              { file_path: p.file_path, sha256: p.sha256 }
            );
            if (res.duplicate) toast.success('日志包已存在（服务端去重），无需重复上传');
            else toast.success('日志包已上传，感谢协助改进');
          } catch (e) {
            toast.error(`日志上传失败：${(e as Error).message}`);
          }
        });
      } catch (e) {
        createLogger('ui:App').error('Failed to setup log-upload-requested listener', { error: e });
      }
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 监听 backstage-update 事件（幕前 → 幕后联动）
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen('backstage-update', (event: any) => {
          const { type, payload } = event.payload || {};
          switch (type) {
            case 'ContentChanged':
              toast('幕前内容已更新', { icon: '📝' });
              break;
            case 'GenerationRequested':
              toast('幕前请求生成内容', { icon: '✨' });
              break;
            // DataRefresh 已统一由 useSyncStore 处理，避免重复刷新
            case 'NavigateTo':
              const targetView = payload?.view || 'dashboard';
              setCurrentView(targetView as ViewType);
              if (payload?.highlight_story_id) {
                // W2-F2: 替代 backstage-navigate-to-story DOM CustomEvent
                useAppStore.getState().setNavigateHighlightStoryId(payload.highlight_story_id);
              }
              // 若指定了具体面板（如 settings/editor），通知目标页面做二次定位
              if (payload?.open_panel) {
                window.dispatchEvent(
                  new CustomEvent('backstage-navigate-to-panel', {
                    detail: { view: targetView, panel: payload.open_panel },
                  })
                );
              }
              break;
          }
        });
      } catch (e) {
        createLogger('ui:App').error('Failed to setup backstage listener', { error: e });
      }
    };

    setupListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // v5.2.0 修复：窗口重新可见时自动恢复数据和渲染
  useEffect(() => {
    const handleWindowShown = async (retries = 3) => {
      // 窗口重新可见时，强制刷新故事列表并恢复 currentStory
      try {
        const stories = await loggedInvoke<Story[]>('list_stories');
        if (stories.length > 0) {
          const { currentStory, stories: oldStories } = useAppStore.getState();
          // 检测是否有新故事（旧列表中不存在的），有则自动切换到新故事
          const newStory = stories.find(s => !oldStories.some(os => os.id === s.id));
          if (newStory) {
            useAppStore.getState().setCurrentStory(newStory);
          } else if (!currentStory) {
            useAppStore.getState().setCurrentStory(stories[0]);
          }
          useAppStore.getState().setStories(stories);
        }
        // v5.1.0: 简化刷新逻辑 — stories 刷新后，currentStory useEffect 会自动刷新关联数据
        queryClient.invalidateQueries({ queryKey: ['stories'] });
        // v5.4.0: 窗口恢复时，若 currentStory 存在，同时刷新关联数据缓存
        const { currentStory: cs } = useAppStore.getState();
        if (cs?.id) {
          queryClient.invalidateQueries({ queryKey: ['characters', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['scenes', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['chapters', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['world_building', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['foreshadowings', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['story-outline', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['knowledge-graph', cs.id] });
          queryClient.invalidateQueries({ queryKey: ['character-relationships', cs.id] });
        }
        // 触发全局数据刷新事件，让各页面重新获取数据
      } catch (e) {
        createLogger('ui:App').error('Failed to refresh on window shown', { error: e });
        if (retries > 0) {
          setTimeout(() => handleWindowShown(retries - 1), 500);
        }
      }
    };

    // 组件 mount 时执行一次
    handleWindowShown();

    // 监听 backstage-shown 事件（Rust 端 show_backstage 命令在窗口显示后发射）
    let unlistenShown: (() => void) | undefined;
    const setupShownListener = async () => {
      try {
        unlistenShown = await listen('backstage-shown', event => {
          // P1-14 修复: 使用事件中的 story_id 精准定位当前故事
          const payload = event.payload as { story_id?: string } | undefined;
          if (payload?.story_id) {
            useAppStore
              .getState()
              .setCurrentStory(
                useAppStore.getState().stories.find(s => s.id === payload.story_id) || null
              );
          }
          handleWindowShown();
          forceRedraw();
        });
      } catch (e) {
        createLogger('ui:App').error('Failed to setup backstage-shown listener', { error: e });
      }
    };
    setupShownListener();

    // 监听前端自定义事件（Rust eval 触发）
    const handleWindowRestored = () => {
      forceRedraw();
    };
    window.addEventListener('backstage-window-restored', handleWindowRestored);

    return () => {
      if (unlistenShown) unlistenShown();
      window.removeEventListener('backstage-window-restored', handleWindowRestored);
    };
  }, [queryClient]);

  // 全局捕获 agency 三 agent 事件到 store（P0 修复）：
  // AgencyStudio 仅在 currentView==='agency-studio' 时挂载，组件内监听随卸载销毁，
  // 创世/续写期间（页面未开）事件全丢 -> 打开页面只能空白等 10s 轮询。
  // 监听器提升到常驻顶层后事件流持续进 store，页面打开即可见实时动态。
  useEffect(() => {
    let unActivity: (() => void) | undefined;
    let unProgress: (() => void) | undefined;
    let unBoard: (() => void) | undefined;
    let unChat: (() => void) | undefined;
    let unCrew: (() => void) | undefined;
    let unLease: (() => void) | undefined;
    let unDirector: (() => void) | undefined;
    const setup = async () => {
      try {
        unActivity = await listen<Omit<AgentActivityEvent, 'at'>>('agency-agent-activity', e => {
          useAgencyActivityStore.getState().appendActivity(e.payload);
        });
        unProgress = await listen<Omit<AgentProgressEvent, 'at'>>('agency-run-progress', e => {
          useAgencyActivityStore.getState().appendProgress(e.payload);
        });
        unBoard = await listen<{ run_id: string }>('agency-board-changed', e => {
          useAgencyActivityStore.getState().setActiveRunId(e.payload.run_id);
          queryClient.invalidateQueries({ queryKey: ['agency-board', e.payload.run_id] });
        });
        // v0.74.0: 导演/剧组事件只做缓存失效；消息与任务本身已落库，轮询兜底。
        unChat = await listen<{ session_id: string }>('agency-chat-update', () => {
          queryClient.invalidateQueries({ queryKey: ['agency-chat'] });
          queryClient.invalidateQueries({ queryKey: ['agency-chat-sessions'] });
        });
        unCrew = await listen<{ story_id: string }>('agency-crew-update', () => {
          queryClient.invalidateQueries({ queryKey: ['agency-crew'] });
        });
        unLease = await listen<{ story_id: string }>('agency-lease-update', () => {
          queryClient.invalidateQueries({ queryKey: ['agency-crew'] });
        });
        unDirector = await listen<{ action_id: string }>('agency-director-action', () => {
          queryClient.invalidateQueries({ queryKey: ['agency-chat'] });
          queryClient.invalidateQueries({ queryKey: ['agency-crew'] });
        });
      } catch (e) {
        createLogger('ui:App').error('Failed to setup agency event listeners', { error: e });
      }
    };
    setup();
    return () => {
      unActivity?.();
      unProgress?.();
      unBoard?.();
      unChat?.();
      unCrew?.();
      unLease?.();
      unDirector?.();
    };
  }, [queryClient]);

  // 幕后深色调主题：挂载即应用，并监听 storage / Tauri 双通道实时切换
  useBackstageTheme();

  const renderView = () => {
    switch (currentView) {
      case 'dashboard':
        return <Dashboard />;
      case 'stories':
        return <Stories />;
      case 'characters':
        return <Characters />;
      case 'world_building':
        return <WorldBuilding />;
      case 'scenes':
        return <Scenes />;
      case 'corpus':
        return <Corpus />;
      case 'knowledge-graph':
        return <KnowledgeGraph />;
      case 'skills':
        return <Skills />;
      case 'mcp':
        // v0.26.40: MCP 已迁至设置「扩展」；兜底仍渲染 Settings
        return <Settings />;
      case 'settings':
        return <Settings />;
      case 'book-deconstruction':
        return <BookDeconstruction />;
      case 'guidebook-distillation':
        return <BookDeconstruction initialTab="guidebooks" />;
      case 'tasks':
        return <Tasks />;
      case 'cascade-center':
        return <CascadeCenter />;
      case 'maintenance':
        return <Maintenance />;
      case 'foreshadowing':
        return <Foreshadowing />;
      case 'narrative-analysis':
        return <NarrativeAnalysis />;
      case 'story-system':
        return <StorySystem />;
      case 'usage-stats':
      case 'writing-stats':
        return <Insights />;
      case 'intention-graph':
        return <IntentionGraphDiagnostics />;
      case 'agency-eval':
        return <AgencyEval />;
      case 'training-lab':
        return <TrainingLab />;
      case 'agency-learning':
        return <AgencyLearning />;
      case 'agency-studio':
        return <AgencyStudio />;
      case 'logs':
        return <Logs />;
      case 'tracing':
        return <TracingPanel />;
      default:
        return <Dashboard />;
    }
  };

  return (
    <ErrorBoundary>
      <div className="flex h-dvh min-h-0 overflow-hidden bg-cinema-950 film-grain dark">
        <DataLoader />
        <ConfirmPortal />
        <UpdateNotification
          isOpen={hasUpdate}
          currentVersion={currentVersion}
          latestVersion={latestVersion}
          updateInfo={updateInfo}
          isInstalling={isInstalling}
          downloadProgress={downloadProgress}
          error={error}
          onInstall={installUpdate}
          onDismiss={dismissUpdate}
          onCheck={checkUpdate}
        />
        <LoginModal
          isOpen={isLoginOpen}
          onClose={() => useAppStore.getState().setLoginModalOpen(false)}
        />
        {currentView !== 'dashboard' && currentView !== 'settings' && (
          <Sidebar currentView={currentView} onNavigate={setCurrentView} />
        )}
        <main ref={mainRef} className="min-w-0 min-h-0 flex-1 overflow-hidden">
          <div
            key={currentView}
            className={`animate-fade-in min-w-0 min-h-0 ${
              FULL_HEIGHT_VIEWS.has(currentView)
                ? 'h-full overflow-hidden'
                : 'h-full overflow-y-auto'
            }`}
          >
            <ErrorBoundary key={currentView}>{renderView()}</ErrorBoundary>
          </div>
        </main>
      </div>
    </ErrorBoundary>
  );
}

export default App;
