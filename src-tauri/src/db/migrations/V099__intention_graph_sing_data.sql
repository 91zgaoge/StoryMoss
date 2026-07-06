-- Intention Graph (SING) 数据层
-- 基于 arXiv:2606.16591v2 的意图-工具异构图理论

-- 意图节点表：原子化的动词-宾语短语，全局归一化
CREATE TABLE IF NOT EXISTS intention_nodes (
    id TEXT PRIMARY KEY,
    intent_type TEXT NOT NULL,          -- 'atomic' | 'compound' | 'synthetic'
    verb TEXT NOT NULL,                 -- 动作动词（如 generate, enhance, analyze）
    object TEXT NOT NULL,               -- 宾语对象（如 prose, style, character）
    description TEXT NOT NULL,          -- 自然语言描述
    embedding BLOB,                     -- 语义嵌入向量（JSON 数组）
    frequency INTEGER NOT NULL DEFAULT 1, -- 出现频率（用于 PPR 权重）
    created_at INTEGER NOT NULL,        -- Unix timestamp
    updated_at INTEGER NOT NULL         -- Unix timestamp
);

CREATE INDEX IF NOT EXISTS idx_intention_nodes_verb ON intention_nodes(verb);
CREATE INDEX IF NOT EXISTS idx_intention_nodes_object ON intention_nodes(object);
CREATE INDEX IF NOT EXISTS idx_intention_nodes_type ON intention_nodes(intent_type);

-- 资产节点表：技能、方法论、风格、MCP 工具等
CREATE TABLE IF NOT EXISTS asset_nodes (
    id TEXT PRIMARY KEY,
    asset_type TEXT NOT NULL,           -- 'skill' | 'methodology' | 'style_dna' | 'genre_profile' | 'mcp_tool' | 'agent' | 'system_command'
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    embedding BLOB,                     -- 语义嵌入向量
    capability_id TEXT,               -- 关联到 CapabilityRegistry 的 ID
    metadata TEXT,                    -- JSON: { parameters, constraints, tags, ... }
    frequency INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_asset_nodes_type ON asset_nodes(asset_type);
CREATE INDEX IF NOT EXISTS idx_asset_nodes_capability ON asset_nodes(capability_id);

-- 意图-资产边表：意图触发资产、资产被意图触发
CREATE TABLE IF NOT EXISTS intention_asset_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    intention_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    edge_type TEXT NOT NULL,            -- 'has_intention' | 'triggered_by' | 'recommended'
    weight REAL NOT NULL DEFAULT 1.0,   -- 边权重（0-1）
    reason TEXT,                        -- 为什么建立这条边（LLM 解释或规则来源）
    cooccurrence_count INTEGER NOT NULL DEFAULT 1, -- 共现次数（用于动态更新）
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (intention_id) REFERENCES intention_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES asset_nodes(id) ON DELETE CASCADE,
    UNIQUE(intention_id, asset_id, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_intention_asset_edges_intention ON intention_asset_edges(intention_id);
CREATE INDEX IF NOT EXISTS idx_intention_asset_edges_asset ON intention_asset_edges(asset_id);
CREATE INDEX IF NOT EXISTS idx_intention_asset_edges_type ON intention_asset_edges(edge_type);

-- 资产-资产边表：工具共现、工具链（tool_next）
CREATE TABLE IF NOT EXISTS asset_asset_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_asset_id TEXT NOT NULL,
    target_asset_id TEXT NOT NULL,
    edge_type TEXT NOT NULL,            -- 'tool_next' | 'tool_cooccur' | 'depends_on' | 'complements'
    weight REAL NOT NULL DEFAULT 1.0,
    cooccurrence_count INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (source_asset_id) REFERENCES asset_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (target_asset_id) REFERENCES asset_nodes(id) ON DELETE CASCADE,
    UNIQUE(source_asset_id, target_asset_id, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_asset_asset_edges_source ON asset_asset_edges(source_asset_id);
CREATE INDEX IF NOT EXISTS idx_asset_asset_edges_target ON asset_asset_edges(target_asset_id);
CREATE INDEX IF NOT EXISTS idx_asset_asset_edges_type ON asset_asset_edges(edge_type);

-- 执行图实例表：运行时动态构建的执行图
CREATE TABLE IF NOT EXISTS execution_graphs (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,           -- 用户请求唯一标识
    story_id TEXT,                      -- 关联故事 ID
    user_input TEXT NOT NULL,           -- 原始用户输入
    root_intention_id TEXT,             -- 根意图节点 ID
    status TEXT NOT NULL,               -- 'building' | 'executing' | 'completed' | 'failed' | 'cancelled'
    plan_json TEXT,                     -- 生成的 ExecutionPlan JSON
    result_json TEXT,                   -- 执行结果 JSON
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    execution_time_ms INTEGER           -- 总执行时间
);

CREATE INDEX IF NOT EXISTS idx_execution_graphs_request ON execution_graphs(request_id);
CREATE INDEX IF NOT EXISTS idx_execution_graphs_story ON execution_graphs(story_id);
CREATE INDEX IF NOT EXISTS idx_execution_graphs_status ON execution_graphs(status);

-- 执行图节点表：运行时动态发现的节点
CREATE TABLE IF NOT EXISTS execution_nodes (
    id TEXT PRIMARY KEY,
    graph_id TEXT NOT NULL,
    intention_id TEXT,                  -- 关联意图（可为空，表示直接资产调用）
    asset_id TEXT,                      -- 关联资产（可为空，表示纯意图节点）
    status TEXT NOT NULL,                 -- 'discovered' | 'pending' | 'running' | 'completed' | 'failed' | 'skipped'
    parameters TEXT,                    -- JSON 参数
    depends_on TEXT,                    -- JSON 数组：依赖的 execution_node IDs
    outputs TEXT,                       -- JSON 执行输出
    discovered_from TEXT,               -- 发现来源：'synthesis' | 'ppr' | 'semantic' | 'output_heuristic' | 'llm_assisted'
    execution_time_ms INTEGER,
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    FOREIGN KEY (graph_id) REFERENCES execution_graphs(id) ON DELETE CASCADE,
    FOREIGN KEY (intention_id) REFERENCES intention_nodes(id) ON DELETE SET NULL,
    FOREIGN KEY (asset_id) REFERENCES asset_nodes(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_execution_nodes_graph ON execution_nodes(graph_id);
CREATE INDEX IF NOT EXISTS idx_execution_nodes_intention ON execution_nodes(intention_id);
CREATE INDEX IF NOT EXISTS idx_execution_nodes_asset ON execution_nodes(asset_id);
CREATE INDEX IF NOT EXISTS idx_execution_nodes_status ON execution_nodes(status);
