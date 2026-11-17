UPDATE "LiteLLM_UserTable"
SET user_email = $2,
    updated_at = CURRENT_TIMESTAMP
WHERE user_id = $1
  AND user_email IS NULL
