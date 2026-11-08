import { useState, useCallback } from 'react';
import toast from 'react-hot-toast';
import {
  callMcpTool,
  disconnectMcpServer,
  listMcpTools,
  executeMcpTool,
  registerMcpTool,
  unregisterMcpTool,
  mcpEnsureConnections,
  type McpConnectionStatus,
} from '@/services/tauri';
import { serverIdFromMcpToolName } from '@/utils/mcp';

export interface McpTool {
  name: string;
  description?: string;
  parameters?: Record<string, unknown>;
  source?: 'builtin' | 'external';
}

export interface ExternalServer {
  id: string;
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  url?: string | null;
  headers?: Record<string, string>;
}

export function useMcpTools() {
  const [tools, setTools] = useState<McpTool[]>([]);
  const [externalTools, setExternalTools] = useState<McpTool[]>([]);
  const [connections, setConnections] = useState<McpConnectionStatus[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);

  const listTools = useCallback(async () => {
    setIsLoading(true);
    try {
      const data = await listMcpTools();
      const builtins: McpTool[] = [];
      const externals: McpTool[] = [];
      for (const t of data) {
        if (t.name.startsWith('mcp.')) {
          externals.push({ ...t, source: 'external' as const });
        } else {
          builtins.push({ ...t, source: 'builtin' as const });
        }
      }
      setTools(builtins);
      setExternalTools(externals);
    } catch (error) {
      toast.error('获取工具列表失败: ' + (error as Error).message);
    } finally {
      setIsLoading(false);
    }
  }, []);

  /**
   * 自动连接所有 enabled 的已保存服务器（幂等），随后刷新工具目录。
   * 页面挂载、点击重新扫描、启用服务器后调用。
   */
  const refreshConnections = useCallback(async () => {
    setIsConnecting(true);
    try {
      const statuses = await mcpEnsureConnections();
      setConnections(statuses);
      await listTools();
      return statuses;
    } catch (error) {
      toast.error('自动连接 MCP 服务器失败: ' + (error as Error).message);
      throw error;
    } finally {
      setIsConnecting(false);
    }
  }, [listTools]);

  const executeTool = useCallback(async (toolName: string, args: Record<string, unknown>) => {
    try {
      const result = await executeMcpTool(toolName, args);
      return result;
    } catch (error) {
      toast.error('执行工具失败: ' + (error as Error).message);
      throw error;
    }
  }, []);

  const callExternalTool = useCallback(async (toolName: string, args: Record<string, unknown>) => {
    const serverId = serverIdFromMcpToolName(toolName);
    if (!serverId) {
      throw new Error('工具名缺少 mcp.{server}.{tool} 前缀');
    }
    try {
      const result = await callMcpTool(serverId, toolName, args);
      return result;
    } catch (error) {
      toast.error('调用外部工具失败: ' + (error as Error).message);
      throw error;
    }
  }, []);

  const disconnectServer = useCallback(
    async (serverId: string) => {
      try {
        await disconnectMcpServer(serverId);
      } catch {
        // 断开失败不阻塞本地刷新；后端连接可能已不存在。
      }
      await refreshConnections();
      toast.success('已断开服务器连接');
    },
    [refreshConnections]
  );

  const registerTool = useCallback(
    async (tool: McpTool) => {
      try {
        await registerMcpTool(tool);
        toast.success(`已注册工具: ${tool.name}`);
        await listTools();
      } catch (error) {
        toast.error('注册工具失败: ' + (error as Error).message);
        throw error;
      }
    },
    [listTools]
  );

  const unregisterTool = useCallback(
    async (toolName: string) => {
      try {
        await unregisterMcpTool(toolName);
        toast.success(`已注销工具: ${toolName}`);
        await listTools();
      } catch (error) {
        toast.error('注销工具失败: ' + (error as Error).message);
        throw error;
      }
    },
    [listTools]
  );

  return {
    tools,
    externalTools,
    allTools: [...tools, ...externalTools],
    connections,
    isLoading,
    isConnecting,
    listTools,
    refreshConnections,
    executeTool,
    callExternalTool,
    disconnectServer,
    registerTool,
    unregisterTool,
  };
}
