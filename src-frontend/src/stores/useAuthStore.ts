/**
 * Auth Store — 认证状态管理
 * v4.5.0
 */

import { create } from 'zustand';
import { createLogger } from '@/utils/logger';
import type { UserInfo, AuthConfig } from '@/services/auth';
import {
  getAuthConfig,
  getCurrentUser,
  logout as logoutApi,
  openOAuthBrowser,
  oauthCallback,
} from '@/services/auth';

const authLogger = createLogger('auth:store');

interface AuthState {
  // State
  user: UserInfo | null;
  isLoggedIn: boolean;
  isLoading: boolean;
  authConfig: AuthConfig | null;
  authToken: string | null;

  // Actions
  setUser: (user: UserInfo | null) => void;
  setAuthToken: (token: string | null) => void;
  login: (provider: string) => Promise<void>;
  handleOAuthCallback: (provider: string, code: string, state: string) => Promise<void>;
  logout: () => Promise<void>;
  checkAuth: () => Promise<void>;
  loadAuthConfig: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  user: null,
  isLoggedIn: false,
  isLoading: false,
  authConfig: null,
  authToken: localStorage.getItem('sf_auth_token'),

  setUser: user => set({ user, isLoggedIn: !!user }),

  setAuthToken: token => {
    if (token) {
      localStorage.setItem('sf_auth_token', token);
    } else {
      localStorage.removeItem('sf_auth_token');
    }
    set({ authToken: token });
  },

  login: async (provider: string) => {
    set({ isLoading: true });
    try {
      const resp = await openOAuthBrowser(provider);
      // 在桌面端，回调通过本地 HTTP 服务器接收
      // 这里返回的 resp 包含 redirect_port，前端需要轮询或监听该端口
      // 简化实现：等待用户手动触发回调处理
      authLogger.debug('OAuth started', { redirect_port: resp.redirect_port });
    } catch (error) {
      authLogger.error('Login failed', { error });
      throw error;
    } finally {
      set({ isLoading: false });
    }
  },

  handleOAuthCallback: async (provider: string, code: string, state: string) => {
    set({ isLoading: true });
    try {
      const user = await oauthCallback(provider, code, state);
      set({ user, isLoggedIn: true });
    } catch (error) {
      authLogger.error('OAuth callback failed', { error });
      throw error;
    } finally {
      set({ isLoading: false });
    }
  },

  logout: async () => {
    const { authToken, setAuthToken } = get();
    if (authToken) {
      try {
        await logoutApi(authToken);
      } catch (e) {
        authLogger.error('Logout API error', { error: e });
      }
    }
    setAuthToken(null);
    set({ user: null, isLoggedIn: false });
  },

  checkAuth: async () => {
    try {
      const user = await getCurrentUser();
      if (user) {
        set({ user, isLoggedIn: true });
      }
    } catch (e) {
      authLogger.error('Auth check failed', { error: e });
    }
  },

  loadAuthConfig: async () => {
    try {
      const config = await getAuthConfig();
      set({ authConfig: config });
    } catch (e) {
      authLogger.error('Failed to load auth config', { error: e });
    }
  },
}));
