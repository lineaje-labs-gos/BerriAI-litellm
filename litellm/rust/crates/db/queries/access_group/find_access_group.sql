SELECT access_group_id,
       access_group_name,
       description,
       access_model_names,
       access_mcp_server_ids,
       mcp_tool_permissions AS "mcp_tool_permissions: RawJson",
       access_agent_ids,
       access_skill_names,
       access_vector_store_ids,
       assigned_team_ids,
       assigned_key_ids,
       created_at,
       created_by,
       updated_at,
       updated_by
FROM "LiteLLM_AccessGroupTable"
WHERE access_group_id = $1
