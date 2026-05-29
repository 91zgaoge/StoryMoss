/**
 * Auth Service — 认证相关 IPC 调用
 * v4.5.0
 */

import { loggedInvoke } from '@/services/tauri';
import { open } from '@tauri-apps/plugin-shell';

export interface AuthConfig {
  google_enabled: boolean;
  github_enabled: boolean;
  wechat_enabled: boolean;
  qq_enabled: boolean;
}

export interface UserInfo {
  id: string;
  email?: string;
  display_name?: string;
  avatar_url?: string;
}

export interface OAuthStartResponse {
  auth_url: string;
  state: string;
  redirect_port: number;
}

/**
 * 获取认证配置
 */
export const getAuthConfig = () => loggedInvoke<AuthConfig>('get_auth_config');

/**
 * 开始 OAuth 登录流程
 */
export const oauthStart = (provider: string) =>
  loggedInvoke<OAuthStartResponse>('oauth_start', { provider });

/**
 * OAuth 回调处理（桌面端通过本地 HTTP 服务器接收后调用）
 */
export const oauthCallback = (provider: string, code: string, state: string) =>
  loggedInvoke<UserInfo>('oauth_callback', { provider, code, state });

/**
 * 获取当前登录用户
 */
export const getCurrentUser = () => loggedInvoke<UserInfo | null>('get_current_user');

/**
 * 注销登录
 */
export const logout = (token: string) => loggedInvoke<void>('logout', { token });

/**
 * 打开系统浏览器进行 OAuth 授权
 */
export const openOAuthBrowser = async (provider: string): Promise<OAuthStartResponse> => {
  const resp = await oauthStart(provider);
  await open(resp.auth_url);
  return resp;
};
