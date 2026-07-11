#!/usr/bin/env python3
"""
精确按行号拆分 api/index.ts
"""
from pathlib import Path

SRC = Path('/Users/yuzaimu/projects/StoryMoss/src-frontend/src/services/api/index.ts')
API_DIR = SRC.parent

# 读取原始文件
lines = SRC.read_text(encoding='utf-8').splitlines(keepends=True)

# 辅助：获取行区间内容（1-based, 包含 start 和 end）
def slice_lines(start, end):
    return lines[start-1:end]

# 辅助：拼接内容为字符串
def join_slice(start, end):
    return ''.join(slice_lines(start, end))

# 辅助：写入文件
def write_file(name, header_lines, content):
    out = API_DIR / f'{name}.ts'
    full = ''.join(header_lines) + '\n' + content
    # 清理多余空行
    while '\n\n\n' in full:
        full = full.replace('\n\n\n', '\n\n')
    out.write_text(full, encoding='utf-8')
    print(f'Written {name}.ts ({len(full)} chars)')

# core.ts: 原始文件的 lines 1-45
CORE = join_slice(1, 45)

# 各子文件内容（按行号精确提取）
# stories: Stories(98-111) + Characters(113-134) + Chapters(136-153) + Scenes(664-695) + WorldBuilding(697-719) + WritingStyle(721-742)
STORIES = join_slice(98, 111) + join_slice(113, 134) + join_slice(136, 153) + join_slice(664, 695) + join_slice(697, 719) + join_slice(721, 742)

# skills: Skills(155-178) + MCP(180-206) + CreationWorkflow(208-239) + StyleBlend(241-273)
SKILLS = join_slice(155, 178) + join_slice(180, 206) + join_slice(208, 239) + join_slice(241, 273)

# settings: VectorSearchLanceDB(275-281) + Settings(283-316)
SETTINGS = join_slice(275, 281) + join_slice(283, 316)

# intent: IntentEngine(318-323) + SmartExecute(325-376) + FeedbackRecording(378-396)
INTENT = join_slice(318, 323) + join_slice(325, 376) + join_slice(378, 396)

# knowledge: KnowledgeGraph(398-422) + VectorSearch(524-529) + MemoryCompressor(531-539) + KnowledgeDistillation(541-552)
KNOWLEDGE = join_slice(398, 422) + join_slice(524, 529) + join_slice(531, 539) + join_slice(541, 552)

# wizard: NovelCreationWizard(424-459)
WIZARD = join_slice(424, 459)

# annotations: SceneAnnotations(461-485) + TextInlineAnnotations(487-514) + CommentatorAgent(516-522)
ANNOTATIONS = join_slice(461, 485) + join_slice(487, 514) + join_slice(516, 522)

# memory: MemorySystem(890-974)
MEMORY = join_slice(890, 974)

# pipeline: PipelineV7(1147-1239)
PIPELINE = join_slice(1147, 1239)

# quality: ReadingPower(976-1004) + StoryAudit(1006-1032) + OverrideContract(1034-1047) + ReadingPowerFuncs(1049-1074) + GenreProfiles(1076-1093) + AntiAiReview(1095-1144)
QUALITY = join_slice(976, 1004) + join_slice(1006, 1032) + join_slice(1034, 1047) + join_slice(1049, 1074) + join_slice(1076, 1093) + join_slice(1095, 1144)

# genesis: GenesisEngine(626-661) + EntitiesRelations(744-773) + IngestJobs(783-799) + GenesisPipeline(1241-1264) + StyleDNA_W3F2(1266-1283) + LitSeg(1285-1340)
GENESIS = join_slice(626, 661) + join_slice(744, 773) + join_slice(783, 799) + join_slice(1241, 1264) + join_slice(1266, 1283) + join_slice(1285, 1340)

# storySystem: StorySystem(801-888)
STORY_SYSTEM = join_slice(801, 888)

# stream: LLMStream(554-565) + InputHint(621-623)
STREAM = join_slice(554, 565) + join_slice(621, 623)

# subscription: Subscription(567-583)
SUBSCRIPTION = join_slice(567, 583)

# index.ts 保留内容: HealthCheck(98-100) + 文思泉涌(585-615) + WindowComm(617-619) + saveGenreProfile/deleteGenreProfile(774-782) + getFeatureUsageStats/logFeatureUsage(788-799)
INDEX_KEEP = join_slice(98, 100) + join_slice(585, 615) + join_slice(617, 619) + join_slice(774, 782) + join_slice(788, 799)

# 写入 core.ts
(API_DIR / 'core.ts').write_text(CORE, encoding='utf-8')
print(f'Written core.ts ({len(CORE)} chars)')

# 写入各子文件，带上各自的类型导入
write_file('stories', [
    "import { loggedInvoke } from './core';",
    "import type { Story, Character, Chapter, CreateStoryRequest, CreateCharacterRequest, UpdateChapterRequest } from '@/types/index';",
    "import type { SceneAnnotation, TextAnnotation, ParagraphCommentary, SceneProposal, WorldBuildingOption, CharacterProfileOption, WritingStyleOption } from '@/types/v3';",
], STORIES)

write_file('skills', [
    "import { loggedInvoke } from './core';",
    "import type { Skill, McpServer, McpTool } from '@/types/index';",
    "import type { SceneAnnotation, TextAnnotation, ParagraphCommentary } from '@/types/v3';",
], SKILLS)

write_file('settings', [
    "import { loggedInvoke } from './core';",
    "import type { LlmConfig, VectorSearchRequest, SimilarityResult } from '@/types/index';",
    "import type { AppSettings } from '@/types/llm';",
], SETTINGS)

write_file('intent', [
    "import { loggedInvoke } from './core';",
    "import type { Intent, IntentParseRequest, IntentExecutionResult } from '@/types/index';",
], INTENT)

write_file('annotations', [
    "import { loggedInvoke } from './core';",
    "import type { SceneAnnotation, TextAnnotation, ParagraphCommentary } from '@/types/v3';",
], ANNOTATIONS)

write_file('knowledge', [
    "import { loggedInvoke } from './core';",
    "import type { VectorSearchRequest, SimilarityResult } from '@/types/index';",
    "import type { StoryGraph, Entity, Relation, RetentionReport, ArchiveResult, AgentResult, StorySummary, VectorSearchResult } from '@/types/v3';",
], KNOWLEDGE)

write_file('wizard', [
    "import { loggedInvoke } from './core';",
    "import type { WorldBuildingOption, CharacterProfileOption, WritingStyleOption, SceneProposal } from '@/types/v3';",
    "import type { WizardCreationResult } from '@/types/index';",
], WIZARD)

write_file('memory', [
    "import { loggedInvoke } from './core';",
], MEMORY)

write_file('pipeline', [
    "import { loggedInvoke } from './core';",
    "import type { Draft, Revision, PipelineReview, PostProcessRun, PostProcessStep, LlmCall, CharacterState, RefineResult, ReviewResult, PipelineResult } from '@/types/pipeline';",
], PIPELINE)

write_file('quality', [
    "import { loggedInvoke } from './core';",
], QUALITY)

write_file('genesis', [
    "import { loggedInvoke } from './core';",
    "import type { StoryOutline, CharacterRelationship } from '@/types/index';",
], GENESIS)

write_file('storySystem', [
    "import { loggedInvoke } from './core';",
], STORY_SYSTEM)

write_file('stream', [
    "import { loggedInvoke } from './core';",
], STREAM)

write_file('subscription', [
    "import { loggedInvoke } from './core';",
], SUBSCRIPTION)

# 写入 api/index.ts（barrel + 保留内容）
index_content = INDEX_KEEP
index_exports = '''\nexport * from './stories';
export * from './skills';
export * from './settings';
export * from './intent';
export * from './annotations';
export * from './knowledge';
export * from './wizard';
export * from './memory';
export * from './pipeline';
export * from './quality';
export * from './genesis';
export * from './storySystem';
export * from './stream';
export * from './subscription';
'''

index_full = "import { loggedInvoke } from './core';\n\n" + index_content + index_exports
while '\n\n\n' in index_full:
    index_full = index_full.replace('\n\n\n', '\n\n')

SRC.write_text(index_full, encoding='utf-8')
print(f'Written index.ts ({len(index_full)} chars)')

print('\nDone!')
