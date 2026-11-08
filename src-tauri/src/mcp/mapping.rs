//! MCP 映射层：外部服务器 schema/权限 → StoryMoss 工具面契约。
//!
//! 规则在 `docs/plans/2026-11-08-mcp-mapping-contract.md` 定稿：
//! - 协议版本白名单不通过时连接直接失败，不静默退回空工具列表；
//! - 源系统版本（当前仅 Legado 0.14.x）超出契约时返回
//!   `MappingUnsupported`，要求新增映射而不是悄悄兼容；
//! - 工具字段漂移按工具隔离，查询该工具时返回 `SchemaDrift`；
//! - 源系统权限变化只撤销对应权限的工具，不使整台服务器失效；
//!   查询被撤销权限的工具返回 `PermissionDenied` / `PermissionChanged`。

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use super::types::{McpError, McpPermission, McpResource, McpServerMeta, McpTool};

/// 映射契约版本；字段别名或权限面变化时递增，并同步更新契约文档。
pub const MCP_MAPPING_VERSION: &str = "1.0.0";

/// 已验收的 MCP 协议版本白名单。
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];

/// Legado（栗子读书）映射契约的基础只读工具。
pub const LEGADO_REQUIRED_TOOLS: &[&str] = &[
    "list_books",
    "get_book",
    "list_chapters",
    "get_chapter_content",
    "get_reading_progress",
    "list_bookmarks",
    "list_highlights",
    "list_groups",
    "get_reading_stats",
    "list_book_sources",
];

/// Legado 契约的已知可选工具；缺失时视为对应权限已撤销。
pub const LEGADO_OPTIONAL_TOOLS: &[(&str, McpPermission)] =
    &[("search_online", McpPermission::Network)];

#[derive(Debug, Clone, Serialize)]
pub struct MappingContract {
    pub mapping_id: String,
    pub mapping_version: String,
    pub source: String,
    pub source_version: String,
    pub protocol_version: String,
    pub required_tools: Vec<String>,
    pub optional_tools: Vec<(String, McpPermission)>,
}

impl MappingContract {
    pub fn has_optional_tool(&self, tool: &str) -> Option<McpPermission> {
        self.optional_tools
            .iter()
            .find(|(name, _)| name == tool)
            .map(|(_, permission)| *permission)
    }
}

/// 工具面解析结果：未漂移的工具进入可用列表，漂移工具单独记录。
#[derive(Debug, Clone, Default)]
pub struct ParsedToolList {
    pub tools: Vec<McpTool>,
    pub drifted: HashMap<String, Vec<String>>,
}

/// 权限快照：`granted=false` 表示源系统当前关闭该权限；
/// `tools` 保留本次看到的工具名，用于错误消息与诊断。
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PermissionState {
    pub granted: bool,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct MappingSnapshot {
    pub mapping_id: String,
    pub mapping_version: String,
    pub protocol_version: String,
    pub source_name: String,
    pub source_version: String,
    pub tool_names: HashSet<String>,
    pub tool_permissions: HashMap<String, McpPermission>,
    pub permissions: BTreeMap<McpPermission, PermissionState>,
    pub drifted_tools: HashMap<String, Vec<String>>,
}

impl MappingSnapshot {
    pub fn permission_for(&self, tool: &str) -> Option<McpPermission> {
        self.tool_permissions.get(tool).copied()
    }

    /// 上次快照有、当前快照没有的工具（权限快照变化的证据）。
    pub fn removed_tools(&self, previous: &MappingSnapshot) -> Vec<String> {
        previous
            .tool_names
            .iter()
            .filter(|tool| !self.tool_names.contains(*tool))
            .cloned()
            .collect()
    }
}

/// 解析 initialize 的源信息并选出映射契约。
pub fn resolve_contract(meta: &McpServerMeta) -> Result<MappingContract, McpError> {
    let protocol = meta.protocol_version.trim();
    if !protocol.is_empty()
        && !SUPPORTED_PROTOCOL_VERSIONS
            .iter()
            .any(|supported| *supported == protocol)
    {
        return Err(McpError::ProtocolUnsupported {
            requested: protocol.to_string(),
            supported: SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .map(|v| (*v).to_string())
                .collect(),
        });
    }

    let source_name = meta.server_info.name.trim().to_string();
    let source_title = meta
        .server_info
        .title
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let normalized_name = source_name.to_ascii_lowercase();
    let normalized_title = source_title.to_ascii_lowercase();
    let is_legado = normalized_name.contains("legado") || normalized_title.contains("栗子");

    if is_legado {
        if protocol.is_empty() {
            return Err(McpError::MappingInvalid {
                reason: "Legado initialize 缺少 protocolVersion，无法套用映射契约".into(),
            });
        }
        let version = meta.server_info.version.trim().to_string();
        let major_minor =
            parse_major_minor(&version).ok_or_else(|| McpError::MappingUnsupported {
                source: if source_title.is_empty() {
                    source_name.clone()
                } else {
                    format!("{source_title}（{source_name}）")
                },
                version: version.clone(),
                mapping_version: MCP_MAPPING_VERSION.to_string(),
                supported: "0.14.x".to_string(),
            })?;
        if major_minor != (0, 14) {
            return Err(McpError::MappingUnsupported {
                source: if source_title.is_empty() {
                    source_name.clone()
                } else {
                    format!("{source_title}（{source_name}）")
                },
                version,
                mapping_version: MCP_MAPPING_VERSION.to_string(),
                supported: "0.14.x".to_string(),
            });
        }
        Ok(legado_contract(protocol.to_string(), version))
    } else {
        Ok(generic_contract(protocol.to_string(), source_name))
    }
}

fn legado_contract(protocol: String, source_version: String) -> MappingContract {
    MappingContract {
        mapping_id: "legado-macos".to_string(),
        mapping_version: MCP_MAPPING_VERSION.to_string(),
        source: "legado-macos".to_string(),
        source_version,
        protocol_version: protocol,
        required_tools: LEGADO_REQUIRED_TOOLS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        optional_tools: LEGADO_OPTIONAL_TOOLS
            .iter()
            .map(|(name, permission)| ((*name).to_string(), *permission))
            .collect(),
    }
}

fn generic_contract(protocol: String, source: String) -> MappingContract {
    MappingContract {
        mapping_id: "mcp-standard".to_string(),
        mapping_version: MCP_MAPPING_VERSION.to_string(),
        source,
        source_version: String::new(),
        protocol_version: protocol,
        required_tools: Vec::new(),
        optional_tools: Vec::new(),
    }
}

fn parse_major_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor))
}

/// 解析 tools/list 结果。兼容标准 `{ tools: [...] }` 与直接数组；
/// 单项漂移不静默吞掉整批工具，全部漂移才让连接失败。
pub fn parse_tools_payload(result: &serde_json::Value) -> Result<ParsedToolList, McpError> {
    let list_value = result
        .get("tools")
        .cloned()
        .unwrap_or_else(|| result.clone());

    if list_value.is_null() {
        return Ok(ParsedToolList::default());
    }
    let items = list_value
        .as_array()
        .ok_or_else(|| McpError::MappingInvalid {
            reason: "tools/list 结果既不是 {tools:[...]}，也不是数组".to_string(),
        })?;

    let mut parsed = ParsedToolList::default();
    for (index, item) in items.iter().enumerate() {
        let name_hint = item
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{index}"));

        match serde_json::from_value::<McpTool>(item.clone()) {
            Ok(tool) => {
                let mut missing = Vec::new();
                if tool.name.trim().is_empty() {
                    missing.push("name".to_string());
                }
                if !tool.parameters.is_object() {
                    // 标准字段是 inputSchema；内部字段是 parameters。
                    missing.push("inputSchema/parameters".to_string());
                }
                if missing.is_empty() {
                    parsed.tools.push(tool);
                } else {
                    parsed.drifted.insert(name_hint, missing);
                }
            }
            Err(error) => {
                parsed
                    .drifted
                    .insert(name_hint, vec![format!("字段解析失败: {error}")]);
            }
        }
    }

    if parsed.tools.is_empty() && !items.is_empty() {
        let details = parsed
            .drifted
            .iter()
            .map(|(tool, missing)| format!("{tool}: {}", missing.join(" + ")))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(McpError::SchemaDrift {
            tool: "*".to_string(),
            missing: vec![format!("tools/list 全部工具字段漂移: {details}")],
        });
    }

    Ok(parsed)
}

/// 解析 resources/list 结果；字段名通过 `mimeType` alias 兼容。资源结构损坏时
/// 返回映射错误而不是空列表。
pub fn parse_resources_payload(result: &serde_json::Value) -> Result<Vec<McpResource>, McpError> {
    let list_value = result
        .get("resources")
        .cloned()
        .unwrap_or_else(|| result.clone());

    if list_value.is_null() {
        return Ok(Vec::new());
    }
    let items = list_value
        .as_array()
        .ok_or_else(|| McpError::MappingInvalid {
            reason: "resources/list 结果既不是 {resources:[...]}，也不是数组".to_string(),
        })?;

    let mut resources = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let resource = serde_json::from_value::<McpResource>(item.clone()).map_err(|error| {
            McpError::MappingInvalid {
                reason: format!("resources/list 第 {index} 项字段漂移: {error}"),
            }
        })?;
        resources.push(resource);
    }
    Ok(resources)
}

/// 根据工具 annotations 推断权限类别；没有注解时按只读处理。
pub fn permission_for_tool(tool: &McpTool) -> McpPermission {
    if let Some(annotations) = tool.annotations.as_ref() {
        if annotations.is_write() {
            return McpPermission::Write;
        }
        if annotations.is_network() {
            return McpPermission::Network;
        }
    }
    McpPermission::Read
}

/// 构建工具面映射快照。基础工具缺失是整机映射失效；权限工具缺失只记撤销。
pub fn build_snapshot(
    contract: &MappingContract,
    meta: &McpServerMeta,
    tools: &[McpTool],
    drifted_tools: HashMap<String, Vec<String>>,
) -> Result<MappingSnapshot, McpError> {
    let mut tool_names = HashSet::new();
    let mut tool_permissions = HashMap::new();
    let mut by_permission: BTreeMap<McpPermission, Vec<String>> = BTreeMap::new();

    for tool in tools {
        if tool.name.trim().is_empty() {
            continue;
        }
        let permission = permission_for_tool(tool);
        tool_names.insert(tool.name.clone());
        tool_permissions.insert(tool.name.clone(), permission);
        by_permission
            .entry(permission)
            .or_default()
            .push(tool.name.clone());
    }

    let missing: Vec<String> = contract
        .required_tools
        .iter()
        .filter(|tool| !tool_names.contains(*tool))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(McpError::MappingInvalid {
            reason: format!(
                "映射 {} 缺少基础工具: {}；源版本 {}",
                contract.mapping_id,
                missing.join(", "),
                meta.server_info.version
            ),
        });
    }

    let mut permissions = BTreeMap::new();
    for permission in [
        McpPermission::Read,
        McpPermission::Write,
        McpPermission::Network,
        McpPermission::Private,
    ] {
        let tools_for_permission = by_permission.get(&permission).cloned().unwrap_or_default();
        let expected_optional = contract
            .optional_tools
            .iter()
            .any(|(_, expected)| *expected == permission);
        let granted = match permission {
            McpPermission::Read => true,
            // 写/隐私权限没有基础工具也不视为契约损坏；只是当前未开启。
            McpPermission::Write | McpPermission::Private => !tools_for_permission.is_empty(),
            // 联网权限若契约声明的可选工具全缺席，即视为源系统已撤销联网。
            McpPermission::Network => !tools_for_permission.is_empty() || !expected_optional,
        };
        permissions.insert(
            permission,
            PermissionState {
                granted,
                tools: tools_for_permission,
            },
        );
    }

    Ok(MappingSnapshot {
        mapping_id: contract.mapping_id.clone(),
        mapping_version: contract.mapping_version.clone(),
        protocol_version: meta.protocol_version.clone(),
        source_name: meta.server_info.name.clone(),
        source_version: meta.server_info.version.clone(),
        tool_names,
        tool_permissions,
        permissions,
        drifted_tools,
    })
}

/// 查询前检查工具可用性；这是映射层对“查询怎么报错”的唯一入口。
pub fn check_tool_available(
    snapshot: &MappingSnapshot,
    contract: &MappingContract,
    tool: &str,
) -> Result<(), McpError> {
    if let Some(missing) = snapshot.drifted_tools.get(tool) {
        return Err(McpError::SchemaDrift {
            tool: tool.to_string(),
            missing: missing.clone(),
        });
    }

    if snapshot.tool_names.contains(tool) {
        if let Some(permission) = snapshot.permission_for(tool) {
            if !snapshot
                .permissions
                .get(&permission)
                .map(|state| state.granted)
                .unwrap_or(false)
            {
                return Err(McpError::PermissionDenied {
                    permission,
                    tool: tool.to_string(),
                });
            }
        }
        return Ok(());
    }

    if let Some(permission) = contract.has_optional_tool(tool) {
        if !snapshot
            .permissions
            .get(&permission)
            .map(|state| state.granted)
            .unwrap_or(false)
        {
            return Err(McpError::PermissionDenied {
                permission,
                tool: tool.to_string(),
            });
        }
    }

    Err(McpError::ToolUnavailable {
        tool: tool.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{super::types::McpToolAnnotations, *};

    fn meta(name: &str, title: &str, version: &str, protocol: &str) -> McpServerMeta {
        McpServerMeta {
            protocol_version: protocol.to_string(),
            server_info: super::super::types::McpServerInfo {
                name: name.to_string(),
                title: Some(title.to_string()),
                version: version.to_string(),
            },
            instructions: None,
        }
    }

    fn legado_tool(name: &str, annotations: Option<McpToolAnnotations>) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({"type": "object"}),
            annotations,
        }
    }

    #[test]
    fn parses_standard_input_schema_and_annotations() {
        let payload = serde_json::json!({
            "tools": [{
                "name": "search_online",
                "description": "在线搜索",
                "inputSchema": {
                    "type": "object",
                    "properties": { "keyword": { "type": "string" } },
                    "required": ["keyword"]
                },
                "annotations": {
                    "readOnlyHint": true,
                    "openWorldHint": true
                }
            }]
        });
        let parsed = parse_tools_payload(&payload).unwrap();
        assert_eq!(parsed.tools.len(), 1);
        assert_eq!(
            parsed.tools[0].parameters["properties"]["keyword"]["type"],
            "string"
        );
        assert_eq!(
            permission_for_tool(&parsed.tools[0]),
            McpPermission::Network
        );
    }

    #[test]
    fn detects_schema_drift_and_keeps_valid_tools() {
        let payload = serde_json::json!({
            "tools": [
                { "name": "list_books", "inputSchema": { "type": "object" } },
                { "name": "broken_tool" }
            ]
        });
        let parsed = parse_tools_payload(&payload).unwrap();
        assert_eq!(parsed.tools.len(), 1);
        assert!(parsed.drifted.contains_key("broken_tool"));
    }

    #[test]
    fn all_tools_drifting_fails_instead_of_empty_success() {
        let payload = serde_json::json!({
            "tools": [{ "name": "broken_a" }, { "name": "broken_b" }]
        });
        let error = parse_tools_payload(&payload).unwrap_err();
        assert!(matches!(error, McpError::SchemaDrift { .. }));
    }

    #[test]
    fn resolves_legado_contract_and_rejects_unsupported_version() {
        let ok = resolve_contract(&meta("legado-macos", "栗子读书", "0.14.24", "2025-06-18"));
        assert!(ok.is_ok());
        assert_eq!(ok.unwrap().mapping_id, "legado-macos");

        let unsupported =
            resolve_contract(&meta("legado-macos", "栗子读书", "0.15.0", "2025-06-18"));
        assert!(matches!(
            unsupported.unwrap_err(),
            McpError::MappingUnsupported { .. }
        ));

        let protocol = resolve_contract(&meta("legado-macos", "栗子读书", "0.14.24", "2026-01-01"));
        assert!(matches!(
            protocol.unwrap_err(),
            McpError::ProtocolUnsupported { .. }
        ));
    }

    #[test]
    fn revoked_network_tool_reports_permission_denied() {
        let contract = legado_contract("2025-06-18".to_string(), "0.14.24".to_string());
        let tools: Vec<McpTool> = LEGADO_REQUIRED_TOOLS
            .iter()
            .map(|name| legado_tool(name, None))
            .collect();
        let snapshot = build_snapshot(
            &contract,
            &meta("legado-macos", "栗子读书", "0.14.24", "2025-06-18"),
            &tools,
            HashMap::new(),
        )
        .unwrap();
        assert!(!snapshot.permissions[&McpPermission::Network].granted);
        let error = check_tool_available(&snapshot, &contract, "search_online").unwrap_err();
        assert!(matches!(
            error,
            McpError::PermissionDenied {
                permission: McpPermission::Network,
                ..
            }
        ));
    }

    #[test]
    fn missing_required_tool_invalidates_mapping() {
        let contract = legado_contract("2025-06-18".to_string(), "0.14.24".to_string());
        let tools: Vec<McpTool> = LEGADO_REQUIRED_TOOLS
            .iter()
            .filter(|name| **name != "list_books")
            .map(|name| legado_tool(name, None))
            .collect();
        let error = build_snapshot(
            &contract,
            &meta("legado-macos", "栗子读书", "0.14.24", "2025-06-18"),
            &tools,
            HashMap::new(),
        )
        .unwrap_err();
        assert!(matches!(error, McpError::MappingInvalid { .. }));
    }

    #[test]
    fn resources_use_standard_mime_type_alias() {
        let payload = serde_json::json!({
            "resources": [{
                "uri": "legado://shelf",
                "name": "书架",
                "mimeType": "application/json",
                "description": "当前书架"
            }]
        });
        let resources = parse_resources_payload(&payload).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].mime_type, "application/json");
    }
}
