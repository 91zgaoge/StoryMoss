import { useState, useCallback } from 'react';
import { loggedInvoke } from '@/services/tauri';
import toast from 'react-hot-toast';
import { callMcpTool, disconnectMcpServer, listMcpTools, executeMcpTool, registerMcpTool, unregisterMcpTool } from '@/services/tauri';

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
  args: string;
  env?: string;
}

export function useMcpTools() {
  const [tools, setTools] = useState<McpTool[]>([]);
  const [externalTools, setExternalTools] = useState<McpTool[]>([]);
  const [connectedServer, setConnectedServer] = useState<ExternalServer | null>(null);
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

  const executeTool = useCallback(async (toolName: string, args: Record<string, unknown>) => {
    try {
      const result = await executeMcpTool(toolName, args);
      return result;
    } catch (error) {
      toast.error('执行工具失败: ' + (error as Error).message);
      throw error;
    }
  }, []);

  const connectServer = useCallback(async (config: ExternalServer) => {
    setIsConnecting(true);
    try {
      const env: Record<string, string> = config.env ? JSON.parse(config.env) : {};
      const serverConfig = {
        id: config.id || crypto.randomUUID(),
        name: config.name,
        command: config.command,
        args: config.args.split(' ').filter(Boolean),
        env,
      };
      const data = await loggedInvoke<McpTool[]>('connect_mcp_server', { config: serverConfig });
      setExternalTools(data.map((t) => ({ ...t, source: 'external' as const })));
      setConnectedServer(config);
      toast.success(`已连接到 ${config.name}，发现 ${data.length} 个工具`);
      return data;
    } catch (error) {
      toast.error('连接外部服务器失败: ' + (error as Error).message);
      setExternalTools([]);
      setConnectedServer(null);
      throw error;
    } finally {
      setIsConnecting(false);
    }
  }, []);

  const callExternalTool = useCallback(async (toolName: string, args: Record<string, unknown>) => {
    if (!connectedServer) throw new Error('未连接外部服务器');
    try {
      const result = await callMcpTool(connectedServer.id, toolName, args);
      return result;
    } catch (error) {
      toast.error('调用外部工具失败: ' + (error as Error).message);
      throw error;
    }
  }, [connectedServer]);

  const disconnectServer = useCallback(async () => {
    if (connectedServer) {
      try {
        await disconnectMcpServer(connectedServer.id);
      } catch {
        // ignore
      }
    }
    setExternalTools([]);
    setConnectedServer(null);
    toast.success('已断开外部服务器连接');
  }, [connectedServer]);

  const registerTool = useCallback(async (tool: McpTool) => {
    try {
      await registerMcpTool(tool);
      toast.success(`已注册工具: ${tool.name}`);
      await listTools();
    } catch (error) {
      toast.error('注册工具失败: ' + (error as Error).message);
      throw error;
    }
  }, [listTools]);

  const unregisterTool = useCallback(async (toolName: string) => {
    try {
      await unregisterMcpTool(toolName);
      toast.success(`已注销工具: ${toolName}`);
      await listTools();
    } catch (error) {
      toast.error('注销工具失败: ' + (error as Error).message);
      throw error;
    }
  }, [listTools]);

  return {
    tools,
    externalTools,
    allTools: [...tools, ...externalTools],
    isLoading,
    isConnecting,
    connectedServer,
    listTools,
    executeTool,
    connectServer,
    callExternalTool,
    disconnectServer,
    registerTool,
    unregisterTool,
  };
}
