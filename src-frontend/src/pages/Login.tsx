/**
 * Login Page — 登录弹窗/页面
 * v4.5.0
 */

import { useState, useEffect } from 'react';
import { X, Chrome, Github, MessageCircle } from 'lucide-react';
import { useAuthStore } from '@/stores/useAuthStore';
import { Card } from '@/components/ui/Card';
import toast from 'react-hot-toast';

interface LoginModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function LoginModal({ isOpen, onClose }: LoginModalProps) {
  const { authConfig, login, isLoading } = useAuthStore();
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setIsVisible(true);
    } else {
      const timer = setTimeout(() => setIsVisible(false), 200);
      return () => clearTimeout(timer);
    }
  }, [isOpen]);

  useEffect(() => {
    if (isOpen) {
      useAuthStore.getState().loadAuthConfig();
    }
  }, [isOpen]);

  if (!isVisible) return null;

  const handleLogin = async (provider: string) => {
    try {
      await login(provider);
      toast.success('请在浏览器中完成授权');
    } catch (error) {
      toast.error(`登录失败: ${error}`);
    }
  };

  return (
    <div
      className={`fixed inset-0 z-50 flex items-center justify-center transition-opacity duration-200 ${
        isOpen ? 'opacity-100' : 'opacity-0'
      }`}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" onClick={onClose} />

      {/* Modal */}
      <Card className="relative w-full max-w-md mx-4 p-6 transform transition-all duration-200">
        {/* Close button */}
        <button
          onClick={onClose}
          className="absolute right-4 top-4 p-1 text-stone-400 hover:text-stone-600 rounded-md hover:bg-stone-100 transition-colors"
        >
          <X className="w-5 h-5" />
        </button>

        {/* Header */}
        <div className="text-center mb-6">
          <h2 className="text-xl font-semibold text-stone-800">登录 StoryForge</h2>
          <p className="text-sm text-stone-500 mt-1">登录后可解锁云同步等跨设备功能</p>
        </div>

        {/* OAuth Buttons */}
        <div className="space-y-3">
          {authConfig?.google_enabled && (
            <button
              onClick={() => handleLogin('google')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-white border border-stone-200 rounded-lg text-stone-700 hover:bg-stone-50 hover:border-stone-300 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Chrome className="w-5 h-5 text-blue-500" />
              <span className="text-sm font-medium">使用 Google 登录</span>
            </button>
          )}

          {authConfig?.github_enabled && (
            <button
              onClick={() => handleLogin('github')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-white border border-stone-200 rounded-lg text-stone-700 hover:bg-stone-50 hover:border-stone-300 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Github className="w-5 h-5 text-stone-800" />
              <span className="text-sm font-medium">使用 GitHub 登录</span>
            </button>
          )}

          {/* WeChat — 预留，未启用时显示提示 */}
          {authConfig?.wechat_enabled && (
            <button
              onClick={() => handleLogin('wechat')}
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-white border border-stone-200 rounded-lg text-stone-700 hover:bg-stone-50 hover:border-stone-300 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
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
              className="w-full flex items-center justify-center gap-3 px-4 py-2.5 bg-white border border-stone-200 rounded-lg text-stone-700 hover:bg-stone-50 hover:border-stone-300 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
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
                <p className="text-sm text-stone-500">尚未配置 OAuth 登录选项</p>
                <p className="text-xs text-stone-400 mt-1">请在设置中配置 OAuth 客户端信息</p>
              </div>
            )}
        </div>

        {/* Footer */}
        <div className="mt-6 pt-4 border-t border-stone-100">
          <p className="text-xs text-stone-400 text-center">
            登录即表示您同意我们的服务条款和隐私政策
          </p>
        </div>
      </Card>
    </div>
  );
}

export default LoginModal;
