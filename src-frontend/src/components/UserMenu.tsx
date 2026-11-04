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
        className="flex items-center gap-2 px-3 py-1.5 text-sm text-ai-ink-2 hover:text-ai-ink hover:bg-cinema-800 rounded-lg transition-colors"
      >
        <User className="w-4 h-4" />
        <span>登录</span>
      </button>
    );
  }

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-2 py-1 rounded-lg hover:bg-cinema-800 transition-colors"
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
        <span className="text-sm text-ai-ink-2 max-w-[120px] truncate">
          {user.display_name || user.email || 'User'}
        </span>
      </button>

      {isOpen && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setIsOpen(false)} />
          <div className="absolute right-0 top-full mt-1 w-48 bg-cinema-900 rounded-xl shadow-float border border-borderSubtle py-1 z-50">
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
