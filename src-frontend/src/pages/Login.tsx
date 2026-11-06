/**
 * Login Page — 登录弹窗/页面
 * v4.5.0
 */

import { useState, useEffect } from 'react';
import { X, Chrome, Github, MessageCircle, Loader2 } from 'lucide-react';
import { useAuthStore } from '@/stores/useAuthStore';
import { extractMessage } from '@/utils/errorHandler';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/controls';
import toast from 'react-hot-toast';

interface LoginModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function LoginModal({ isOpen, onClose }: LoginModalProps) {
  const { authConfig, login, isLoading, isWaitingForOAuth, cancelLogin } = useAuthStore();
  const [invite, setInvite] = useState('');

  useEffect(() => {
    if (isOpen) {
      useAuthStore.getState().loadAuthConfig();
    }
  }, [isOpen]);

  const handleLogin = async (provider: string) => {
    try {
      await login(provider, invite.trim() || undefined);
      if (useAuthStore.getState().isLoggedIn) {
        toast.success('登录成功');
        onClose();
      }
    } catch (error) {
      const message = extractMessage(error);
      // 以 server 失败码为准；正则兜底兼容旧文案
      const code = /AUTH_FAILED:([a-z_]+)/.exec(message)?.[1];
      if (code === 'invalid_or_used_invite' || /邀请|invite/i.test(message)) {
        toast.error('邀请码无效或已使用');
      } else {
        toast.error(`登录失败: ${message}`);
      }
    }
  };

  return (
    <Modal open={isOpen} onClose={onClose} title="登录 StoryMoss" size="sm">
      {/* Header */}
      <p className="text-center text-sm text-ai-ink-3 mb-6">登录后可解锁云同步等跨设备功能</p>

      {/* OAuth Buttons / 等待授权 */}
      {isWaitingForOAuth ? (
        <div className="text-center py-6 space-y-4">
          <Loader2 className="w-8 h-8 mx-auto animate-spin text-ai-ink-3" />
          <p className="text-sm text-ai-ink-3">等待浏览器授权…</p>
          <button
            onClick={cancelLogin}
            className="px-4 py-2 text-sm text-ai-ink-2 border border-borderSubtle rounded-lg hover:bg-cinema-700 hover:border-borderSubtle transition-all"
          >
            取消
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          {/* 邀请码（新用户注册必填） */}
          <Input
            type="text"
            value={invite}
            onChange={e => setInvite(e.target.value)}
            placeholder="邀请码（新用户注册必填）"
            disabled={isLoading}
          />

          {authConfig?.google_enabled && (
            <button
              onClick={() => handleLogin('google')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-cinema-800 border border-borderSubtle rounded-lg text-ai-ink-2 hover:bg-cinema-700 hover:border-borderSubtle transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Chrome className="w-5 h-5 text-blue-500" />
              <span className="text-sm font-medium">使用 Google 登录</span>
            </button>
          )}

          {authConfig?.github_enabled && (
            <button
              onClick={() => handleLogin('github')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-cinema-800 border border-borderSubtle rounded-lg text-ai-ink-2 hover:bg-cinema-700 hover:border-borderSubtle transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Github className="w-5 h-5 text-ai-ink" />
              <span className="text-sm font-medium">使用 GitHub 登录</span>
            </button>
          )}

          {/* WeChat — 预留，未启用时显示提示 */}
          {authConfig?.wechat_enabled && (
            <button
              onClick={() => handleLogin('wechat')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-cinema-800 border border-borderSubtle rounded-lg text-ai-ink-2 hover:bg-cinema-700 hover:border-borderSubtle transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <MessageCircle className="w-5 h-5 text-green-500" />
              <span className="text-sm font-medium">使用微信登录</span>
            </button>
          )}

          {/* QQ — 预留 */}
          {authConfig?.qq_enabled && (
            <button
              onClick={() => handleLogin('qq')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-cinema-800 border border-borderSubtle rounded-lg text-ai-ink-2 hover:bg-cinema-700 hover:border-borderSubtle transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <MessageCircle className="w-5 h-5 text-blue-400" />
              <span className="text-sm font-medium">使用 QQ 登录</span>
            </button>
          )}

          {/* 无可用provider时的提示 */}
          {authConfig &&
            !authConfig.google_enabled &&
            !authConfig.github_enabled &&
            !authConfig.wechat_enabled &&
            !authConfig.qq_enabled && (
              <div className="text-center py-4">
                <p className="text-sm text-ai-ink-3">尚未配置 OAuth 登录选项</p>
                <p className="text-xs text-ai-ink-3 mt-1">请在设置中配置 OAuth 客户端信息</p>
              </div>
            )}
        </div>
      )}

      {/* Footer */}
      <div className="mt-6 pt-4 border-t border-borderSubtle">
        <p className="text-xs text-ai-ink-3 text-center">
          登录即表示您同意我们的服务条款和隐私政策
        </p>
      </div>
    </Modal>
  );
}

export default LoginModal;
