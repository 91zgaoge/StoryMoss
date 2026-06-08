import { loggedInvoke } from './core';
import type {
  WorldBuildingOption,
  CharacterProfileOption,
  WritingStyleOption,
  SceneProposal,
} from '@/types/v3';
import type { WizardCreationResult } from '@/types/index';
// Novel Creation Wizard
export const generateWorldBuildingOptions = (userInput: string) =>
  loggedInvoke<WorldBuildingOption[]>('generate_world_building_options', { user_input: userInput });

export const generateCharacterProfiles = (worldBuilding: WorldBuildingOption) =>
  loggedInvoke<CharacterProfileOption[][]>('generate_character_profiles', {
    world_building: worldBuilding,
  });

export const generateWritingStyles = (genre: string, worldBuilding: WorldBuildingOption) =>
  loggedInvoke<WritingStyleOption[]>('generate_writing_styles', {
    genre,
    world_building: worldBuilding,
  });

export const generateFirstScene = (
  worldBuilding: WorldBuildingOption,
  characters: CharacterProfileOption[],
  writingStyle: WritingStyleOption
) =>
  loggedInvoke<SceneProposal>('generate_first_scene', {
    world_building: worldBuilding,
    characters,
    writing_style: writingStyle,
  });

export const createStoryWithWizard = (params: {
  title: string;
  description?: string;
  genre?: string;
  world_building: WorldBuildingOption;
  characters: CharacterProfileOption[];
  writing_style: WritingStyleOption;
  first_scene: SceneProposal;
}) =>
  loggedInvoke<import('@/types/index').WizardCreationResult>('create_story_with_wizard', params);
