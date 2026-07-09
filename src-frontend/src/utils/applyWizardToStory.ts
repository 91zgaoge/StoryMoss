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
  worldBuilding?: WorldBuilding;
  writingStyle?: WritingStyle;
  ingestedEntities?: number;
  ingestedRelations?: number;
}

/**
 * 将 NovelCreationWizard 产出的资产应用到已有故事。
 * 走后端 apply_wizard_to_story：角色按名去重、首场景更新/创建、含 KG 摄取。
 */
export async function applyWizardToStory(
  story: Story,
  data: WizardData
): Promise<WizardApplyResult> {
  const result = await loggedInvoke<{
    story: Story;
    world_building: WorldBuilding;
    writing_style: WritingStyle;
    first_scene: Scene;
    characters: Character[];
    ingested_entities: number;
    ingested_relations: number;
  }>('apply_wizard_to_story', {
    story_id: story.id,
    genre: data.genreInput || story.genre || null,
    style_dna_id: data.selectedStrategy?.style_dna_ids?.[0] ?? story.style_dna_id ?? null,
    genre_profile_id: data.selectedStrategy?.genre_profile_id ?? story.genre_profile_id ?? null,
    methodology_id: data.selectedStrategy?.methodology_id ?? story.methodology_id ?? null,
    world_building: data.worldBuilding,
    characters: data.characters,
    writing_style: data.writingStyle,
    first_scene: data.firstScene,
  });

  return {
    story: result.story,
    characters: result.characters,
    scene: result.first_scene,
    worldBuilding: result.world_building,
    writingStyle: result.writing_style,
    ingestedEntities: result.ingested_entities,
    ingestedRelations: result.ingested_relations,
  };
}
