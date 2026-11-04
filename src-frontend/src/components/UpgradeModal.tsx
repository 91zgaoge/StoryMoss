/**
 * 共享升级引导弹窗（幕后 / Tailwind cinema 主题）。
 *
 * 幕前另有 frontstage/components/UpgradePanel（frontstage.css 样式）。
 * 本组件供幕后各 Pro 功能入口统一使用：
 * - 功能入口处的「Pro」徽标 / 禁用态点击升级；
 * - 后端返回 SUBSCRIPTION_REQUIRED 错误时弹出（配合
 *   utils/errorHandler 的 isSubscriptionRequired 判定）。
 */

import React, { useState } from 'react';
import { Sparkles, Zap, BookOpen, Wand2, X, Loader2 } from 'lucide-react';
import { devUpgradeSubscription } from '@/services/tauri';
import { useAuthStore } from '@/stores/useAuthStore';
import { createLogger } from '@/utils/logger';

const upgradeLogger = createLogger('ui:UpgradeModal');

export interface UpgradeModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** 触发来源功能名（如「指导书提炼」），用于标题文案 */
  featureName?: string;
  onUpgraded?: () => void;
}

const proFeatures = [
  { icon: BookOpen, title: '指导书提炼', desc: '上传创作指导书，自动提炼为可调用的创作方法论' },
  { icon: Zap, title: '自动续写 / 智能修改', desc: 'AI 持续创作与全文级润色，长篇写作不断档' },
  { icon: Wand2, title: '拆书 / Pipeline', desc: '参考书解构与 Refine / Review / Finalize 全流程' },
];

export const UpgradeModal: React.FC<UpgradeModalProps> = ({
  isOpen,
  onClose,
  featureName,
  onUpgraded,
}) => {
  const [isUpgrading, setIsUpgrading] = useState(false);
  const [upgradeError, setUpgradeError] = useState<string | null>(null);
  const [invite, setInvite] = useState('');
  const { isLoggedIn, isWaitingForOAuth, login } = useAuthStore();

  if (!isOpen) return null;

  const handleUpgrade = async () => {
    if (isUpgrading) return;
    setIsUpgrading(true);
    setUpgradeError(null);
    try {
      await devUpgradeSubscription('pro');
      onUpgraded?.();
      onClose();
    } catch (err) {
      upgradeLogger.error('Upgrade failed', { error: err });
      setUpgradeError('升级失败，请稍后重试');
    } finally {
      setIsUpgrading(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-md mx-4 rounded-xl border border-cinema-700 bg-cinema-900 p-6 shadow-2xl"
        onClick={e => e.stopPropagation()}
      >
        <button
          className="absolute top-4 right-4 text-gray-500 hover:text-white transition-colors"
          onClick={onClose}
          aria-label="关闭"
        >
          <X size={18} />
        </button>

        <div className="text-center mb-5">
          <div className="w-14 h-14 mx-auto rounded-full bg-cinema-gold/15 flex items-center justify-center mb-3">
            <Sparkles size={28} className="text-cinema-gold" />
          </div>
          <h2 className="text-lg font-bold text-white">
            {featureName ? `「${featureName}」需要 Pro` : '升级专业版'}
          </h2>
          <p className="text-sm text-gray-400 mt-1">升级专业版，释放 AI 创作全部潜能</p>
        </div>

        <div className="space-y-3 mb-5">
          {proFeatures.map((f, i) => (
            <div key={i} className="flex items-start gap-3">
              <div className="w-8 h-8 rounded-lg bg-cinema-800 flex items-center justify-center shrink-0">
                <f.icon size={16} className="text-cinema-gold" />
              </div>
              <div>
                <div className="text-sm font-medium text-gray-200">{f.title}</div>
                <div className="text-xs text-gray-500">{f.desc}</div>
              </div>
            </div>
          ))}
        </div>

        {!isLoggedIn && (
          <div className="mb-5 rounded-lg border border-cinema-700 bg-cinema-800/50 p-3">
            <p className="text-xs text-gray-400 text-center mb-2">
              登录后升级，Pro 跟随账号（换设备不丢）
            </p>
            <input
              type="text"
              value={invite}
              onChange={e => setInvite(e.target.value)}
              placeholder="邀请码（新用户注册必填）"
              className="w-full mb-2 px-3 py-2 text-sm rounded-lg bg-cinema-800 border border-cinema-700 text-gray-200 placeholder:text-gray-500 focus:outline-none focus:border-cinema-600"
            />
            <div className="flex gap-2">
              <button
                onClick={() => void login('google', invite.trim() || undefined)}
                className="flex-1 py-2 rounded-lg bg-cinema-700 text-sm text-gray-200 hover:bg-cinema-600"
              >
                Google 登录
              </button>
              <button
                onClick={() => void login('github', invite.trim() || undefined)}
                className="flex-1 py-2 rounded-lg bg-cinema-700 text-sm text-gray-200 hover:bg-cinema-600"
              >
                GitHub 登录
              </button>
            </div>
          </div>
        )}

        <div className="text-center mb-5">
          <span className="text-3xl font-bold text-cinema-gold">¥19</span>
          <span className="text-sm text-gray-400">/月</span>
          <p className="text-xs text-gray-500 mt-1">限时早鸟价 · 随时可退订</p>
        </div>

        {upgradeError && (
          <div className="mb-4 text-center text-sm text-red-400">{upgradeError}</div>
        )}

        <div className="flex flex-col gap-2">
          <button
            className="flex items-center justify-center gap-2 w-full py-2.5 rounded-lg bg-cinema-gold text-cinema-950 font-medium hover:bg-cinema-gold/90 transition-colors disabled:opacity-50"
            onClick={handleUpgrade}
            disabled={isUpgrading || isWaitingForOAuth}
          >
            {isUpgrading ? <Loader2 size={16} className="animate-spin" /> : <Sparkles size={16} />}
            {isUpgrading ? '升级中...' : isLoggedIn ? '立即升级' : '暂不登录，仅本设备升级'}
          </button>
          <button
            className="w-full py-2 rounded-lg text-sm text-gray-400 hover:text-white transition-colors"
            onClick={onClose}
          >
            继续使用免费版
          </button>
        </div>
      </div>
    </div>
  );
};

export default UpgradeModal;
