SELECT object_permission_id,
       mcp_servers,
       mcp_access_groups,
       mcp_tool_permissions AS "mcp_tool_permissions: RawJson",
       vector_stores,
       agents,
       agent_access_groups,
       models,
       blocked_tools,
       mcp_toolsets,
       search_tools,
       mcp_tool_search_enabled,
       skills
FROM "LiteLLM_ObjectPermissionTable"
WHERE object_permission_id = $1
