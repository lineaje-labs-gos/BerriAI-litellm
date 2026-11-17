//! `LiteLLM_ObjectPermissionTable` rows, joined into the objects that carry an object permission

use serde::Serialize;

use crate::RawJson;

/// Every scalar column of one `LiteLLM_ObjectPermissionTable` row
#[derive(Clone, Debug, Serialize)]
pub struct ObjectPermission {
    pub object_permission_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_access_groups: Option<Vec<String>>,
    pub mcp_tool_permissions: Option<RawJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_stores: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_access_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_toolsets: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_tools: Option<Vec<String>>,
    pub mcp_tool_search_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
}
