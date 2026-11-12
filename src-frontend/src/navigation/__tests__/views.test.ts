import { describe, it, expect } from 'vitest';
import { FULL_HEIGHT_VIEWS, IMPACT_LABELS, NAV_GROUPS, NAV_VIEW_IDS } from '../views';

describe('navigation/views 单一视图清单', () => {
  it('所有导航项 id 唯一且六组齐全', () => {
    const ids = NAV_GROUPS.flatMap(group => group.items.map(item => item.id));
    expect(NAV_GROUPS).toHaveLength(6);
    expect(new Set(ids).size).toBe(ids.length);
    expect(NAV_GROUPS.map(g => g.id)).toEqual([
      'create',
      'tasks',
      'assets',
      'ops',
      'analysis',
      'system',
    ]);
  });

  it('布局集合从同一份 NAV_GROUPS 派生', () => {
    const expected = [
      'agency-studio',
      'book-deconstruction',
      'corpus',
      'dashboard',
      'guidebook-distillation',
      'knowledge-graph',
      'logs',
      'scenes',
      'settings',
      'story-system',
      'tracing',
    ];
    expect([...FULL_HEIGHT_VIEWS].sort()).toEqual(expected);
  });

  it('废弃别名 mcp/writing-stats/creation-wizard 不进入导航清单', () => {
    expect(NAV_VIEW_IDS.has('mcp')).toBe(false);
    expect(NAV_VIEW_IDS.has('writing-stats')).toBe(false);
    expect(NAV_VIEW_IDS.has('creation-wizard')).toBe(false);
    expect(IMPACT_LABELS.hot).toContain('默认生成');
  });
});
