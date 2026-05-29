/**
 * useAudit - 场景审计 Hook
 */

import { useQuery } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';

export interface AuditIssue {
  severity: 'blocking' | 'warning' | 'info';
  message: string;
  suggestion?: string;
}

export interface AuditDimension {
  name: string;
  score: number;
  issues: AuditIssue[];
}

export interface AuditReport {
  scene_id: string;
  overall_score: number;
  dimensions: AuditDimension[];
  has_blocking_issues: boolean;
  audit_type: string;
  content_word_count: number;
}

const AUDIT_KEY = 'audit';

export function useAuditScene(
  sceneId: string | null,
  auditType: string = 'light',
  enabled: boolean = false
) {
  return useQuery<AuditReport>({
    queryKey: [AUDIT_KEY, sceneId, auditType],
    queryFn: async () => {
      if (!sceneId) throw new Error('Scene ID is required');
      return loggedInvoke<AuditReport>('audit_scene', { scene_id: sceneId, audit_type: auditType });
    },
    enabled: !!sceneId && enabled,
    staleTime: 1000 * 60 * 5, // 5 minutes
  });
}
