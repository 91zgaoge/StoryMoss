import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
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
  const [currentView, setCurrentView] = useState<ViewType>('dashboard');
  const [isFrontstageOpen, setIsFrontstageOpen] = useState(false);
  const [isLoginOpen, setIsLoginOpen] = useState(false);

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
    const handleWindowShown = async () => {
      // 窗口重新可见时，强制刷新故事列表并恢复 currentStory
      try {
        const stories = await invoke<Story[]>('list_stories');
        if (stories.length > 0) {
          const { currentStory } = useAppStore.getState();
          if (!currentStory) {
            // 如果没有当前故事，自动选择第一个
            useAppStore.getState().setCurrentStory(stories[0]);
          }
          useAppStore.getState().setStories(stories);
        }
        // 触发全局数据刷新事件，让各页面重新获取数据
        window.dispatchEvent(new CustomEvent('backstage-data-refreshed', { detail: 'all' }));
      } catch (e) {
        console.error('Failed to refresh on window shown:', e);
      }
    };

    // 组件 mount 时执行一次
    handleWindowShown();

    // 监听 Tauri 窗口焦点事件（窗口从隐藏到显示时会触发）
    let unlistenFocus: (() => void) | undefined;
    const setupFocusListener = async () => {
      try {
        unlistenFocus = await listen('tauri://focus', handleWindowShown);
      } catch (e) {
        // tauri://focus 可能不可用，忽略错误
      }
    };
    setupFocusListener();

    return () => {
      if (unlistenFocus) unlistenFocus();
    };
  }, []);

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
        <main className="flex-1 overflow-auto">
          {renderView()}
        </main>
      </div>
    </ErrorBoundary>
  );
}

export default App;
