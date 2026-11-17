SELECT user_id,
       organization_id,
       user_role,
       spend,
       budget_id,
       created_at,
       updated_at
FROM "LiteLLM_OrganizationMembership"
WHERE user_id = $1
ORDER BY organization_id
