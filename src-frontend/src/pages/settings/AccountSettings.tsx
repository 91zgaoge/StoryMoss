import { useState, useEffect } from 'react';
import { User, Shield, Link2 } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { useAuthStore } from '@/stores/useAuthStore';
import { useAppStore } from '@/stores/appStore';
import { createLogger } from '@/utils/logger';

const accountLogger = createLogger('ui:AccountSettings');

// ==================== AccountSettings (v4.5.0) ====================

export function AccountSettings() {
  const { user, isLoggedIn, logout } = useAuthStore();
  const [authConfig, setAuthConfig] = useState<{
    google_enabled: boolean;
    github_enabled: boolean;
    wechat_enabled: boolean;
    qq_enabled: boolean;
  } | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    loadAuthConfig();
  }, []);

  const loadAuthConfig = async () => {
    try {
      const config = await import('@/services/auth').then(m => m.getAuthConfig());
      setAuthConfig(config);
    } catch (e) {
      accountLogger.error('Failed to load auth config', { error: e });
    }
  };

  const handleLogout = async () => {
    setIsLoading(true);
    try {
      await logout();
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* 登录状态卡片 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-4">
            <div className="w-14 h-14 rounded-full bg-cinema-800 flex items-center justify-center">
              {user?.avatar_url ? (
                <img src={user.avatar_url} alt="" className="w-14 h-14 rounded-full object-cover" />
              ) : (
                <User className="w-7 h-7 text-gray-400" />
              )}
            </div>
            <div className="flex-1">
              {isLoggedIn && user ? (
                <>
                  <h3 className="text-lg font-medium text-white">
                    {user.display_name || '已登录用户'}
                  </h3>
                  <p className="text-sm text-gray-400">{user.email || ''}</p>
                  <p className="text-xs text-green-500 mt-1 flex items-center gap-1">
                    <Shield className="w-3 h-3" />
                    已登录
                  </p>
                </>
              ) : (
                <>
                  <h3 className="text-lg font-medium text-white">未登录</h3>
                  <p className="text-sm text-gray-400">登录后可使用云同步等跨设备功能</p>
                </>
              )}
            </div>
            {isLoggedIn ? (
              <button
                onClick={handleLogout}
                disabled={isLoading}
                className="px-4 py-2 bg-red-500/10 text-red-400 rounded-lg hover:bg-red-500/20 transition-colors text-sm disabled:opacity-50"
              >
                {isLoading ? '退出中...' : '退出登录'}
              </button>
            ) : (
              <button
                onClick={() => useAppStore.getState().setLoginModalOpen(true)}
                className="px-4 py-2 bg-cinema-gold text-cinema-900 rounded-lg hover:bg-cinema-gold-light transition-colors text-sm font-medium"
              >
                登录
              </button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* OAuth 配置状态 */}
      <Card>
        <CardContent className="p-6">
          <h3 className="text-lg font-medium text-white mb-4 flex items-center gap-2">
            <Link2 className="w-5 h-5 text-cinema-gold" />
            OAuth 登录选项
          </h3>
          <div className="space-y-3">
            <ProviderStatus
              name="Google"
              enabled={authConfig?.google_enabled || false}
              icon={<span className="text-blue-400 font-medium text-sm">G</span>}
            />
            <ProviderStatus
              name="GitHub"
              enabled={authConfig?.github_enabled || false}
              icon={<span className="text-white font-medium text-sm">H</span>}
            />
            <ProviderStatus
              name="微信"
              enabled={authConfig?.wechat_enabled || false}
              icon={<span className="text-green-400 font-medium text-sm">W</span>}
            />
            <ProviderStatus
              name="QQ"
              enabled={authConfig?.qq_enabled || false}
              icon={<span className="text-blue-300 font-medium text-sm">Q</span>}
            />
          </div>
          <p className="text-xs text-gray-500 mt-4">
            在配置文件中设置 OAuth 客户端 ID 后，对应登录选项将自动启用。 微信/QQ
            登录需要在中国内地开放平台注册应用。
          </p>
        </CardContent>
      </Card>
    </div>
  );
}

function ProviderStatus({
  name,
  enabled,
  icon,
}: {
  name: string;
  enabled: boolean;
  icon: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between py-2 px-3 rounded-lg bg-cinema-800/30">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-cinema-800 flex items-center justify-center">
          {icon}
        </div>
        <span className="text-sm text-gray-300">{name}</span>
      </div>
      <span
        className={`text-xs px-2 py-0.5 rounded-full ${
          enabled ? 'bg-green-500/10 text-green-400' : 'bg-gray-700 text-gray-500'
        }`}
      >
        {enabled ? '已启用' : '未配置'}
      </span>
    </div>
  );
}
