import React from 'react';
import {
  Brain,
  ClipboardList,
  Cog,
  CheckCircle,
  Send,
  Plug,
  Zap,
  XCircle,
  Timer,
  PenTool,
  Ban,
  AlertTriangle,
  BookOpen,
  Sparkles,
  Loader2,
  Settings2,
  FolderOpen,
  Search,
  Pencil,
  Upload,
  HardDrive,
  Network,
  Hourglass,
} from 'lucide-react';

interface StatusIconProps {
  text: string;
}

/** 剥离 emoji / 符号变体选择符，避免 WebView 缺字时留下 □□ */
function stripEmoji(text: string): string {
  return text
    .replace(
      /[\u{1F300}-\u{1FAFF}]|[\u{2600}-\u{27BF}]|[\u{2300}-\u{23FF}]|[\u{FE0E}\u{FE0F}\u{200D}]/gu,
      ''
    )
    .replace(/\s+/g, ' ')
    .trim();
}

export const StatusIcon: React.FC<StatusIconProps> = ({ text }) => {
  const cleanText = stripEmoji(text);

  let Icon = Loader2;
  let iconClass = 'w-3.5 h-3.5';

  if (
    cleanText.includes('准备上下文') ||
    cleanText.includes('加载故事') ||
    cleanText.includes('加载') ||
    cleanText.includes('读取') ||
    cleanText.includes('渲染') ||
    cleanText.includes('查询') ||
    cleanText.includes('计算')
  ) {
    Icon = FolderOpen;
  } else if (
    cleanText.includes('分析') ||
    cleanText.includes('Thinking') ||
    cleanText.includes('构建') ||
    cleanText.includes('准备')
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
  } else if (
    cleanText.includes('审校') ||
    cleanText.includes('质检') ||
    cleanText.includes('检查')
  ) {
    Icon = Search;
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
    cleanText.includes('候选') ||
    cleanText.includes('生成中') ||
    cleanText.includes('generating') ||
    cleanText.includes('生成内容')
  ) {
    Icon = Zap;
  } else if (cleanText.includes('最终输出')) {
    Icon = Upload;
  } else if (cleanText.includes('保存记忆') || cleanText.includes('记忆')) {
    Icon = HardDrive;
  } else if (
    cleanText.includes('错误') ||
    cleanText.includes('失败') ||
    cleanText.includes('error') ||
    cleanText.includes('超时')
  ) {
    Icon = XCircle;
    iconClass = 'w-3.5 h-3.5 text-red-500';
  } else if (
    cleanText.includes('等待') ||
    cleanText.includes('时间') ||
    cleanText.includes('后台正在完善')
  ) {
    Icon = cleanText.includes('后台') ? Hourglass : Timer;
  } else if (
    cleanText.includes('改写') ||
    cleanText.includes('润色') ||
    cleanText.includes('revise')
  ) {
    Icon = Pencil;
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
  } else if (cleanText.includes('图谱') || cleanText.includes('知识')) {
    Icon = Network;
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
