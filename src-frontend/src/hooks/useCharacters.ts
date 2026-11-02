import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getStoryCharacters,
  createCharacter,
  updateCharacter,
  deleteCharacter,
  setCharacterLifeStatus,
  notifyFrontstageDataRefresh,
  loggedInvoke,
} from '@/services/tauri';

export interface CharacterNameRecord {
  character_id: string;
  story_id: string;
  formal: string;
  surname: string;
  given: string;
  courtesy: string | null;
  art: string | null;
  childhood: string | null;
  nickname: string | null;
  aliases_json: string;
  titles_json: string;
  taboo_json: string;
  address_rules_json: string;
  name_story: string;
}

export interface CharacterNameUpdate {
  characterId: string;
  storyId: string;
  courtesy?: string;
  art?: string;
  childhood?: string;
  nickname?: string;
  aliasesJson?: string;
  titlesJson?: string;
  tabooJson?: string;
  addressRulesJson?: string;
  nameStory?: string;
}

const CHARACTER_NAME_KEY = 'character-name';
import type { CreateCharacterRequest, Character } from '@/types/index';
import toast from 'react-hot-toast';
import { useEffect } from 'react';

const CHARACTERS_KEY = 'characters';

export function useCharacters(storyId: string | null) {
  // W2-F2: characters-refreshed DOM CustomEvent 已废弃，数据刷新由 useSyncStore 统一处理
  return useQuery<Character[]>({
    queryKey: [CHARACTERS_KEY, storyId],
    queryFn: () => (storyId ? getStoryCharacters(storyId) : Promise.resolve([])),
    enabled: !!storyId,
  });
}

export function useCreateCharacter() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: createCharacter,
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({
        queryKey: [CHARACTERS_KEY, variables.story_id],
      });
      toast.success('角色创建成功');
    },
    onError: (error: Error) => {
      toast.error('创建失败: ' + error.message);
    },
  });
}

export function useUpdateCharacter() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, updates }: { id: string; updates: Partial<Character> }) =>
      updateCharacter(id, updates),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY] });
      toast.success('角色更新成功');
    },
    onError: (error: Error) => {
      toast.error('更新失败: ' + error.message);
    },
  });
}

export function useDeleteCharacter() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: deleteCharacter,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY] });
      toast.success('角色已删除');
    },
    onError: (error: Error) => {
      toast.error('删除失败: ' + error.message);
    },
  });
}

/**
 * v0.64.7：作者改判生死。
 * 正文判定是单调的（写成死亡即持久化），这里是唯一回写口——
 * 用于假死/诈死情节「其实没死」，或手动把角色标记身故。
 */
export function useCharacterName(characterId: string | null) {
  return useQuery<CharacterNameRecord | null>({
    queryKey: [CHARACTER_NAME_KEY, characterId],
    queryFn: () =>
      loggedInvoke<CharacterNameRecord | null>('get_character_name', {
        character_id: characterId,
      }),
    enabled: !!characterId,
  });
}

export function useUpdateCharacterName() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (update: CharacterNameUpdate) =>
      loggedInvoke<CharacterNameRecord>('update_character_name', {
        character_id: update.characterId,
        story_id: update.storyId,
        courtesy: update.courtesy ?? null,
        art: update.art ?? null,
        childhood: update.childhood ?? null,
        nickname: update.nickname ?? null,
        aliases_json: update.aliasesJson ?? null,
        titles_json: update.titlesJson ?? null,
        taboo_json: update.tabooJson ?? null,
        address_rules_json: update.addressRulesJson ?? null,
        name_story: update.nameStory ?? null,
      }),
    onSuccess: (_, update) => {
      queryClient.invalidateQueries({
        queryKey: [CHARACTER_NAME_KEY, update.characterId],
      });
      toast.success('称呼规则已保存');
    },
    onError: (error: Error) => {
      toast.error('称呼规则保存失败: ' + error.message);
    },
  });
}

export function useSetCharacterLifeStatus() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ storyId, name, alive }: { storyId: string; name: string; alive: boolean }) =>
      setCharacterLifeStatus(storyId, name, alive),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY] });
      queryClient.invalidateQueries({ queryKey: [CHARACTERS_KEY, variables.storyId] });
      toast.success(variables.alive ? '已改回存活（假死情节）' : '已标记身故');
    },
    onError: (error: Error) => {
      toast.error('生死状态更新失败: ' + error.message);
    },
  });
}
