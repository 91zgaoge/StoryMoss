import { useEffect, useState } from 'react';
import { Plug, Plus, TestTube, Play, Wrench, Server, Unplug, Globe } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useMcpTools, type McpTool } from '@/hooks/useMcpTools';

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

  useEffect(() => {
    listTools();
  }, [listTools]);

  const handleConnect = async () => {
    if (!serverName.trim() || !serverCommand.trim()) return;
    await connectServer({
      id: crypto.randomUUID(),
      name: serverName.trim(),
      command: serverCommand.trim(),
      args: serverArgs.trim(),
      env: serverEnv.trim() || undefined,
    });
  };

  const handleExecute = async () => {
    if (!selectedTool) return;
    setIsExecuting(true);
    try {
      const args = JSON.parse(toolArgs);
      const result =
        selectedTool.source === 'external'
          ? await callExternalTool(selectedTool.name, args)
          : await executeTool(selectedTool.name, args);
      setToolResult(result);
    } catch (e) {
      setToolResult({ error: 'Invalid JSON arguments' });
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

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div>
              <label className="block text-xs text-gray-400 mb-1">服务器名称</label>
              <input
                type="text"
                value={serverName}
                onChange={e => setServerName(e.target.value)}
                placeholder="例如: Filesystem MCP"
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
              />
            </div>
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
          </div>

          <div className="flex items-center gap-3">
            <Button
              variant="primary"
              onClick={handleConnect}
              isLoading={isConnecting}
              disabled={!serverName.trim() || !serverCommand.trim()}
            >
              <Plug className="w-4 h-4" />
              连接服务器
            </Button>
            {connectedServer && (
              <Button variant="secondary" onClick={disconnectServer}>
                <Unplug className="w-4 h-4" />
                断开连接
              </Button>
            )}
          </div>
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
                  <div className="mt-4">
                    <label className="block text-sm text-gray-400 mb-2">结果:</label>
                    <pre className="bg-cinema-900 p-3 rounded-lg text-xs text-gray-300 overflow-auto max-h-60">
                      {JSON.stringify(toolResult, null, 2)}
                    </pre>
                  </div>
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
