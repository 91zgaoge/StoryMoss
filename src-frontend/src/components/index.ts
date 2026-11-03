// Component exports
export { Button } from './ui/Button';
export { EmptyHint } from './ui/EmptyHint';
export { Card, CardContent } from './ui/Card';
export { ConnectionStatus } from './ConnectionStatus';
// v0.81.0 UI 基建（幕后重设计 P0，设计文档 §5）
export { Modal } from './ui/Modal';
export type { ModalProps } from './ui/Modal';
export { ConfirmDialog } from './ui/ConfirmDialog';
export type { ConfirmDialogProps } from './ui/ConfirmDialog';
export { PageHeader } from './ui/PageHeader';
export type { PageHeaderProps } from './ui/PageHeader';
export { Field } from './ui/Field';
export type { FieldProps } from './ui/Field';
export { Input, Select, Textarea, controlClass } from './ui/controls';
export { Table } from './ui/Table';
export type { TableColumn, TableProps } from './ui/Table';
export { EmptyState } from './ui/EmptyState';
export type { EmptyStateProps } from './ui/EmptyState';
export { Skeleton } from './ui/Skeleton';
export { Z_INDEX } from './ui/layers';
// P1 - AI Native Components（生成体验）
export { AiLoading } from './ui/ai/AiLoading';
export { AiThinking } from './ui/ai/AiThinking';
export { AiStreamingText } from './ui/ai/AiStreamingText';
export { AiPromptBar } from './ui/ai/AiPromptBar';
export { AiApprovalCard } from './ui/ai/AiApprovalCard';
export type { AiLoadingProps } from './ui/ai/AiLoading';
export type { AiThinkingProps, AiThinkingRow } from './ui/ai/AiThinking';
export type { AiStreamingTextProps } from './ui/ai/AiStreamingText';
export type {
  AiPromptBarProps,
  AiPromptSource,
  AiPromptCommand,
  AiPromptModel,
} from './ui/ai/AiPromptBar';
export type {
  AiApprovalCardProps,
  AiApprovalQuestion,
  AiApprovalOption,
} from './ui/ai/AiApprovalCard';
// P2 - AI Native Components（代理与任务）
export { AiContextCards } from './ui/ai/AiContextCards';
export { AiToolChips } from './ui/ai/AiToolChips';
export { AiRecommendationCard } from './ui/ai/AiRecommendationCard';
export { AiTaskRows } from './ui/ai/AiTaskRows';
export type {
  AiContextCardsProps,
  AiContextCardItem,
  AiContextCardSource,
} from './ui/ai/AiContextCards';
export type { AiToolChipsProps, AiToolChipItem } from './ui/ai/AiToolChips';
export type {
  AiRecommendationCardProps,
  AiRecommendationOption,
} from './ui/ai/AiRecommendationCard';
export type {
  AiTaskRowsProps,
  AiTaskRowItem,
  AiTaskRowDetail,
  AiTaskRowStatus,
} from './ui/ai/AiTaskRows';
// P3 - AI Native Components（数据展示）
export { AiSearchList } from './ui/ai/AiSearchList';
export { AiCodeBlock } from './ui/ai/AiCodeBlock';
export { AiDiffTable } from './ui/ai/AiDiffTable';
export { AiFilterTable, AiFilterChipsBar } from './ui/ai/AiFilterTable';
export { AiRecordsTable } from './ui/ai/AiRecordsTable';
export { AiInsightCards } from './ui/ai/AiInsightCards';
export type { AiSearchListProps } from './ui/ai/AiSearchList';
export type { AiCodeBlockProps } from './ui/ai/AiCodeBlock';
export type { AiDiffTableProps, AiDiffRow } from './ui/ai/AiDiffTable';
export type {
  AiFilterTableProps,
  AiFilterChipsBarProps,
  AiFilterChipItem,
  AiFilterColumn,
} from './ui/ai/AiFilterTable';
export type { AiRecordsTableProps, AiRecordsColumn, AiRecordsSort } from './ui/ai/AiRecordsTable';
export type { AiInsightCardsProps, AiInsightCardItem } from './ui/ai/AiInsightCards';
export { DataLoader } from './DataLoader';
export { MonacoEditor as Editor } from './Editor';
export { EditorSettings } from './EditorSettings';
export { ErrorBoundary } from './ErrorBoundary';
export { ExportDialog } from './ExportDialog';
export { FrontstageLauncher } from './FrontstageLauncher';
export { NovelCreationWizard } from './NovelCreationWizard';
export { SceneEditor } from './SceneEditor';
export { Sidebar } from './Sidebar';
export { StoryTimeline } from './StoryTimeline';
export { VectorSearch } from './VectorSearch';

// Phase 3.x - Version Management
export { VersionTimeline } from './VersionTimeline';
export { ConfidenceIndicator, ConfidenceBadge } from './ConfidenceIndicator';
export { DiffViewer } from './DiffViewer';
export { ExecutionPanel } from './ExecutionPanel';
