import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Plug,
  Plus,
  TestTube,
  Play,
  Wrench,
  Server,
  Unplug,
  Globe,
  Trash2,
  Save,
  Power,
} from 'lucide-react';
import toast from 'react-hot-toast';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { AiCodeBlock } from '@/components/ui/ai/AiCodeBlock';
import { useMcpTools, type McpTool, type ExternalServer } from '@/hooks/useMcpTools';
import {
  mcpListSavedServers,
  mcpSaveServer,
  mcpDeleteServer,
  mcpSetServerEnabled,
  disconnectMcpServer,
  type SavedMcpServer,
} from '@/services/api/skills';

export function Mcp() {
  const {
    tools,
    externalTools,
    allTools,
    isLoading,
    isConnecting,
    connectedServer,
    listTools,
    executeTool,
    connectServer,
    callExternalTool,
    disconnectServer,
  } = useMcpTools();

  const [selectedTool, setSelectedTool] = useState<McpTool | null>(null);
  const [toolArgs, setToolArgs] = useState('{}');
  const [toolResult, setToolResult] = useState<unknown>(null);
  const [isExecuting, setIsExecuting] = useState(false);

  const [serverName, setServerName] = useState('');
  const [serverCommand, setServerCommand] = useState('');
  const [serverArgs, setServerArgs] = useState('');
  const [serverEnv, setServerEnv] = useState('');
  // v0.88.0: 传输模式与 HTTP 字段（栗子读书 legado 等）
  const [transportMode, setTransportMode] = useState<'stdio' | 'http'>('http');
  const [serverUrl, setServerUrl] = useState('');
  const [authToken, setAuthToken] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const queryClient = useQueryClient();

  // 持久化的服务器配置（重启后自动重连 HTTP 型）
  const savedQuery = useQuery({
    queryKey: ['mcp-saved-servers'],
    queryFn: mcpListSavedServers,
  });
  const savedServers: SavedMcpServer[] = savedQuery.data ?? [];

  useEffect(() => {
    listTools();
  }, [listTools]);

  const currentServerId = connectedServer?.id;

  /** 断开当前连接并刷新外部工具列表 */
  const refreshAfterConnect = async () => {
    await listTools();
  };

  const buildConfig = (id?: string): ExternalServer => ({
    id: id || crypto.randomUUID(),
    name: serverName.trim(),
    command: transportMode === 'stdio' ? serverCommand.trim() : '',
    args: transportMode === 'stdio' ? serverArgs.trim().split(' ').filter(Boolean) : [],
    env:
      transportMode === 'stdio' && serverEnv.trim()
        ? (JSON.parse(serverEnv) as Record<string, string>)
        : {},
    url: transportMode === 'http' ? serverUrl.trim() : null,
    headers: (transportMode === 'http' && authToken.trim()
      ? { Authorization: `Bearer ${authToken.trim()}` }
      : {}) as Record<string, string>,
  });

  const handleConnect = async () => {
    if (!serverName.trim()) return;
    if (transportMode === 'stdio' && !serverCommand.trim()) return;
    if (transportMode === 'http' && !serverUrl.trim()) return;
    await connectServer(buildConfig());
  };

  /** 保存到本机数据库并连接（HTTP 型服务器重启后自动重连） */
  const handleSaveAndConnect = async () => {
    if (!serverName.trim()) return;
    if (transportMode === 'stdio' && !serverCommand.trim()) return;
    if (transportMode === 'http' && !serverUrl.trim()) return;
    setIsSaving(true);
    try {
      const config = buildConfig();
      await mcpSaveServer(config, true, false);
      await connectServer(config);
      await queryClient.invalidateQueries({ queryKey: ['mcp-saved-servers'] });
      await refreshAfterConnect();
      toast.success(`已保存并连接 ${config.name}`);
    } catch (e) {
      toast.error('保存失败: ' + (e as Error).message);
    } finally {
      setIsSaving(false);
    }
  };

  const savedToExternal = (srv: SavedMcpServer) => ({
    id: srv.id,
    name: srv.name,
    command: srv.config.command ?? '',
    args: srv.config.args ?? [],
    env: srv.config.env,
    url: srv.config.url,
    headers: srv.config.headers,
  });

  const handleToggleSaved = async (srv: SavedMcpServer) => {
    try {
      if (srv.enabled) {
        await mcpSetServerEnabled(srv.id, false);
        await disconnectMcpServer(srv.id).catch(() => undefined);
      } else {
        await mcpSaveServer(srv.config, true, false);
        await connectServer(savedToExternal(srv));
        await refreshAfterConnect();
      }
      await queryClient.invalidateQueries({ queryKey: ['mcp-saved-servers'] });
      await listTools();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const handleDeleteSaved = async (srv: SavedMcpServer) => {
    try {
      await mcpDeleteServer(srv.id);
      await queryClient.invalidateQueries({ queryKey: ['mcp-saved-servers'] });
      await listTools();
      toast.success(`已删除 ${srv.name}`);
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const handleConnectSaved = async (srv: SavedMcpServer) => {
    try {
      await connectServer(savedToExternal(srv));
      await refreshAfterConnect();
      toast.success(`已连接 ${srv.name}`);
    } catch (e) {
      toast.error('连接失败: ' + (e as Error).message);
    }
  };

  const handleExecute = async () => {
    if (!selectedTool) return;
    setIsExecuting(true);
    let args: Record<string, unknown>;
    try {
      args = JSON.parse(toolArgs) as Record<string, unknown>;
    } catch {
      setToolResult({ error: '参数不是合法 JSON' });
      setIsExecuting(false);
      return;
    }
    try {
      const result =
        selectedTool.source === 'external'
          ? await callExternalTool(selectedTool.name, args)
          : await executeTool(selectedTool.name, args);
      setToolResult(result);
    } catch (e) {
      setToolResult({ error: (e as Error).message });
    } finally {
      setIsExecuting(false);
    }
  };

  const handleSelectTool = (tool: McpTool) => {
    setSelectedTool(tool);
    setToolResult(null);
  };

  const renderToolCard = (tool: McpTool) => (
    <Card
      key={`${tool.source}-${tool.name}`}
      className={`cursor-pointer transition-colors ${
        selectedTool?.name === tool.name && selectedTool?.source === tool.source
          ? 'border-cinema-gold'
          : ''
      }`}
      onClick={() => handleSelectTool(tool)}
    >
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-cinema-800 flex items-center justify-center">
            {tool.source === 'external' ? (
              <Globe className="w-5 h-5 text-blue-400" />
            ) : (
              <Plug className="w-5 h-5 text-cinema-gold" />
            )}
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="font-medium text-white truncate">{tool.name}</h3>
              {tool.source === 'external' && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-300">
                  外部
                </span>
              )}
            </div>
            {tool.description && (
              <p className="text-sm text-gray-400 truncate">{tool.description}</p>
            )}
          </div>
          {selectedTool?.name === tool.name && selectedTool?.source === tool.source && (
            <Play className="w-4 h-4 text-cinema-gold shrink-0" />
          )}
        </div>
      </CardContent>
    </Card>
  );

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">MCP 工具</h1>
          <p className="text-gray-400">内置与外部 Model Context Protocol 工具</p>
        </div>
        <Button variant="primary" onClick={listTools} isLoading={isLoading}>
          <Plus className="w-4 h-4" />
          刷新工具
        </Button>
      </div>

      {/* External Server Config */}
      <Card>
        <CardContent className="p-5 space-y-4">
          <div className="flex items-center gap-2 mb-2">
            <Server className="w-5 h-5 text-cinema-gold" />
            <h2 className="font-display text-lg font-semibold text-white">外部服务器</h2>
            {connectedServer && (
              <span className="ml-2 text-xs px-2 py-0.5 rounded-full bg-green-500/20 text-green-400">
                已连接: {connectedServer.name}
              </span>
            )}
          </div>

          {/* v0.88.0: 传输模式——流式 HTTP（栗子读书 legado 等）或 stdio 子进程 */}
          <div className="flex items-center gap-2">
            {(['http', 'stdio'] as const).map(m => (
              <button
                key={m}
                type="button"
                onClick={() => setTransportMode(m)}
                className={`px-3 py-1 rounded-lg text-xs border transition-colors ${
                  transportMode === m
                    ? 'border-cinema-gold text-cinema-gold bg-cinema-gold/10'
                    : 'border-cinema-700 text-gray-400 hover:text-white'
                }`}
              >
                {m === 'http' ? 'HTTP（远程服务）' : 'stdio（本地命令）'}
              </button>
            ))}
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div>
              <label className="block text-xs text-gray-400 mb-1">服务器名称</label>
              <input
                type="text"
                value={serverName}
                onChange={e => setServerName(e.target.value)}
                placeholder="例如: 栗子读书"
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
              />
            </div>
            {transportMode === 'http' ? (
              <>
                <div>
                  <label className="block text-xs text-gray-400 mb-1">MCP 端点 URL</label>
                  <input
                    type="text"
                    value={serverUrl}
                    onChange={e => setServerUrl(e.target.value)}
                    placeholder="http://127.0.0.1:1122/mcp"
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-400 mb-1">Bearer 令牌 (可选)</label>
                  <input
                    type="password"
                    value={authToken}
                    onChange={e => setAuthToken(e.target.value)}
                    placeholder="你的令牌"
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                  />
                </div>
              </>
            ) : (
              <>
                <div>
                  <label className="block text-xs text-gray-400 mb-1">启动命令</label>
                  <input
                    type="text"
                    value={serverCommand}
                    onChange={e => setServerCommand(e.target.value)}
                    placeholder="例如: npx 或 /usr/local/bin/mcp-server"
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-400 mb-1">参数 (空格分隔)</label>
                  <input
                    type="text"
                    value={serverArgs}
                    onChange={e => setServerArgs(e.target.value)}
                    placeholder="例如: -y @modelcontextprotocol/server-filesystem"
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-400 mb-1">环境变量 JSON (可选)</label>
                  <input
                    type="text"
                    value={serverEnv}
                    onChange={e => setServerEnv(e.target.value)}
                    placeholder='{"API_KEY":"xxx"}'
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                  />
                </div>
              </>
            )}
          </div>

          <div className="flex items-center gap-3">
            <Button
              variant="primary"
              onClick={handleSaveAndConnect}
              isLoading={isSaving || isConnecting}
              disabled={
                !serverName.trim() ||
                (transportMode === 'http' ? !serverUrl.trim() : !serverCommand.trim())
              }
            >
              <Save className="w-4 h-4" />
              保存并连接
            </Button>
            <Button
              variant="secondary"
              onClick={handleConnect}
              isLoading={isConnecting}
              disabled={
                !serverName.trim() ||
                (transportMode === 'http' ? !serverUrl.trim() : !serverCommand.trim())
              }
            >
              <Plug className="w-4 h-4" />
              仅连接（不保存）
            </Button>
            {connectedServer && (
              <Button variant="secondary" onClick={disconnectServer}>
                <Unplug className="w-4 h-4" />
                断开连接
              </Button>
            )}
          </div>

          {/* 已保存的服务器（V156 持久化；HTTP 型随应用启动自动重连） */}
          {savedServers.length > 0 && (
            <div className="space-y-2 pt-2 border-t border-cinema-800">
              <h3 className="text-xs font-semibold text-gray-300">
                已保存 ({savedServers.length}) — HTTP 型服务器随应用启动自动重连
              </h3>
              {savedServers.map(srv => (
                <div
                  key={srv.id}
                  data-testid={`mcp-saved-${srv.id}`}
                  className="flex items-center gap-3 rounded-lg border border-cinema-700 bg-cinema-800/60 px-3 py-2"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-white truncate">{srv.name}</span>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-700 text-gray-300">
                        {srv.config.url ? 'HTTP' : 'stdio'}
                      </span>
                      {currentServerId === srv.id && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-green-500/20 text-green-400">
                          已连接
                        </span>
                      )}
                    </div>
                    <p className="text-[11px] text-gray-500 truncate">
                      {srv.config.url || srv.config.command}
                    </p>
                  </div>
                  <button
                    type="button"
                    title={srv.enabled ? '停用（并断开）' : '启用并连接'}
                    onClick={() => handleToggleSaved(srv)}
                    className={`p-1.5 rounded ${srv.enabled ? 'text-green-400' : 'text-gray-500'} hover:bg-cinema-700`}
                  >
                    <Power className="w-4 h-4" />
                  </button>
                  {!currentServerId && (
                    <button
                      type="button"
                      title="连接"
                      onClick={() => handleConnectSaved(srv)}
                      className="p-1.5 rounded text-cinema-gold hover:bg-cinema-700"
                    >
                      <Plug className="w-4 h-4" />
                    </button>
                  )}
                  <button
                    type="button"
                    title="删除配置"
                    onClick={() => handleDeleteSaved(srv)}
                    className="p-1.5 rounded text-red-400 hover:bg-cinema-700"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Tools List */}
        <div className="space-y-4">
          {tools.length > 0 && (
            <div className="space-y-3">
              <h2 className="font-display text-lg font-semibold text-white flex items-center gap-2">
                <Wrench className="w-5 h-5 text-cinema-gold" />
                内置工具 ({tools.length})
              </h2>
              {tools.map(renderToolCard)}
            </div>
          )}

          {externalTools.length > 0 && (
            <div className="space-y-3 pt-2 border-t border-cinema-800">
              <h2 className="font-display text-lg font-semibold text-white flex items-center gap-2">
                <Globe className="w-5 h-5 text-blue-400" />
                外部工具 ({externalTools.length})
              </h2>
              {externalTools.map(renderToolCard)}
            </div>
          )}

          {allTools.length === 0 && !isLoading && (
            <Card>
              <CardContent className="p-8 text-center text-gray-500">
                <Wrench className="w-12 h-12 mx-auto mb-4 opacity-50" />
                <p>暂无可用工具</p>
              </CardContent>
            </Card>
          )}
        </div>

        {/* Tool Execution */}
        <div className="space-y-4">
          <h2 className="font-display text-lg font-semibold text-white flex items-center gap-2">
            <TestTube className="w-5 h-5 text-cinema-gold" />
            工具执行
          </h2>

          {selectedTool ? (
            <Card>
              <CardContent className="p-4 space-y-4">
                <div className="flex items-center gap-2">
                  <span
                    className={`text-xs px-2 py-0.5 rounded ${
                      selectedTool.source === 'external'
                        ? 'bg-blue-500/20 text-blue-300'
                        : 'bg-cinema-700 text-gray-300'
                    }`}
                  >
                    {selectedTool.source === 'external' ? '外部' : '内置'}
                  </span>
                  <span className="text-white font-medium">{selectedTool.name}</span>
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-2">参数 (JSON)</label>
                  <textarea
                    value={toolArgs}
                    onChange={e => setToolArgs(e.target.value)}
                    rows={6}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white font-mono text-sm focus:border-cinema-gold focus:outline-none"
                    placeholder='{"key": "value"}'
                  />
                </div>

                <Button
                  variant="primary"
                  onClick={handleExecute}
                  isLoading={isExecuting}
                  className="w-full gap-2"
                >
                  <Play className="w-4 h-4" />
                  执行 {selectedTool.name}
                </Button>

                {toolResult !== null && (
                  <AiCodeBlock
                    className="mt-4"
                    code={JSON.stringify(toolResult, null, 2)}
                    title="结果"
                    language="JSON"
                    maxHeight={240}
                  />
                )}
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardContent className="p-8 text-center text-gray-500">
                <Wrench className="w-12 h-12 mx-auto mb-4 opacity-50" />
                <p>选择一个工具开始执行</p>
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
