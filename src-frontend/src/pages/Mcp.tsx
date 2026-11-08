import { useEffect, useMemo, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Globe,
  Play,
  Plug,
  Plus,
  Power,
  RefreshCw,
  Server,
  Trash2,
  Unplug,
  Wrench,
} from 'lucide-react';
import toast from 'react-hot-toast';
import { Button } from '@/components/ui/Button';
import { Card, CardContent } from '@/components/ui/Card';
import { EmptyState } from '@/components/ui/EmptyState';
import { Field } from '@/components/ui/Field';
import { Input, Select, Textarea } from '@/components/ui/controls';
import { Modal } from '@/components/ui/Modal';
import { AiCodeBlock } from '@/components/ui/ai/AiCodeBlock';
import { useMcpTools, type McpTool, type ExternalServer } from '@/hooks/useMcpTools';
import {
  mcpDeleteServer,
  mcpListSavedServers,
  mcpSaveServer,
  mcpSetServerEnabled,
  connectMcpServer,
  type McpConnectionStatus,
  type SavedMcpServer,
} from '@/services/api/skills';
import { serverIdFromMcpToolName } from '@/utils/mcp';
import { cn } from '@/utils/cn';

function permissionChips(status?: McpConnectionStatus): string[] {
  if (!status || !status.connected) return [];
  const permissions = status.permissions ?? {};
  const labels: string[] = [];
  if (permissions.read?.granted) labels.push('读取');
  if (permissions.network?.granted) labels.push('联网');
  if (permissions.write?.granted) labels.push('写入');
  if (permissions.private?.granted) labels.push('隐私');
  return labels;
}

function statusLabel(status: McpConnectionStatus | undefined, enabled: boolean): string {
  if (status?.connected) return '已连接';
  if (status?.error_code) return '连接失败';
  if (enabled) return '未连接';
  return '已停用';
}

export function Mcp() {
  const {
    tools,
    externalTools,
    isLoading,
    isConnecting,
    connections,
    refreshConnections,
    executeTool,
    callExternalTool,
  } = useMcpTools();

  const [selectedTool, setSelectedTool] = useState<McpTool | null>(null);
  const [toolArgs, setToolArgs] = useState('{}');
  const [toolResult, setToolResult] = useState<unknown>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [serverName, setServerName] = useState('');
  const [serverCommand, setServerCommand] = useState('');
  const [serverArgs, setServerArgs] = useState('');
  const [serverEnv, setServerEnv] = useState('');
  const [transportMode, setTransportMode] = useState<'http' | 'stdio'>('http');
  const [serverUrl, setServerUrl] = useState('');
  const [authToken, setAuthToken] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const queryClient = useQueryClient();

  const savedQuery = useQuery({
    queryKey: ['mcp-saved-servers'],
    queryFn: mcpListSavedServers,
  });
  const savedServers: SavedMcpServer[] = savedQuery.data ?? [];
  const statusById = useMemo(
    () => new Map(connections.map(status => [status.id, status])),
    [connections]
  );

  useEffect(() => {
    void refreshConnections();
  }, [refreshConnections]);

  const externalGroups = useMemo(
    () =>
      externalTools.reduce<Record<string, McpTool[]>>((groups, tool) => {
        const serverId = serverIdFromMcpToolName(tool.name) ?? 'unknown';
        (groups[serverId] ??= []).push(tool);
        return groups;
      }, {}),
    [externalTools]
  );
  const connectedCount = connections.filter(status => status.connected).length;
  const mappingVersions = [
    ...new Set(
      connections
        .filter(status => status.connected)
        .map(status => status.mapping_version)
        .filter((version): version is string => Boolean(version))
    ),
  ];

  const buildConfig = (): ExternalServer => {
    let env: Record<string, string> = {};
    if (transportMode === 'stdio' && serverEnv.trim()) {
      env = JSON.parse(serverEnv) as Record<string, string>;
    }
    return {
      id: crypto.randomUUID(),
      name: serverName.trim(),
      command: transportMode === 'stdio' ? serverCommand.trim() : '',
      args: transportMode === 'stdio' ? serverArgs.trim().split(' ').filter(Boolean) : [],
      env,
      url: transportMode === 'http' ? serverUrl.trim() : null,
      headers:
        transportMode === 'http' && authToken.trim()
          ? { Authorization: `Bearer ${authToken.trim()}` }
          : {},
    };
  };

  const handleSaveServer = async () => {
    if (!serverName.trim()) return;
    if (transportMode === 'http' && !serverUrl.trim()) return;
    if (transportMode === 'stdio' && !serverCommand.trim()) return;
    setIsSaving(true);
    try {
      const config = buildConfig();
      await mcpSaveServer(config, true, false);
      await refreshConnections();
      await queryClient.invalidateQueries({ queryKey: ['mcp-saved-servers'] });
      setAddOpen(false);
      setServerName('');
      setServerCommand('');
      setServerArgs('');
      setServerEnv('');
      setServerUrl('');
      setAuthToken('');
      toast.success(`已保存并自动连接 ${config.name}`);
    } catch (error) {
      toast.error('保存服务器失败: ' + (error as Error).message);
    } finally {
      setIsSaving(false);
    }
  };

  const handleReconnect = async (srv: SavedMcpServer) => {
    try {
      await connectMcpServer(srv.config);
      await refreshConnections();
      toast.success(`已重新连接 ${srv.name}`);
    } catch (error) {
      toast.error('重新连接失败: ' + (error as Error).message);
    }
  };

  const handleToggleSaved = async (srv: SavedMcpServer) => {
    try {
      await mcpSetServerEnabled(srv.id, !srv.enabled);
      await queryClient.invalidateQueries({ queryKey: ['mcp-saved-servers'] });
      await refreshConnections();
    } catch (error) {
      toast.error((error as Error).message);
    }
  };

  const handleDeleteSaved = async (srv: SavedMcpServer) => {
    try {
      await mcpDeleteServer(srv.id);
      await queryClient.invalidateQueries({ queryKey: ['mcp-saved-servers'] });
      await refreshConnections();
      toast.success(`已删除 ${srv.name}`);
    } catch (error) {
      toast.error((error as Error).message);
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
    } catch (error) {
      setToolResult({ error: (error as Error).message });
    } finally {
      setIsExecuting(false);
    }
  };

  const openTool = (tool: McpTool) => {
    setSelectedTool(tool);
    setToolArgs('{}');
    setToolResult(null);
  };

  const renderToolCard = (tool: McpTool) => (
    <Card
      key={tool.name}
      className={cn(
        'cursor-pointer transition-colors',
        selectedTool?.name === tool.name ? 'border-cinema-gold' : ''
      )}
      onClick={() => openTool(tool)}
    >
      <CardContent className="flex items-center gap-3 p-4">
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-cinema-800">
          {tool.source === 'external' ? (
            <Globe className="h-5 w-5 text-blue-400" />
          ) : (
            <Plug className="h-5 w-5 text-cinema-gold" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <h3 className="truncate font-medium text-white">{tool.name}</h3>
          {tool.description && <p className="truncate text-sm text-gray-400">{tool.description}</p>}
        </div>
        <Play className="h-4 w-4 shrink-0 text-cinema-gold" />
      </CardContent>
    </Card>
  );

  return (
    <div className="space-y-5 animate-fade-in">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">MCP 连接</h1>
          <p className="mt-1 text-sm text-ai-ink-3">
            已保存服务器自动连接；全部工具自动注册，无需逐个选择。
          </p>
        </div>
        <Button
          variant="secondary"
          onClick={() => refreshConnections()}
          isLoading={isConnecting || isLoading}
        >
          <RefreshCw className="h-4 w-4" />
          重新扫描
        </Button>
      </div>

      <Card>
        <CardContent className="flex flex-wrap items-center gap-2 p-4">
          <span className="rounded-full bg-green-500/15 px-2.5 py-1 text-xs text-green-400">
            已连接 {connectedCount}/{savedServers.length}
          </span>
          <span className="rounded-full bg-cinema-800 px-2.5 py-1 text-xs text-ai-ink-2">
            外部工具 {externalTools.length}
          </span>
          {mappingVersions.map(version => (
            <span
              key={version}
              className="rounded-full bg-cinema-800 px-2.5 py-1 text-xs text-ai-ink-2"
            >
              映射 v{version}
            </span>
          ))}
          <span className="ml-auto text-xs text-ai-ink-3">
            权限仍由源系统控制，写/联网关闭时按 MCP_* 错误码拒绝。
          </span>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="space-y-3 p-4">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <Server className="h-5 w-5 text-cinema-gold" />
              <h2 className="font-display text-lg font-semibold text-white">服务器</h2>
              <span className="text-xs text-ai-ink-3">已保存 {savedServers.length}</span>
            </div>
            <Button variant="primary" size="sm" onClick={() => setAddOpen(true)}>
              <Plus className="h-4 w-4" />
              添加服务器
            </Button>
          </div>

          {savedServers.length === 0 ? (
            <EmptyState
              icon={Server}
              title="暂无 MCP 服务器"
              action={{ label: '添加服务器', onClick: () => setAddOpen(true) }}
            />
          ) : (
            <div className="space-y-2">
              {savedServers.map(srv => {
                const status = statusById.get(srv.id);
                const label = statusLabel(status, srv.enabled);
                const chips = permissionChips(status);
                return (
                  <div
                    key={srv.id}
                    data-testid={`mcp-saved-${srv.id}`}
                    className="flex flex-wrap items-center gap-3 rounded-lg border border-cinema-700 bg-cinema-800/60 px-3 py-2"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-sm text-white">{srv.name}</span>
                        <span className="rounded bg-cinema-700 px-1.5 py-0.5 text-[10px] text-gray-300">
                          {srv.config.url ? 'HTTP' : 'stdio'}
                        </span>
                        <span
                          className={cn(
                            'rounded px-1.5 py-0.5 text-[10px]',
                            status?.connected
                              ? 'bg-green-500/20 text-green-400'
                              : status?.error_code
                                ? 'bg-red-500/20 text-red-400'
                                : 'bg-cinema-700 text-gray-400'
                          )}
                        >
                          {label}
                        </span>
                        {status?.connected && (
                          <span className="text-[10px] text-ai-ink-3">{status.tools} 工具</span>
                        )}
                        {chips.map(chip => (
                          <span
                            key={chip}
                            className="rounded bg-cinema-700 px-1.5 py-0.5 text-[10px] text-ai-ink-2"
                          >
                            {chip}
                          </span>
                        ))}
                        {status?.mapping_version && (
                          <span className="text-[10px] text-ai-ink-3">
                            映射 v{status.mapping_version}
                          </span>
                        )}
                      </div>
                      <p className="mt-1 truncate text-[11px] text-gray-500">
                        {srv.config.url || srv.config.command}
                      </p>
                      {status?.error_message && (
                        <p className="mt-1 text-xs text-red-400">{status.error_message}</p>
                      )}
                    </div>
                    <div className="flex items-center gap-1">
                      {srv.enabled && !status?.connected && (
                        <button
                          type="button"
                          title="重新连接"
                          onClick={() => handleReconnect(srv)}
                          className="rounded p-1.5 text-cinema-gold hover:bg-cinema-700"
                        >
                          <RefreshCw className="h-4 w-4" />
                        </button>
                      )}
                      <button
                        type="button"
                        title={srv.enabled ? '停用（并断开）' : '启用并连接'}
                        onClick={() => handleToggleSaved(srv)}
                        className={cn(
                          'rounded p-1.5 hover:bg-cinema-700',
                          srv.enabled ? 'text-green-400' : 'text-gray-500'
                        )}
                      >
                        {srv.enabled ? (
                          <Power className="h-4 w-4" />
                        ) : (
                          <Unplug className="h-4 w-4" />
                        )}
                      </button>
                      <button
                        type="button"
                        title="删除配置"
                        onClick={() => handleDeleteSaved(srv)}
                        className="rounded p-1.5 text-red-400 hover:bg-cinema-700"
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardContent className="space-y-3 p-4">
          <div className="flex items-center gap-2">
            <Wrench className="h-5 w-5 text-cinema-gold" />
            <h2 className="font-display text-lg font-semibold text-white">可用工具</h2>
            <span className="text-xs text-ai-ink-3">{externalTools.length}</span>
          </div>
          {externalTools.length === 0 ? (
            <EmptyState icon={Wrench} title="连接服务器后自动列出全部工具" />
          ) : (
            Object.entries(externalGroups).map(([serverId, group]) => (
              <div key={serverId} className="space-y-2">
                <div className="flex items-center gap-2">
                  <span className="text-xs text-ai-ink-3">
                    {connections.find(status => status.id === serverId)?.name ?? serverId}
                  </span>
                  <span className="text-[10px] text-ai-ink-3">{group.length} 个工具</span>
                </div>
                <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                  {group.map(renderToolCard)}
                </div>
              </div>
            ))
          )}
          {tools.length > 0 && (
            <div className="space-y-2 border-t border-cinema-800 pt-3">
              <span className="text-xs text-ai-ink-3">内置工具 {tools.length}</span>
              <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                {tools.map(renderToolCard)}
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <Modal
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title="添加 MCP 服务器"
        size="lg"
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={() => setAddOpen(false)}>
              取消
            </Button>
            <Button
              variant="primary"
              onClick={handleSaveServer}
              isLoading={isSaving}
              disabled={
                !serverName.trim() ||
                (transportMode === 'http' ? !serverUrl.trim() : !serverCommand.trim())
              }
            >
              <Plug className="h-4 w-4" />
              保存并自动连接
            </Button>
          </div>
        }
      >
        <div className="space-y-3">
          <Field label="服务器名称" required>
            <Input
              value={serverName}
              onChange={e => setServerName(e.target.value)}
              placeholder="例如：栗子读书"
            />
          </Field>
          <Field label="传输方式">
            <Select
              value={transportMode}
              onChange={e => setTransportMode(e.target.value as 'http' | 'stdio')}
            >
              <option value="http">HTTP（远程服务）</option>
              <option value="stdio">stdio（本地命令）</option>
            </Select>
          </Field>
          {transportMode === 'http' ? (
            <>
              <Field label="MCP 端点 URL" required>
                <Input
                  value={serverUrl}
                  onChange={e => setServerUrl(e.target.value)}
                  placeholder="http://127.0.0.1:1122/mcp"
                />
              </Field>
              <Field label="Bearer 令牌（可选）">
                <Input
                  type="password"
                  value={authToken}
                  onChange={e => setAuthToken(e.target.value)}
                  placeholder="你的令牌"
                />
              </Field>
            </>
          ) : (
            <>
              <Field label="启动命令" required>
                <Input
                  value={serverCommand}
                  onChange={e => setServerCommand(e.target.value)}
                  placeholder="例如：npx 或 /usr/local/bin/mcp-server"
                />
              </Field>
              <Field label="参数（空格分隔）">
                <Input
                  value={serverArgs}
                  onChange={e => setServerArgs(e.target.value)}
                  placeholder="例如：-y @modelcontextprotocol/server-filesystem"
                />
              </Field>
              <Field label="环境变量 JSON（可选）">
                <Input
                  value={serverEnv}
                  onChange={e => setServerEnv(e.target.value)}
                  placeholder='{"API_KEY":"xxx"}'
                />
              </Field>
            </>
          )}
        </div>
      </Modal>

      <Modal
        open={Boolean(selectedTool)}
        onClose={() => {
          setSelectedTool(null);
          setToolResult(null);
        }}
        title={selectedTool ? `工具调试：${selectedTool.name}` : '工具调试'}
        size="lg"
        footer={
          <Button
            variant="primary"
            onClick={handleExecute}
            isLoading={isExecuting}
            className="w-full"
          >
            <Play className="h-4 w-4" />
            运行
          </Button>
        }
      >
        <div className="space-y-3">
          {selectedTool?.description && (
            <p className="text-sm text-ai-ink-3">{selectedTool.description}</p>
          )}
          <Field label="参数（JSON）">
            <Textarea
              value={toolArgs}
              onChange={e => setToolArgs(e.target.value)}
              rows={5}
              className="font-mono text-xs"
            />
          </Field>
          {toolResult !== null && (
            <AiCodeBlock
              code={JSON.stringify(toolResult, null, 2)}
              title="结果"
              language="JSON"
              maxHeight={220}
            />
          )}
        </div>
      </Modal>
    </div>
  );
}
