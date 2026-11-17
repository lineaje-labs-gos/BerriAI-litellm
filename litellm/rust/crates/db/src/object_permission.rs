//! Reads one `LiteLLM_ObjectPermissionTable` row, the grants keys, teams, users, projects and end users point at.

use sqlx::PgExecutor;

use crate::{Error, RawJson};

/// One `LiteLLM_ObjectPermissionTable` row.
#[macro_rules_attribute::apply(crate::db_row)]
#[derive(serde::Serialize)]
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

pub async fn find_object_permission(
    executor: impl PgExecutor<'_>,
    object_permission_id: &str,
) -> Result<Option<ObjectPermission>, Error> {
    Ok(sqlx::query_file_as!(
        ObjectPermission,
        "queries/object_permission/find_object_permission.sql",
        object_permission_id,
    )
    .fetch_optional(executor)
    .await?)
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use rstest::rstest;
    use sqlx::PgPool;

    use super::find_object_permission;
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
            INSERT INTO "LiteLLM_ObjectPermissionTable"
                (object_permission_id, mcp_servers, mcp_access_groups, mcp_tool_permissions, vector_stores,
                 agents, agent_access_groups, models, blocked_tools, mcp_toolsets, search_tools,
                 mcp_tool_search_enabled, skills)
            VALUES ('perm-full', '{s1,s2}', '{g1}', '{"s1": ["t1"]}', '{v1}', '{a1}', '{ag1}', '{m1}', '{bt}',
                    '{ts}', '{st}', true, '{sk}')
            "#,
        )
        .execute(&mut *tx)
        .await?;

        let found = find_object_permission(&mut *tx, "perm-full")
            .await?
            .ok_or("row missing")?;

        assert_eq!(found.object_permission_id, "perm-full");
        assert_eq!(found.mcp_servers, names(&["s1", "s2"]));
        assert_eq!(found.mcp_access_groups, names(&["g1"]));
        assert_eq!(
            found.mcp_tool_permissions.as_ref().map(RawJson::get),
            Some(r#"{"s1": ["t1"]}"#)
        );
        assert_eq!(found.vector_stores, names(&["v1"]));
        assert_eq!(found.agents, names(&["a1"]));
        assert_eq!(found.agent_access_groups, names(&["ag1"]));
        assert_eq!(found.models, names(&["m1"]));
        assert_eq!(found.blocked_tools, names(&["bt"]));
        assert_eq!(found.mcp_toolsets, names(&["ts"]));
        assert_eq!(found.search_tools, names(&["st"]));
        assert_eq!(found.mcp_tool_search_enabled, Some(true));
        assert_eq!(found.skills, names(&["sk"]));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn null_lists_stay_absent(
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
            INSERT INTO "LiteLLM_ObjectPermissionTable" (object_permission_id, mcp_servers, models, skills)
            VALUES ('perm-null', NULL, NULL, NULL)
            "#,
        )
        .execute(&mut *tx)
        .await?;

        let found = find_object_permission(&mut *tx, "perm-null")
            .await?
            .ok_or("row missing")?;

        assert_eq!(
            (found.mcp_servers, found.models, found.skills),
            (None, None, None)
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_row_is_none(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        assert!(
            find_object_permission(&pool, "perm-missing")
                .await?
                .is_none()
        );
        Ok(())
    }
}
