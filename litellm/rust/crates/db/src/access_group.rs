//! Reads one `LiteLLM_AccessGroupTable` row, the access object that bundles models, MCP servers, agents,
//! skills and vector stores and assigns them to teams and keys.

use chrono::NaiveDateTime;
use sqlx::PgExecutor;

use crate::{Error, RawJson};

/// One `LiteLLM_AccessGroupTable` row.
#[macro_rules_attribute::apply(crate::db_row)]
#[derive(serde::Serialize)]
pub struct AccessGroup {
    pub access_group_id: String,
    pub access_group_name: String,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_model_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_mcp_server_ids: Option<Vec<String>>,
    pub mcp_tool_permissions: Option<RawJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_agent_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_skill_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_vector_store_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_team_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_key_ids: Option<Vec<String>>,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub updated_at: NaiveDateTime,
    pub updated_by: Option<String>,
}

pub async fn find_access_group(
    executor: impl PgExecutor<'_>,
    access_group_id: &str,
) -> Result<Option<AccessGroup>, Error> {
    Ok(sqlx::query_file_as!(
        AccessGroup,
        "queries/access_group/find_access_group.sql",
        access_group_id,
    )
    .fetch_optional(executor)
    .await?)
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use rstest::rstest;
    use sqlx::PgPool;

    use super::find_access_group;
    use crate::{RawJson, test_support::database_pool};

    fn names(values: &[&str]) -> Option<Vec<String>> {
        Some(values.iter().map(|value| (*value).to_owned()).collect())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_every_column(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        sqlx::query(
            r#"
            INSERT INTO "LiteLLM_AccessGroupTable"
                (access_group_id, access_group_name, description, access_model_names, access_mcp_server_ids,
                 mcp_tool_permissions, access_agent_ids, access_skill_names, access_vector_store_ids,
                 assigned_team_ids, assigned_key_ids, created_at, created_by, updated_at, updated_by)
            VALUES ('ag-full', 'group-full', 'desc', '{m1}', '{s1}', '{"s1": ["t1"]}', '{a1}', '{sk1}', '{v1}',
                    '{team1}', '{key1}', '2026-01-02 03:04:05.678', 'creator', '2026-02-03 04:05:06.789', NULL)
            "#,
        )
        .execute(&mut *tx)
        .await?;

        let found = find_access_group(&mut *tx, "ag-full")
            .await?
            .ok_or("row missing")?;

        assert_eq!(found.access_group_id, "ag-full");
        assert_eq!(found.access_group_name, "group-full");
        assert_eq!(found.description.as_deref(), Some("desc"));
        assert_eq!(found.access_model_names, names(&["m1"]));
        assert_eq!(found.access_mcp_server_ids, names(&["s1"]));
        assert_eq!(
            found.mcp_tool_permissions.as_ref().map(RawJson::get),
            Some(r#"{"s1": ["t1"]}"#)
        );
        assert_eq!(found.access_agent_ids, names(&["a1"]));
        assert_eq!(found.access_skill_names, names(&["sk1"]));
        assert_eq!(found.access_vector_store_ids, names(&["v1"]));
        assert_eq!(found.assigned_team_ids, names(&["team1"]));
        assert_eq!(found.assigned_key_ids, names(&["key1"]));
        assert_eq!(found.created_at.to_string(), "2026-01-02 03:04:05.678");
        assert_eq!(found.created_by.as_deref(), Some("creator"));
        assert_eq!(found.updated_at.to_string(), "2026-02-03 04:05:06.789");
        assert_eq!(found.updated_by, None);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_row_is_none(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        assert!(find_access_group(&pool, "ag-missing").await?.is_none());
        Ok(())
    }
}
