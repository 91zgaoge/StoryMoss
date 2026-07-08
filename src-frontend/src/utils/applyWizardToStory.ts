import { loggedInvoke } from '@/services/tauri';
import type { Story, Character } from '@/types/index';
import type {
  WorldBuildingOption,
  CharacterProfileOption,
  WritingStyleOption,
  SceneProposal,
  WorldBuilding,
  WritingStyle,
  Scene,
} from '@/types/v3';

export interface WizardData {
  worldBuilding: WorldBuildingOption;
  characters: CharacterProfileOption[];
  writingStyle: WritingStyleOption;
  firstScene: SceneProposal;
  genreInput: string;
  selectedStrategy?: {
    style_dna_ids?: string[];
    genre_profile_id?: string;
    methodology_id?: string;
  };
}

export interface WizardApplyResult {
  story: Story;
  characters: Character[];
  scene: Scene;
}

/**
 * 将 NovelCreationWizard 产出的资产应用到已有故事，避免重复创建故事。
 *
 * 注：当前为前端最小实现，覆盖世界观、角色、文风、首场景；
 * 知识图谱摄取与 create_story_with_wizard 的完整事务原子性不在此实现范围内。
 */
export async function applyWizardToStory(
  story: Story,
  data: WizardData
): Promise<WizardApplyResult> {
  // 1. 更新故事元数据（保留原标题，写入策略与类型）
  await loggedInvoke<void>('update_story', {
    id: story.id,
    genre: data.genreInput || story.genre,
    style_dna_id: data.selectedStrategy?.style_dna_ids?.[0] ?? story.style_dna_id,
    genre_profile_id: data.selectedStrategy?.genre_profile_id ?? story.genre_profile_id,
    methodology_id: data.selectedStrategy?.methodology_id ?? story.methodology_id,
  });

  // 2. 世界观：获取或创建后完整更新
  let worldBuilding = await loggedInvoke<WorldBuilding | null>('get_world_building', {
    story_id: story.id,
  });
  if (!worldBuilding) {
    worldBuilding = await loggedInvoke<WorldBuilding>('create_world_building', {
      story_id: story.id,
      concept: data.worldBuilding.concept,
    });
  }
  await loggedInvoke('update_world_building', {
    id: worldBuilding.id,
    concept: data.worldBuilding.concept,
    rules: data.worldBuilding.rules,
    history: data.worldBuilding.history,
    cultures: data.worldBuilding.cultures,
  });

  // 3. 角色
  const createdCharacters: Character[] = [];
  for (const profile of data.characters) {
    const character = await loggedInvoke<Character>('create_character', {
      story_id: story.id,
      name: profile.name,
      background: profile.background,
      personality: profile.personality,
      goals: profile.goals,
    });
    createdCharacters.push(character);
  }

  // 4. 文风：获取或创建后完整更新
  let writingStyle = await loggedInvoke<WritingStyle | null>('get_writing_style', {
    story_id: story.id,
  });
  if (!writingStyle) {
    writingStyle = await loggedInvoke<WritingStyle>('create_writing_style', {
      story_id: story.id,
      name: data.writingStyle.name,
    });
  }
  await loggedInvoke('update_writing_style', {
    id: writingStyle.id,
    updates: {
      name: data.writingStyle.name,
      description: data.writingStyle.description,
      tone: data.writingStyle.tone,
      pacing: data.writingStyle.pacing,
      vocabulary_level: data.writingStyle.vocabulary_level,
      sentence_structure: data.writingStyle.sentence_structure,
      custom_rules: [],
    },
  });

  // 5. 首场景
  const characterIds = createdCharacters.map(c => c.id);
  const scene = await loggedInvoke<Scene>('create_scene', {
    story_id: story.id,
    sequence_number: 1,
    title: data.firstScene.title,
    dramatic_goal: data.firstScene.dramatic_goal,
    external_pressure: data.firstScene.external_pressure,
    conflict_type: data.firstScene.conflict_type,
    characters_present: characterIds,
    setting_location: data.firstScene.setting_location,
    setting_time: data.firstScene.setting_time,
    setting_atmosphere: data.firstScene.setting_atmosphere,
    content: data.firstScene.content,
    confidence_score: 0.8,
  });

  return { story, characters: createdCharacters, scene };
}
