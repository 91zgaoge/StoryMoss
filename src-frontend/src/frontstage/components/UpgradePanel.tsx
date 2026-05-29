/**
 * 付费引导面板 — 转化漏斗核心 UI (V2)
 *
 * V2: 仅限制 auto_write / auto_revise，其余功能全部免费。
 * 引导文案聚焦于两个杀手级功能的价值。
 */

import React, { useState } from 'react';
import { Sparkles, Zap, BookOpen, Infinity, X, Loader2, PenTool, Wand2 } from 'lucide-react';
import { devUpgradeSubscription } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const upgradeLogger = createLogger('ui:frontstage:UpgradePanel');

interface UpgradePanelProps {
  isOpen: boolean;
  onClose: () => void;
  trigger?: string;
  onUpgraded?: () => void;
}

const features = [
  { icon: Zap, title: '🔥 自动续写', desc: '一口气写上 50 万字不停歇，AI 替你持续创作' },
  { icon: Wand2, title: '🎯 智能修改', desc: '基于故事设定全文润色，专业级修改建议' },
  { icon: Infinity, title: '💎 其余功能已免费', desc: '单次续写、排版、斜杠命令等全部免费开放' },
];

export const UpgradePanel: React.FC<UpgradePanelProps> = ({
  isOpen,
  onClose,
  trigger,
  onUpgraded,
}) => {
  const [isUpgrading, setIsUpgrading] = useState(false);

  if (!isOpen) return null;

  const handleUpgrade = async () => {
    if (isUpgrading) return;
    setIsUpgrading(true);
    try {
      await devUpgradeSubscription('pro');
      toast.success('🎉 升级成功！欢迎体验专业版功能');
      onUpgraded?.();
      onClose();
    } catch (err) {
      upgradeLogger.error('Upgrade failed', { error: err });
      toast.error('升级失败，请稍后重试');
    } finally {
      setIsUpgrading(false);
    }
  };

  return (
    <div className="upgrade-panel-overlay" onClick={onClose}>
      <div className="upgrade-panel" onClick={e => e.stopPropagation()}>
        <button className="upgrade-panel-close" onClick={onClose}>
          <X size={18} />
        </button>

        <div className="upgrade-panel-header">
          <div className="upgrade-panel-icon">
            <Sparkles size={32} />
          </div>
          <h2 className="upgrade-panel-title">解锁文思泉涌</h2>
          <p className="upgrade-panel-subtitle">{trigger || '升级专业版，释放 AI 创作全部潜能'}</p>
        </div>

        <div className="upgrade-panel-features">
          {features.map((f, i) => (
            <div key={i} className="upgrade-feature">
              <div className="upgrade-feature-icon">
                <f.icon size={18} />
              </div>
              <div className="upgrade-feature-text">
                <span className="upgrade-feature-title">{f.title}</span>
                <span className="upgrade-feature-desc">{f.desc}</span>
              </div>
            </div>
          ))}
        </div>

        <div className="upgrade-panel-pricing">
          <div className="upgrade-price">
            <span className="upgrade-price-amount">¥19</span>
            <span className="upgrade-price-unit">/月</span>
          </div>
          <p className="upgrade-price-note">限时早鸟价 · 随时可退订</p>
        </div>

        <div className="upgrade-panel-actions">
          <button className="upgrade-btn-primary" onClick={handleUpgrade} disabled={isUpgrading}>
            {isUpgrading ? <Loader2 size={16} className="spin" /> : <Sparkles size={16} />}
            {isUpgrading ? '升级中...' : '立即升级'}
          </button>
          <button className="upgrade-btn-secondary" onClick={onClose}>
            继续使用免费版
          </button>
        </div>

        <p className="upgrade-panel-footer">当前为开发测试模式，点击升级即可解锁全部功能</p>
      </div>
    </div>
  );
};

export default UpgradePanel;
