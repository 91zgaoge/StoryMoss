import React from 'react';
import { cn } from '@/utils/cn';
import { AlertCircle, CheckCircle2, HelpCircle, MinusCircle } from 'lucide-react';

interface ConfidenceIndicatorProps {
  score?: number;
  size?: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
  variant?: 'circular' | 'bar';
  className?: string;
}

function getConfidenceColor(score?: number): string {
  if (score === undefined) return '#9ca3af'; // gray-400
  if (score >= 0.8) return '#10b981'; // emerald-500
  if (score >= 0.6) return '#3b82f6'; // blue-500
  if (score >= 0.4) return '#f59e0b'; // amber-500
  if (score >= 0.2) return '#ef4444'; // red-500
  return '#6b7280'; // gray-500
}

function getConfidenceLabel(score?: number): string {
  if (score === undefined) return '未知';
  if (score >= 0.8) return '高';
  if (score >= 0.6) return '较高';
  if (score >= 0.4) return '中等';
  if (score >= 0.2) return '低';
  return '极低';
}

function getConfidenceBgColor(score?: number): string {
  if (score === undefined) return 'bg-gray-500/20';
  if (score >= 0.8) return 'bg-emerald-500/20';
  if (score >= 0.6) return 'bg-blue-500/20';
  if (score >= 0.4) return 'bg-amber-500/20';
  if (score >= 0.2) return 'bg-red-500/20';
  return 'bg-gray-500/20';
}

function getConfidenceTextColor(score?: number): string {
  if (score === undefined) return 'text-gray-400';
  if (score >= 0.8) return 'text-emerald-400';
  if (score >= 0.6) return 'text-blue-400';
  if (score >= 0.4) return 'text-amber-400';
  if (score >= 0.2) return 'text-red-400';
  return 'text-gray-400';
}

function getConfidenceIcon(score?: number) {
  if (score === undefined) return HelpCircle;
  if (score >= 0.8) return CheckCircle2;
  if (score >= 0.4) return MinusCircle;
  return AlertCircle;
}

// Circular Progress Component
function CircularProgress({
  score,
  size,
  showLabel,
}: {
  score?: number;
  size: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
}) {
  const color = getConfidenceColor(score);
  const percentage = score !== undefined ? Math.round(score * 100) : 0;

  const dimensions = {
    sm: { width: 32, strokeWidth: 3, fontSize: 10 },
    md: { width: 48, strokeWidth: 4, fontSize: 12 },
    lg: { width: 64, strokeWidth: 5, fontSize: 14 },
  };

  const { width, strokeWidth, fontSize } = dimensions[size];
  const radius = (width - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - (percentage / 100) * circumference;

  const Icon = getConfidenceIcon(score);

  if (score === undefined) {
    return (
      <div
        className="flex items-center justify-center rounded-full bg-gray-500/20"
        style={{ width, height: width }}
      >
        <Icon className="text-gray-400" style={{ width: width * 0.5, height: width * 0.5 }} />
      </div>
    );
  }

  return (
    <div className="relative inline-flex items-center justify-center">
      <svg width={width} height={width} className="-rotate-90">
        {/* Background circle */}
        <circle
          cx={width / 2}
          cy={width / 2}
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          className="text-cinema-700"
        />
        {/* Progress circle */}
        <circle
          cx={width / 2}
          cy={width / 2}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          className="transition-all duration-500 ease-out"
        />
      </svg>

      {/* Center content */}
      <div className="absolute inset-0 flex items-center justify-center">
        {size === 'lg' && showLabel ? (
          <span className="font-semibold" style={{ fontSize, color }}>
            {percentage}%
          </span>
        ) : (
          <Icon className="text-gray-400" style={{ width: width * 0.4, height: width * 0.4 }} />
        )}
      </div>
    </div>
  );
}

// Bar Progress Component
function BarProgress({
  score,
  size,
  showLabel,
}: {
  score?: number;
  size: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
}) {
  const percentage = score !== undefined ? Math.round(score * 100) : 0;
  const color = getConfidenceColor(score);
  const bgColorClass = getConfidenceBgColor(score);
  const textColorClass = getConfidenceTextColor(score);
  const Icon = getConfidenceIcon(score);

  const heights = {
    sm: 'h-1.5',
    md: 'h-2',
    lg: 'h-3',
  };

  return (
    <div className="flex items-center gap-2 flex-1">
      <div className={cn('flex-1 bg-cinema-700 rounded-full overflow-hidden', heights[size])}>
        <div
          className="h-full rounded-full transition-all duration-500 ease-out"
          style={{
            width: `${percentage}%`,
            backgroundColor: color,
          }}
        />
      </div>

      {showLabel && (
        <div className={cn('flex items-center gap-1 shrink-0', textColorClass)}>
          <Icon className={cn(size === 'sm' ? 'w-3 h-3' : size === 'md' ? 'w-4 h-4' : 'w-5 h-5')} />
          <span
            className={cn(
              'font-medium whitespace-nowrap',
              size === 'sm' ? 'text-xs' : size === 'md' ? 'text-sm' : 'text-base'
            )}
          >
            {getConfidenceLabel(score)}
          </span>
          {size === 'lg' && <span className="text-gray-500 text-sm">({percentage}%)</span>}
        </div>
      )}
    </div>
  );
}

export function ConfidenceIndicator({
  score,
  size = 'md',
  showLabel = false,
  variant = 'bar',
  className,
}: ConfidenceIndicatorProps) {
  const textColorClass = getConfidenceTextColor(score);
  const bgColorClass = getConfidenceBgColor(score);
  const Icon = getConfidenceIcon(score);

  // Compact badge style (when not showing label and using bar variant)
  if (!showLabel && variant === 'bar') {
    return (
      <div
        className={cn(
          'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium',
          bgColorClass,
          textColorClass,
          className
        )}
        title={score !== undefined ? `置信度: ${(score * 100).toFixed(0)}%` : '未知置信度'}
      >
        <Icon className="w-3 h-3" />
        <span>置信度</span>
        {score !== undefined && <span>{(score * 100).toFixed(0)}%</span>}
      </div>
    );
  }

  return (
    <div
      className={cn(
        'inline-flex items-center gap-2',
        variant === 'circular' && size === 'lg' && 'flex-col gap-1',
        className
      )}
      title={score !== undefined ? `置信度: ${(score * 100).toFixed(0)}%` : '未知置信度'}
    >
      {variant === 'circular' ? (
        <CircularProgress score={score} size={size} showLabel={showLabel} />
      ) : (
        <BarProgress score={score} size={size} showLabel={showLabel} />
      )}

      {showLabel && variant === 'circular' && size !== 'lg' && (
        <span className={cn('text-sm font-medium', textColorClass)}>
          {getConfidenceLabel(score)}
        </span>
      )}
    </div>
  );
}

// Compact version for table/list displays
export function ConfidenceBadge({ score, className }: { score?: number; className?: string }) {
  const color = getConfidenceColor(score);
  const label = getConfidenceLabel(score);
  const Icon = getConfidenceIcon(score);

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full font-medium',
        className
      )}
      style={{
        backgroundColor: `${color}20`,
        color,
      }}
    >
      <Icon className="w-3 h-3" />
      {label}
    </span>
  );
}

export default ConfidenceIndicator;
