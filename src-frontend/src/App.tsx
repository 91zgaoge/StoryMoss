import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useQueryClient } from '@tanstack/react-query';
import { Sidebar } from '@/components/Sidebar';
import { Dashboard } from '@/pages/Dashboard';
import { Stories } from '@/pages/Stories';
import { Characters } from '@/pages/Characters';
import { Scenes } from '@/pages/Scenes';
import { KnowledgeGraph } from '@/pages/KnowledgeGraph';
import { Skills } from '@/pages/Skills';
import { Mcp } from '@/pages/Mcp';
import { Settings } from '@/pages/Settings';
import { BookDeconstruction } from '@/pages/BookDeconstruction';
import { Tasks } from '@/pages/Tasks';
import { Foreshadowing } from '@/pages/Foreshadowing';
import { CreationWizard } from '@/pages/CreationWizard';
import { DataLoader } from '@/components/DataLoader';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { ConnectionStatus } from '@/components/ConnectionStatus';
import { FrontstageLauncher } from '@/components/FrontstageLauncher';
import { UpdateNotification } from '@/components/updater';
import { useUpdater } from '@/hooks/useUpdater';
import { LoginModal } from '@/pages/Login';
import { useAppStore } from '@/stores/appStore';
import type { ViewType, Story } from '@/types';
import toast from 'react-hot-toast';

function App() {
  const queryClient = useQueryClient();
  const [currentView, setCurrentView] = useState<ViewType>('dashboard');
  const [isFrontstageOpen, setIsFrontstageOpen] = useState(false);
  const [isLoginOpen, setIsLoginOpen] = useState(false);
  // v5.0.0 修复：用于强制 React 重新渲染的 key
  const [renderKey, setRenderKey] = useState(0);

  // 自动更新检测
  const {
    currentVersion,
    hasUpdate,
    latestVersion,
    updateInfo,
    isInstalling,
    error,
    checkUpdate,
    installUpdate,
    dismissUpdate,
  } = useUpdater(true);

  // Check if we're in frontstage mode (via URL or window label)
  useEffect(() => {
    const checkFrontstage = () => {
      const url = window.location.href;
      const isFrontstage = url.includes('frontstage') ||
                          (window as any).__TAURI__?.window?.label === 'frontstage';
      setIsFrontstageOpen(isFrontstage);
    };

    checkFrontstage();
  }, []);

  // 监听登录弹窗事件
  useEffect(() => {
    const handleShowLogin = () => setIsLoginOpen(true);
    window.addEventListener('show-login-modal', handleShowLogin);
    return () => window.removeEventListener('show-login-modal', handleShowLogin);
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
            case 'FrontstageClosed':
              setIsFrontstageOpen(false);
              break;
            case 'FrontstageFocused':
              setIsFrontstageOpen(true);
              break;
            case 'DataRefresh':
              const entity = payload?.entity || 'data';
              toast(`幕后${entity}已更新`, { icon: '🔄' });
              // 强制刷新相关 TanStack Query 缓存
              queryClient.invalidateQueries({ queryKey: ['stories'] });
              queryClient.invalidateQueries({ queryKey: ['characters'] });
              queryClient.invalidateQueries({ queryKey: ['scenes'] });
              queryClient.invalidateQueries({ queryKey: ['foreshadowings'] });
              queryClient.invalidateQueries({ queryKey: ['story-outlines'] });
              queryClient.invalidateQueries({ queryKey: ['world-building'] });
              queryClient.invalidateQueries({ queryKey: ['knowledge-graph'] });
              queryClient.invalidateQueries({ queryKey: ['character-relationships'] });
              window.dispatchEvent(new CustomEvent('backstage-data-refreshed', { detail: entity }));
              break;
            case 'NavigateTo':
              const targetView = payload?.view || 'dashboard';
              setCurrentView(targetView as ViewType);
              if (payload?.highlight_story_id) {
                window.dispatchEvent(
                  new CustomEvent('backstage-navigate-to-story', {
                    detail: { storyId: payload.highlight_story_id },
                  })
                );
              }
              break;
          }
        });
      } catch (e) {
        console.error('Failed to setup backstage listener:', e);
      }
    };

    setupListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // v5.0.0 修复：窗口重新可见时自动恢复数据和渲染
  useEffect(() => {
    const handleWindowShown = async (retries = 3) => {
      // 窗口重新可见时，强制刷新故事列表并恢复 currentStory
      try {
        const stories = await invoke<Story[]>('list_stories');
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
        // 强制刷新所有 TanStack Query 缓存，确保各页面重新获取数据
        queryClient.invalidateQueries({ queryKey: ['stories'] });
        queryClient.invalidateQueries({ queryKey: ['characters'] });
        queryClient.invalidateQueries({ queryKey: ['scenes'] });
        queryClient.invalidateQueries({ queryKey: ['foreshadowings'] });
        queryClient.invalidateQueries({ queryKey: ['story-outlines'] });
        queryClient.invalidateQueries({ queryKey: ['world-building'] });
        queryClient.invalidateQueries({ queryKey: ['knowledge-graph'] });
        queryClient.invalidateQueries({ queryKey: ['character-relationships'] });
        // 触发全局数据刷新事件，让各页面重新获取数据
        window.dispatchEvent(new CustomEvent('backstage-data-refreshed', { detail: 'all' }));
        // 强制触发 resize 帮助重绘
        window.dispatchEvent(new Event('resize'));
        // 强制 React 重新渲染当前视图
        setRenderKey(k => k + 1);
      } catch (e) {
        console.error('Failed to refresh on window shown:', e);
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
        unlistenShown = await listen('backstage-shown', () => handleWindowShown());
      } catch (e) {
        console.error('Failed to setup backstage-shown listener:', e);
      }
    };
    setupShownListener();

    return () => {
      if (unlistenShown) unlistenShown();
    };
  }, [queryClient]);

  const renderView = () => {
    switch (currentView) {
      case 'dashboard': return <Dashboard />;
      case 'stories': return <Stories />;
      case 'characters': return <Characters />;
      case 'scenes': return <Scenes />;
      case 'knowledge-graph': return <KnowledgeGraph />;
      case 'skills': return <Skills />;
      case 'mcp': return <Mcp />;
      case 'settings': return <Settings />;
      case 'book-deconstruction': return <BookDeconstruction />;
      case 'tasks': return <Tasks />;
      case 'foreshadowing': return <Foreshadowing />;
      default: return <Dashboard />;
    }
  };

  return (
    <ErrorBoundary>
      <div className="flex h-screen bg-cinema-950 film-grain">
        <DataLoader />
        <ConnectionStatus />
        <UpdateNotification
          isOpen={hasUpdate}
          currentVersion={currentVersion}
          latestVersion={latestVersion}
          updateInfo={updateInfo}
          isInstalling={isInstalling}
          error={error}
          onInstall={installUpdate}
          onDismiss={dismissUpdate}
          onCheck={checkUpdate}
        />
        <FrontstageLauncher
          isOpen={isFrontstageOpen}
          onToggle={() => setIsFrontstageOpen(!isFrontstageOpen)}
        />
        <LoginModal isOpen={isLoginOpen} onClose={() => setIsLoginOpen(false)} />
        <Sidebar currentView={currentView} onNavigate={setCurrentView} />
        <main key={renderKey} className="flex-1 overflow-auto">
          {renderView()}
        </main>
      </div>
    </ErrorBoundary>
  );
}

export default App;
