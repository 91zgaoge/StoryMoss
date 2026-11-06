/**
 * UserMenu — 用户头像下拉菜单
 * v4.5.0
 */

import { useState } from 'react';
import { useAuthStore } from '@/stores/useAuthStore';
import { useAppStore } from '@/stores/appStore';
import { LogOut, User, Settings } from 'lucide-react';

export function UserMenu() {
  const { user, isLoggedIn, logout } = useAuthStore();
  const [isOpen, setIsOpen] = useState(false);

  if (!isLoggedIn || !user) {
    return (
      <button
        onClick={() => useAppStore.getState().setLoginModalOpen(true)}
        className="flex w-full items-center justify-center gap-2 rounded-lg px-2 py-2 text-sm text-ai-ink-2 transition-colors hover:bg-cinema-800 hover:text-ai-ink lg:justify-start"
        title="登录"
      >
        <User className="h-4 w-4 shrink-0" />
        <span className="hidden whitespace-nowrap lg:inline">登录</span>
      </button>
    );
  }

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex w-full items-center justify-center gap-2 rounded-lg px-2 py-1.5 transition-colors hover:bg-cinema-800 lg:justify-start"
      >
        {user.avatar_url ? (
          <img
            src={user.avatar_url}
            alt={user.display_name || 'User'}
            className="w-7 h-7 rounded-full object-cover"
          />
        ) : (
          <div className="w-7 h-7 rounded-full bg-ai-field flex items-center justify-center text-ai-ink-2 text-xs font-medium">
            {(user.display_name || user.email || 'U').charAt(0).toUpperCase()}
          </div>
        )}
        <span className="hidden max-w-[120px] truncate text-sm text-ai-ink-2 lg:inline">
          {user.display_name || user.email || 'User'}
        </span>
      </button>

      {isOpen && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setIsOpen(false)} />
          <div className="absolute bottom-full left-0 z-50 mb-2 w-48 rounded-xl border border-borderSubtle bg-cinema-900 py-1 shadow-float">
            <div className="px-3 py-2 border-b border-borderSubtle">
              <p className="text-sm font-medium text-ai-ink truncate">
                {user.display_name || 'User'}
              </p>
              <p className="text-xs text-ai-ink-3 truncate">{user.email || ''}</p>
            </div>

            <button
              onClick={() => {
                setIsOpen(false);
                useAppStore.getState().setSettingsTab('account');
                useAppStore.getState().setCurrentView('settings');
              }}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-ai-ink-2 hover:bg-cinema-800 transition-colors"
            >
              <Settings className="w-4 h-4" />
              <span>账号设置</span>
            </button>

            <button
              onClick={() => {
                setIsOpen(false);
                logout();
              }}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-status-danger hover:bg-status-danger/10 transition-colors"
            >
              <LogOut className="w-4 h-4" />
              <span>退出登录</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}
