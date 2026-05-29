import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import { save, open } from '@tauri-apps/plugin-dialog';
import { writeFile, readFile } from '@tauri-apps/plugin-fs';
import type { StudioConfig, StudioExportRequest, ImportOptions, Story } from '@/types';

const STUDIO_CONFIG_KEY = 'studio_config';

// ==================== Studio Config ====================

export function useStudioConfig(storyId: string | null) {
  return useQuery({
    queryKey: [STUDIO_CONFIG_KEY, storyId],
    queryFn: async () => {
      if (!storyId) return null;
      return loggedInvoke<StudioConfig | null>('get_studio_config', { story_id: storyId });
    },
    enabled: !!storyId,
  });
}

export function useCreateStudioConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (storyId: string) => {
      return loggedInvoke<StudioConfig>('create_studio_config', { story_id: storyId });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [STUDIO_CONFIG_KEY, variables] });
    },
  });
}

export function useUpdateStudioConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      id: string;
      storyId: string;
      penName?: string;
      llmConfig?: StudioConfig['llm_config'];
      uiConfig?: StudioConfig['ui_config'];
      agentBots?: StudioConfig['agent_bots'];
    }) => {
      return loggedInvoke<number>('update_studio_config', {
        id: params.id,
        pen_name: params.penName,
        llm_config: params.llmConfig,
        ui_config: params.uiConfig,
        agent_bots: params.agentBots,
      });
    },
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: [STUDIO_CONFIG_KEY, variables.storyId] });
    },
  });
}

// ==================== Export/Import ====================

export function useExportStudio() {
  return useMutation({
    mutationFn: async (request: StudioExportRequest) => {
      const data = await loggedInvoke<Uint8Array>('export_studio', { request });

      // Save to file
      const filePath = await save({
        filters: [{ name: 'StoryForge Studio', extensions: ['storyforge'] }],
        defaultPath: `${request.story_id}.storyforge`,
      });

      if (filePath) {
        await writeFile(filePath, data);
      }

      return filePath;
    },
  });
}

export function useImportStudio() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (options: ImportOptions) => {
      // Open file dialog
      const filePath = await open({
        filters: [{ name: 'StoryForge Studio', extensions: ['storyforge'] }],
        multiple: false,
      });

      if (!filePath) {
        throw new Error('No file selected');
      }

      // Read file
      const data = await readFile(filePath as string);

      // Import
      const story = await loggedInvoke<Story>('import_studio', {
        data: Array.from(data),
        options,
      });

      return story;
    },
    onSuccess: () => {
      // Invalidate all story-related queries
      queryClient.invalidateQueries({ queryKey: ['stories'] });
    },
  });
}
