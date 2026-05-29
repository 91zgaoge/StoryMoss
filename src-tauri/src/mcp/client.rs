#![allow(dead_code)]
use uuid::Uuid;

use super::{
    transport::{StdioTransport, Transport},
    types::*,
};

pub struct McpClient {
    config: McpServerConfig,
    transport: Option<Box<dyn Transport + Send>>,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            transport: None,
            tools: Vec::new(),
            resources: Vec::new(),
        }
    }

    pub async fn connect(&mut self) -> Result<(), McpError> {
        let mut transport =
            StdioTransport::new(&self.config.command, &self.config.args, &self.config.env);

        transport.connect().await?;

        let init_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "cinema-ai",
                    "version": "2.0.0"
                }
            })),
        };

        transport.send(init_request).await?;
        let _response = transport.receive().await?;

        self.transport = Some(Box::new(transport));

        self.refresh_tools().await?;
        self.refresh_resources().await?;

        Ok(())
    }

    async fn refresh_tools(&mut self) -> Result<(), McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "tools/list".to_string(),
            params: None,
        };

        if let Some(ref mut transport) = self.transport {
            transport.send(request).await?;
            let response = transport.receive().await?;

            if let Some(result) = response.result {
                if let Ok(tools) = serde_json::from_value::<Vec<McpTool>>(result) {
                    self.tools = tools;
                }
            }
        }

        Ok(())
    }

    async fn refresh_resources(&mut self) -> Result<(), McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "resources/list".to_string(),
            params: None,
        };

        if let Some(ref mut transport) = self.transport {
            transport.send(request).await?;
            let response = transport.receive().await?;

            if let Some(result) = response.result {
                if let Ok(resources) = serde_json::from_value::<Vec<McpResource>>(result) {
                    self.resources = resources;
                }
            }
        }

        Ok(())
    }

    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            })),
        };

        if let Some(ref mut transport) = self.transport {
            transport.send(request).await?;
            let response = transport.receive().await?;

            if let Some(error) = response.error {
                return Err(McpError::RpcError(error.message));
            }

            return Ok(response.result.unwrap_or(serde_json::Value::Null));
        }

        Err(McpError::NotConnected)
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<serde_json::Value, McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "resources/read".to_string(),
            params: Some(serde_json::json!({
                "uri": uri
            })),
        };

        if let Some(ref mut transport) = self.transport {
            transport.send(request).await?;
            let response = transport.receive().await?;

            if let Some(error) = response.error {
                return Err(McpError::RpcError(error.message));
            }

            return Ok(response.result.unwrap_or(serde_json::Value::Null));
        }

        Err(McpError::NotConnected)
    }

    pub fn get_tools(&self) -> &Vec<McpTool> {
        &self.tools
    }

    pub fn get_resources(&self) -> &Vec<McpResource> {
        &self.resources
    }

    pub async fn disconnect(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            let _ = transport.close().await;
        }
    }
}
