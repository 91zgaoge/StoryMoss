#![allow(dead_code)]
#![allow(unused_imports)]
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
};

use tokio::{
    io::AsyncBufReadExt,
    process::{Child, Command},
};

use super::types::*;
use crate::error::AppError;

pub trait McpToolHandler: Send + Sync {
    fn handle(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>>;
}

/// Built-in tool: File System Operations
pub struct FileSystemTool;

impl McpToolHandler for FileSystemTool {
    fn handle(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let operation = arguments
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("read");
        let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");

        match operation {
            "read" => {
                let content = std::fs::read_to_string(path)?;
                Ok(serde_json::json!({ "content": content }))
            }
            "write" => {
                let content = arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                std::fs::write(path, content)?;
                Ok(serde_json::json!({ "success": true }))
            }
            "list" => {
                let entries: Vec<String> = std::fs::read_dir(path)?
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                Ok(serde_json::json!({ "entries": entries }))
            }
            _ => Err("Unknown operation".into()),
        }
    }
}

/// Built-in tool: Text Processing
pub struct TextProcessingTool;

impl McpToolHandler for TextProcessingTool {
    fn handle(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let operation = arguments
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("count");
        let text = arguments.get("text").and_then(|v| v.as_str()).unwrap_or("");

        match operation {
            "count" => {
                let chars = text.chars().count();
                let words = text.split_whitespace().count();
                let lines = text.lines().count();
                Ok(serde_json::json!({
                    "characters": chars,
                    "words": words,
                    "lines": lines
                }))
            }
            "split" => {
                let delimiter = arguments
                    .get("delimiter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("\n");
                let parts: Vec<String> = text.split(delimiter).map(|s| s.to_string()).collect();
                Ok(serde_json::json!({ "parts": parts }))
            }
            "replace" => {
                let from = arguments.get("from").and_then(|v| v.as_str()).unwrap_or("");
                let to = arguments.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let result = text.replace(from, to);
                Ok(serde_json::json!({ "result": result }))
            }
            _ => Err("Unknown operation".into()),
        }
    }
}

/// Built-in tool: Web Search (Simulated)
pub struct WebSearchTool;

impl McpToolHandler for WebSearchTool {
    fn handle(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Simulate search results
        Ok(serde_json::json!({
            "query": query,
            "results": [
                {
                    "title": format!("Search result for: {}", query),
                    "snippet": "This is a simulated search result...",
                    "url": "https://example.com/result1"
                }
            ],
            "note": "This is a simulated search. Connect to real search API for actual results."
        }))
    }
}

pub struct McpServer {
    config: McpServerConfig,
    tools: Arc<Mutex<HashMap<String, (McpTool, Box<dyn McpToolHandler>)>>>,
    child_process: Arc<Mutex<Option<Child>>>,
}

impl McpServer {
    pub fn new(config: McpServerConfig) -> Self {
        let server = Self {
            config,
            tools: Arc::new(Mutex::new(HashMap::new())),
            child_process: Arc::new(Mutex::new(None)),
        };

        // Register built-in tools
        server.register_built_in_tools();
        server
    }

    fn register_built_in_tools(&self) {
        // File System Tool
        self.register_tool(
            McpTool {
                name: "filesystem".to_string(),
                description: "File system operations (read, write, list)".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operation": { "type": "string", "enum": ["read", "write", "list"] },
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["operation", "path"]
                }),
            },
            Box::new(FileSystemTool),
        );

        // Text Processing Tool
        self.register_tool(
            McpTool {
                name: "text_processing".to_string(),
                description: "Text processing operations (count, split, replace)".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operation": { "type": "string", "enum": ["count", "split", "replace"] },
                        "text": { "type": "string" },
                        "delimiter": { "type": "string" },
                        "from": { "type": "string" },
                        "to": { "type": "string" }
                    },
                    "required": ["operation", "text"]
                }),
            },
            Box::new(TextProcessingTool),
        );

        // Web Search Tool
        self.register_tool(
            McpTool {
                name: "web_search".to_string(),
                description: "Search the web for information".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }),
            },
            Box::new(WebSearchTool),
        );
    }

    pub fn register_tool(&self, tool: McpTool, handler: Box<dyn McpToolHandler>) {
        self.tools
            .lock()
            .unwrap()
            .insert(tool.name.clone(), (tool, handler));
    }

    pub fn get_tools(&self) -> Vec<McpTool> {
        self.tools
            .lock()
            .unwrap()
            .values()
            .map(|(t, _)| t.clone())
            .collect()
    }

    pub fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let tools = self.tools.lock().unwrap();
        if let Some((_, handler)) = tools.get(tool_name) {
            handler
                .handle(arguments)
                .map_err(|e| McpError::RpcError(e.to_string()))
        } else {
            Err(McpError::RpcError(format!("Tool not found: {}", tool_name)))
        }
    }

    pub async fn start(&self) -> Result<(), McpError> {
        // Start external MCP server process if configured
        if !self.config.command.is_empty() {
            let mut cmd = Command::new(&self.config.command);
            cmd.args(&self.config.args)
                .envs(&self.config.env)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let child = cmd
                .spawn()
                .map_err(|e| McpError::TransportError(e.to_string()))?;

            *self.child_process.lock().unwrap() = Some(child);
        }

        log::info!("MCP Server started with {} tools", self.get_tools().len());
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), McpError> {
        if let Some(mut child) = self.child_process.lock().unwrap().take() {
            let _ = child.kill().await;
        }
        Ok(())
    }

    /// Execute tool with timeout
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let timeout = tokio::time::Duration::from_secs(self.config.timeout_seconds.max(30));

        // web_search 使用真实 HTTP 搜索
        if tool_name == "web_search" {
            return match tokio::time::timeout(timeout, async {
                perform_web_search(arguments).await
            })
            .await
            {
                Ok(Ok(result)) => Ok(result),
                Ok(Err(e)) => Err(McpError::RpcError(e.to_string())),
                Err(_) => Err(McpError::Timeout),
            };
        }

        match tokio::time::timeout(timeout, async {
            self.handle_tool_call(tool_name, arguments)
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err(McpError::Timeout),
        }
    }
}

/// 执行真实的网页搜索（使用 DuckDuckGo Lite）
async fn perform_web_search(arguments: serde_json::Value) -> Result<serde_json::Value, AppError> {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if query.is_empty() {
        return Ok(serde_json::json!({"query": "", "results": [], "note": "Empty query"}));
    }

    // 尝试 DuckDuckGo Lite
    let encoded_query = query.replace(' ', "+");
    let url = format!("https://lite.duckduckgo.com/lite/?q={}", encoded_query);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.0")
        .build()
        .map_err(AppError::from)?;

    match client.get(&url).send().await {
        Ok(response) => {
            if let Ok(html) = response.text().await {
                let results = parse_duckduckgo_results(&html);
                if !results.is_empty() {
                    return Ok(serde_json::json!({
                        "query": query,
                        "results": results,
                        "source": "duckduckgo"
                    }));
                }
            }
        }
        Err(e) => {
            log::warn!("[web_search] DuckDuckGo request failed: {}", e);
        }
    }

    // 回退到模拟数据
    log::info!(
        "[web_search] Falling back to simulated results for: {}",
        query
    );
    Ok(serde_json::json!({
        "query": query,
        "results": [
            {"title": format!("搜索结果: {}", query), "snippet": "未找到真实搜索结果，这是一个模拟结果。", "url": "https://example.com/result1"}
        ],
        "note": "真实搜索服务暂时不可用，已返回模拟数据。",
        "source": "simulated"
    }))
}

/// 简单解析 DuckDuckGo Lite HTML 结果
fn parse_duckduckgo_results(html: &str) -> Vec<serde_json::Value> {
    let mut results = Vec::new();
    // DuckDuckGo Lite 的结果在 .result-link 和 .result-snippet 中
    // 使用简单的字符串匹配提取
    let link_pattern = "class=\"result-link\"";
    let snippet_pattern = "class=\"result-snippet\"";

    let mut pos = 0;
    while let Some(link_start) = html[pos..].find(link_pattern) {
        let link_abs = pos + link_start;
        if let Some(href_start) = html[link_abs..].find("href=\"") {
            let href_abs = link_abs + href_start + 6;
            if let Some(href_end) = html[href_abs..].find("\"") {
                let href = &html[href_abs..href_abs + href_end];
                // 提取标题（在 > 和 < 之间）
                let title_start = href_abs + href_end + 1;
                let title_html = &html[title_start..title_start + 200];
                let title = if let Some(gt) = title_html.find('>') {
                    if let Some(lt) = title_html[gt..].find('<') {
                        title_html[gt + 1..gt + lt].trim().to_string()
                    } else {
                        href.to_string()
                    }
                } else {
                    href.to_string()
                };

                // 查找对应的 snippet
                let snippet = if let Some(snippet_start) = html[link_abs..].find(snippet_pattern) {
                    let snippet_abs = link_abs + snippet_start;
                    let snippet_html = &html[snippet_abs..snippet_abs + 300];
                    if let Some(gt) = snippet_html.find('>') {
                        if let Some(lt) = snippet_html[gt..].find('<') {
                            snippet_html[gt + 1..gt + lt].trim().to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                if !title.is_empty() && title != href {
                    results.push(serde_json::json!({
                        "title": title,
                        "snippet": if snippet.is_empty() { "无描述".to_string() } else { snippet },
                        "url": if href.starts_with("http") { href.to_string() } else { format!("https://duckduckgo.com{}", href) }
                    }));
                }

                if results.len() >= 5 {
                    break;
                }
            }
        }
        pos = link_abs + link_pattern.len();
    }

    results
}
