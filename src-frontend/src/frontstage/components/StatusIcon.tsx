import React from 'react';
import {
  Brain,
  ClipboardList,
  Cog,
  CheckCircle,
  Check,
  Send,
  Plug,
  Zap,
  XCircle,
  Timer,
  PenTool,
  Hourglass,
  Ban,
  AlertTriangle,
  BookOpen,
  Sparkles,
  Loader2,
  Settings2,
} from 'lucide-react';

interface StatusIconProps {
  text: string;
}

export const StatusIcon: React.FC<StatusIconProps> = ({ text }) => {
  // 移除旧emoji（如果有）
  const cleanText = text
    .replace(
      /[\u{1F300}-\u{1F9FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]|[\u{23F0}-\u{23FF}]|[\u{200D}]/gu,
      ''
    )
    .trim();

  let Icon = Loader2;
  let iconClass = 'w-3.5 h-3.5';

  if (
    cleanText.includes('分析') ||
    cleanText.includes('Thinking') ||
    cleanText.includes('构建') ||
    cleanText.includes('加载') ||
    cleanText.includes('读取') ||
    cleanText.includes('渲染') ||
    cleanText.includes('准备') ||
    cleanText.includes('查询') ||
    cleanText.includes('计算')
  ) {
    Icon = Brain;
  } else if (
    cleanText.includes('注入') ||
    cleanText.includes('组装') ||
    cleanText.includes('拼接')
  ) {
    Icon = Cog;
  } else if (
    cleanText.includes('计划') ||
    cleanText.includes('规划') ||
    cleanText.includes('plan')
  ) {
    Icon = ClipboardList;
  } else if (
    cleanText.includes('执行') ||
    cleanText.includes('running') ||
    cleanText.includes('步骤')
  ) {
    Icon = Cog;
  } else if (
    cleanText.includes('完成') ||
    cleanText.includes('completed') ||
    cleanText.includes('通过')
  ) {
    Icon = CheckCircle;
    iconClass = 'w-3.5 h-3.5 text-green-500';
  } else if (cleanText.includes('质检') || cleanText.includes('检查')) {
    Icon = Check;
  } else if (cleanText.includes('大纲')) {
    Icon = BookOpen;
  } else if (cleanText.includes('连接') || cleanText.includes('connecting')) {
    Icon = Plug;
  } else if (
    cleanText.includes('发送') ||
    cleanText.includes('sent') ||
    cleanText.includes('请求')
  ) {
    Icon = Send;
  } else if (
    cleanText.includes('生成中') ||
    cleanText.includes('generating') ||
    cleanText.includes('生成内容')
  ) {
    Icon = Zap;
  } else if (
    cleanText.includes('错误') ||
    cleanText.includes('失败') ||
    cleanText.includes('error') ||
    cleanText.includes('超时')
  ) {
    Icon = XCircle;
    iconClass = 'w-3.5 h-3.5 text-red-500';
  } else if (cleanText.includes('等待') || cleanText.includes('时间')) {
    Icon = Timer;
  } else if (
    cleanText.includes('写作') ||
    cleanText.includes('Writer') ||
    cleanText.includes('writer')
  ) {
    Icon = PenTool;
  } else if (cleanText.includes('取消')) {
    Icon = Ban;
  } else if (
    cleanText.includes('警告') ||
    cleanText.includes('空内容') ||
    cleanText.includes('注意')
  ) {
    Icon = AlertTriangle;
  } else if (
    cleanText.includes('构思') ||
    cleanText.includes('bootstrap') ||
    cleanText.includes('新建') ||
    cleanText.includes('创建')
  ) {
    Icon = Sparkles;
  } else if (cleanText.includes('续写') || cleanText.includes('撰写')) {
    Icon = PenTool;
  } else if (cleanText.includes('学习') || cleanText.includes('自适应')) {
    Icon = Brain;
  } else if (cleanText.includes('设置') || cleanText.includes('配置')) {
    Icon = Settings2;
  }

  const isLoading =
    !cleanText.includes('完成') && !cleanText.includes('错误') && !cleanText.includes('失败');

  return (
    <span className="inline-flex items-center gap-1.5">
      <Icon className={`${iconClass} ${isLoading ? 'animate-spin' : ''}`} />
      <span>{cleanText}</span>
    </span>
  );
};

export default React.memo(StatusIcon);
