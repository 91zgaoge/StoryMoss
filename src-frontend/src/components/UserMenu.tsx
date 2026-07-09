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
        className="flex items-center gap-2 px-3 py-1.5 text-sm text-stone-600 hover:text-stone-900 hover:bg-stone-100 rounded-md transition-colors"
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
        className="flex items-center gap-2 px-2 py-1 rounded-md hover:bg-stone-100 transition-colors"
      >
        {user.avatar_url ? (
          <img
            src={user.avatar_url}
            alt={user.display_name || 'User'}
            className="w-7 h-7 rounded-full object-cover"
          />
        ) : (
          <div className="w-7 h-7 rounded-full bg-stone-200 flex items-center justify-center text-stone-600 text-xs font-medium">
            {(user.display_name || user.email || 'U').charAt(0).toUpperCase()}
          </div>
        )}
        <span className="text-sm text-stone-700 max-w-[120px] truncate">
          {user.display_name || user.email || 'User'}
        </span>
      </button>

      {isOpen && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setIsOpen(false)} />
          <div className="absolute right-0 top-full mt-1 w-48 bg-white rounded-lg shadow-lg border border-stone-200 py-1 z-50">
            <div className="px-3 py-2 border-b border-stone-100">
              <p className="text-sm font-medium text-stone-800 truncate">
                {user.display_name || 'User'}
              </p>
              <p className="text-xs text-stone-500 truncate">{user.email || ''}</p>
            </div>

            <button
              onClick={() => {
                setIsOpen(false);
                useAppStore.getState().setSettingsTab('account');
                useAppStore.getState().setCurrentView('settings');
              }}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-stone-700 hover:bg-stone-50 transition-colors"
            >
              <Settings className="w-4 h-4" />
              <span>账号设置</span>
            </button>

            <button
              onClick={() => {
                setIsOpen(false);
                logout();
              }}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-red-600 hover:bg-red-50 transition-colors"
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
