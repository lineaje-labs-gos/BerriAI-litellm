//! `LiteLLM_EndUserTable` reads: the spend-path budget lookup, the whole row for the end user store, and
//! the ids of end users that restrict something auth enforces

use serde::Serialize;
use sqlx::{PgExecutor, types::chrono::NaiveDateTime};

use crate::{Budget, Error, ObjectPermission, RawJson};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RateLimits {
    pub rpm: Option<i64>,
    pub tpm: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EndUser {
    pub spend: Option<f64>,
    pub blocked: bool,
    pub max_budget: Option<f64>,
    pub limits: RateLimits,
}

#[macro_rules_attribute::apply(crate::db_row)]
struct EndUserRow {
    spend: Option<f64>,
    blocked: Option<bool>,
    max_budget: Option<f64>,
    rpm_limit: Option<i64>,
    tpm_limit: Option<i64>,
}

pub async fn find_end_user(
    executor: impl PgExecutor<'_>,
    end_user_id: &str,
    default_budget_id: Option<&str>,
    key_budget_id: Option<&str>,
) -> Result<EndUser, Error> {
    let row = sqlx::query_file_as!(
        EndUserRow,
        "queries/end_user/find_end_user.sql",
        end_user_id,
        default_budget_id,
        key_budget_id,
    )
    .fetch_one(executor)
    .await?;
    Ok(EndUser {
        spend: row.spend,
        blocked: row.blocked.unwrap_or(false),
        max_budget: row.max_budget,
        limits: RateLimits {
            rpm: row.rpm_limit,
            tpm: row.tpm_limit,
        },
    })
}

/// One `LiteLLM_EndUserTable` row with the budget and object permission it links, as the end user store hands it out
#[derive(Clone, Debug, Serialize)]
pub struct EndUserRecord {
    pub user_id: String,
    pub alias: Option<String>,
    pub spend: f64,
    pub allowed_model_region: Option<String>,
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    pub budget_id: Option<String>,
    pub litellm_budget_table: Option<Budget>,
    pub object_permission_id: Option<String>,
    pub object_permission: Option<ObjectPermission>,
    pub blocked: bool,
}

struct EndUserRecordRow {
    user_id: String,
    alias: Option<String>,
    spend: f64,
    allowed_model_region: Option<String>,
    default_model: Option<String>,
    models: Option<Vec<String>>,
    budget_id: Option<String>,
    object_permission_id: Option<String>,
    blocked: bool,
    budget_budget_id: Option<String>,
    budget_name: Option<String>,
    budget_max_budget: Option<f64>,
    budget_soft_budget: Option<f64>,
    budget_max_parallel_requests: Option<i32>,
    budget_tpm_limit: Option<i64>,
    budget_rpm_limit: Option<i64>,
    budget_tpd_limit: Option<i64>,
    budget_model_max_budget: Option<RawJson>,
    budget_model_tpm_limit: Option<RawJson>,
    budget_model_rpm_limit: Option<RawJson>,
    budget_budget_duration: Option<String>,
    budget_budget_limits: Option<RawJson>,
    budget_cap_policy: Option<String>,
    budget_budget_reset_at: Option<NaiveDateTime>,
    budget_budget_reset_count: Option<i32>,
    budget_allowed_models: Option<Vec<String>>,
    budget_temp_budget_increase: Option<f64>,
    budget_temp_budget_expiry: Option<NaiveDateTime>,
    budget_created_at: Option<NaiveDateTime>,
    budget_created_by: Option<String>,
    budget_updated_at: Option<NaiveDateTime>,
    budget_updated_by: Option<String>,
    permission_object_permission_id: Option<String>,
    permission_mcp_servers: Option<Vec<String>>,
    permission_mcp_access_groups: Option<Vec<String>>,
    permission_mcp_tool_permissions: Option<RawJson>,
    permission_vector_stores: Option<Vec<String>>,
    permission_agents: Option<Vec<String>>,
    permission_agent_access_groups: Option<Vec<String>>,
    permission_models: Option<Vec<String>>,
    permission_blocked_tools: Option<Vec<String>>,
    permission_mcp_toolsets: Option<Vec<String>>,
    permission_search_tools: Option<Vec<String>>,
    permission_mcp_tool_search_enabled: Option<bool>,
    permission_skills: Option<Vec<String>>,
}

impl EndUserRecordRow {
    fn budget(&mut self) -> Option<Budget> {
        Some(Budget {
            budget_id: self.budget_budget_id.take()?,
            name: self.budget_name.take(),
            max_budget: self.budget_max_budget,
            soft_budget: self.budget_soft_budget,
            max_parallel_requests: self.budget_max_parallel_requests,
            tpm_limit: self.budget_tpm_limit,
            rpm_limit: self.budget_rpm_limit,
            tpd_limit: self.budget_tpd_limit,
            model_max_budget: self.budget_model_max_budget.take(),
            model_tpm_limit: self.budget_model_tpm_limit.take(),
            model_rpm_limit: self.budget_model_rpm_limit.take(),
            budget_duration: self.budget_budget_duration.take(),
            budget_limits: self.budget_budget_limits.take(),
            cap_policy: self.budget_cap_policy.take(),
            budget_reset_at: self.budget_budget_reset_at,
            budget_reset_count: self.budget_budget_reset_count?,
            allowed_models: self.budget_allowed_models.take(),
            temp_budget_increase: self.budget_temp_budget_increase,
            temp_budget_expiry: self.budget_temp_budget_expiry,
            created_at: self.budget_created_at?,
            created_by: self.budget_created_by.take()?,
            updated_at: self.budget_updated_at?,
            updated_by: self.budget_updated_by.take()?,
        })
    }

    fn object_permission(&mut self) -> Option<ObjectPermission> {
        Some(ObjectPermission {
            object_permission_id: self.permission_object_permission_id.take()?,
            mcp_servers: self.permission_mcp_servers.take(),
            mcp_access_groups: self.permission_mcp_access_groups.take(),
            mcp_tool_permissions: self.permission_mcp_tool_permissions.take(),
            vector_stores: self.permission_vector_stores.take(),
            agents: self.permission_agents.take(),
            agent_access_groups: self.permission_agent_access_groups.take(),
            models: self.permission_models.take(),
            blocked_tools: self.permission_blocked_tools.take(),
            mcp_toolsets: self.permission_mcp_toolsets.take(),
            search_tools: self.permission_search_tools.take(),
            mcp_tool_search_enabled: self.permission_mcp_tool_search_enabled,
            skills: self.permission_skills.take(),
        })
    }

    fn into_record(mut self) -> EndUserRecord {
        let litellm_budget_table = self.budget();
        let object_permission = self.object_permission();
        EndUserRecord {
            user_id: self.user_id,
            alias: self.alias,
            spend: self.spend,
            allowed_model_region: self.allowed_model_region,
            default_model: self.default_model,
            models: self.models,
            budget_id: self.budget_id,
            litellm_budget_table,
            object_permission_id: self.object_permission_id,
            object_permission,
            blocked: self.blocked,
        }
    }
}

/// The end user `end_user_id` with its linked budget and object permission, or `None` when no row exists
pub async fn find_end_user_record(
    executor: impl PgExecutor<'_>,
    end_user_id: &str,
) -> Result<Option<EndUserRecord>, Error> {
    let row = sqlx::query_file_as!(
        EndUserRecordRow,
        "queries/end_user/find_end_user_row.sql",
        end_user_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(EndUserRecordRow::into_record))
}

/// Ids of end users whose row restricts something auth enforces, at most `limit` of them
pub async fn find_restricted_end_user_ids(
    executor: impl PgExecutor<'_>,
    limit: i64,
) -> Result<Vec<String>, Error> {
    Ok(
        sqlx::query_file_scalar!("queries/end_user/find_restricted_end_user_ids.sql", limit)
            .fetch_all(executor)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{EndUser, RateLimits, find_end_user};
    use crate::test_support::database_pool;
    use rstest::rstest;
    use sqlx::{PgPool, Postgres, Transaction};

    #[derive(Clone, Copy)]
    struct BudgetFixture {
        budget_id: &'static str,
        max_budget: f64,
        rpm_limit: i64,
        tpm_limit: i64,
    }

    async fn insert_budget(
        tx: &mut Transaction<'_, Postgres>,
        budget: BudgetFixture,
    ) -> Result<(), sqlx::Error> {
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let result = sqlx::query(
            r#"
            INSERT INTO "LiteLLM_BudgetTable"
                (budget_id, max_budget, rpm_limit, tpm_limit, created_by, updated_by)
            VALUES ($1, $2, $3, $4, 'db-query-test', 'db-query-test')
            "#,
        )
        .bind(budget.budget_id)
        .bind(budget.max_budget)
        .bind(budget.rpm_limit)
        .bind(budget.tpm_limit)
        .execute(&mut **tx)
        .await;
        result.map(|_| ())
    }

    async fn insert_end_user(
        tx: &mut Transaction<'_, Postgres>,
        user_id: &str,
        spend: f64,
        blocked: bool,
        budget_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let result = sqlx::query(
            r#"
            INSERT INTO "LiteLLM_EndUserTable" (user_id, spend, blocked, budget_id)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(user_id)
        .bind(spend)
        .bind(blocked)
        .bind(budget_id)
        .execute(&mut **tx)
        .await;
        result.map(|_| ())
    }

    #[rstest]
    #[case::without_default_budget(None)]
    #[case::with_default_budget(Some(BudgetFixture {
        budget_id: "missing-user-default-budget",
        max_budget: 11.5,
        rpm_limit: 12,
        tpm_limit: 1300,
    }))]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_end_user_maps_to_defaults(
        #[case] default_budget: Option<BudgetFixture>,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        if let Some(budget) = default_budget {
            insert_budget(&mut tx, budget).await?;
        }

        let actual = find_end_user(
            &mut *tx,
            "missing-end-user",
            default_budget.map(|budget| budget.budget_id),
            None,
        )
        .await?;

        assert_eq!(
            actual,
            EndUser {
                spend: None,
                blocked: false,
                max_budget: default_budget.map(|budget| budget.max_budget),
                limits: RateLimits {
                    rpm: default_budget.map(|budget| budget.rpm_limit),
                    tpm: default_budget.map(|budget| budget.tpm_limit),
                },
            }
        );
        Ok(())
    }

    #[rstest]
    #[case::row_budget_precedes_default(
        "row-budget-priority-user",
        Some(BudgetFixture {
            budget_id: "row-budget-priority-row",
            max_budget: 10.0,
            rpm_limit: 101,
            tpm_limit: 1001,
        }),
        BudgetFixture {
            budget_id: "row-budget-priority-default",
            max_budget: 20.0,
            rpm_limit: 201,
            tpm_limit: 2001,
        },
        None,
        Some(BudgetFixture {
            budget_id: "row-budget-priority-row",
            max_budget: 10.0,
            rpm_limit: 101,
            tpm_limit: 1001,
        }),
        false
    )]
    #[case::default_applies_without_row_budget(
        "default-budget-user",
        None,
        BudgetFixture {
            budget_id: "default-budget-fallback",
            max_budget: 30.0,
            rpm_limit: 301,
            tpm_limit: 3001,
        },
        None,
        Some(BudgetFixture {
            budget_id: "default-budget-fallback",
            max_budget: 30.0,
            rpm_limit: 301,
            tpm_limit: 3001,
        }),
        false
    )]
    #[case::assigned_budget_precedes_key_when_all_ids_differ(
        "override-priority-user",
        Some(BudgetFixture {
            budget_id: "override-priority-row",
            max_budget: 40.0,
            rpm_limit: 401,
            tpm_limit: 4001,
        }),
        BudgetFixture {
            budget_id: "override-priority-default",
            max_budget: 50.0,
            rpm_limit: 501,
            tpm_limit: 5001,
        },
        Some(BudgetFixture {
            budget_id: "override-priority-override",
            max_budget: 60.0,
            rpm_limit: 601,
            tpm_limit: 6001,
        }),
        Some(BudgetFixture {
            budget_id: "override-priority-row",
            max_budget: 40.0,
            rpm_limit: 401,
            tpm_limit: 4001,
        }),
        true
    )]
    #[case::missing_key_budget_keeps_assigned_budget(
        "dangling-override-user",
        Some(BudgetFixture {
            budget_id: "dangling-override-row",
            max_budget: 70.0,
            rpm_limit: 701,
            tpm_limit: 7001,
        }),
        BudgetFixture {
            budget_id: "dangling-override-default",
            max_budget: 80.0,
            rpm_limit: 801,
            tpm_limit: 8001,
        },
        Some(BudgetFixture {
            budget_id: "dangling-override-missing",
            max_budget: 90.0,
            rpm_limit: 901,
            tpm_limit: 9001,
        }),
        Some(BudgetFixture {
            budget_id: "dangling-override-row",
            max_budget: 70.0,
            rpm_limit: 701,
            tpm_limit: 7001,
        }),
        false
    )]
    #[case::key_budget_precedes_proxy_default(
        "key-default-user",
        None,
        BudgetFixture {
            budget_id: "key-default-proxy",
            max_budget: 80.0,
            rpm_limit: 801,
            tpm_limit: 8001,
        },
        Some(BudgetFixture {
            budget_id: "key-default-key",
            max_budget: 90.0,
            rpm_limit: 901,
            tpm_limit: 9001,
        }),
        Some(BudgetFixture {
            budget_id: "key-default-key",
            max_budget: 90.0,
            rpm_limit: 901,
            tpm_limit: 9001,
        }),
        true
    )]
    #[case::missing_key_budget_falls_back_to_proxy_default(
        "missing-key-default-user",
        None,
        BudgetFixture {
            budget_id: "missing-key-default-proxy",
            max_budget: 80.0,
            rpm_limit: 801,
            tpm_limit: 8001,
        },
        Some(BudgetFixture {
            budget_id: "missing-key-default-key",
            max_budget: 90.0,
            rpm_limit: 901,
            tpm_limit: 9001,
        }),
        Some(BudgetFixture {
            budget_id: "missing-key-default-proxy",
            max_budget: 80.0,
            rpm_limit: 801,
            tpm_limit: 8001,
        }),
        false
    )]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn budget_resolution_matches_python_precedence(
        #[case] user_id: &'static str,
        #[case] user_budget: Option<BudgetFixture>,
        #[case] default_budget: BudgetFixture,
        #[case] override_budget: Option<BudgetFixture>,
        #[case] expected_budget: Option<BudgetFixture>,
        #[case] insert_override_budget: bool,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_budget(&mut tx, default_budget).await?;
        if let Some(budget) = user_budget {
            insert_budget(&mut tx, budget).await?;
        }
        if let (Some(budget), true) = (override_budget, insert_override_budget) {
            insert_budget(&mut tx, budget).await?;
        }
        insert_end_user(
            &mut tx,
            user_id,
            7.5,
            true,
            user_budget.map(|budget| budget.budget_id),
        )
        .await?;

        let actual = find_end_user(
            &mut *tx,
            user_id,
            Some(default_budget.budget_id),
            override_budget.map(|budget| budget.budget_id),
        )
        .await?;

        assert_eq!(
            actual,
            EndUser {
                spend: Some(7.5),
                blocked: true,
                max_budget: expected_budget.map(|budget| budget.max_budget),
                limits: RateLimits {
                    rpm: expected_budget.map(|budget| budget.rpm_limit),
                    tpm: expected_budget.map(|budget| budget.tpm_limit),
                },
            }
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn end_user_record_carries_budget_and_object_permission(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        crate::test_support::insert_budget(&mut tx, "end-user-record-budget").await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let _ = sqlx::query(
            r#"
            WITH permission AS (
                INSERT INTO "LiteLLM_ObjectPermissionTable" (object_permission_id, models, mcp_tool_permissions)
                VALUES ('end-user-record-permission', ARRAY['gpt-4o'], '{"server": ["tool"]}')
            )
            INSERT INTO "LiteLLM_EndUserTable"
                (user_id, alias, spend, allowed_model_region, models, budget_id, object_permission_id, blocked)
            VALUES ('end-user-record', 'alias', 2.5, 'eu', ARRAY['gpt-4o'], 'end-user-record-budget',
                    'end-user-record-permission', true)
            "#,
        )
        .execute(&mut *tx)
        .await?;

        let record = super::find_end_user_record(&mut *tx, "end-user-record")
            .await?
            .ok_or("end user row missing")?;

        let encoded = serde_json::to_value(&record)?;
        assert_eq!(
            encoded["litellm_budget_table"]["budget_id"],
            "end-user-record-budget"
        );
        assert_eq!(
            encoded["litellm_budget_table"]["created_at"],
            "2026-11-01T00:00:00+00:00"
        );
        assert_eq!(
            serde_json::to_value(&record.object_permission)?,
            serde_json::json!({
                "object_permission_id": "end-user-record-permission",
                "mcp_servers": [],
                "mcp_access_groups": [],
                "mcp_tool_permissions": {"server": ["tool"]},
                "vector_stores": [],
                "agents": [],
                "agent_access_groups": [],
                "models": ["gpt-4o"],
                "blocked_tools": [],
                "mcp_toolsets": [],
                "search_tools": [],
                "mcp_tool_search_enabled": null,
                "skills": [],
            })
        );
        assert_eq!(
            (
                encoded["user_id"].clone(),
                encoded["alias"].clone(),
                encoded["spend"].clone(),
                encoded["allowed_model_region"].clone(),
                encoded["models"].clone(),
                encoded["blocked"].clone(),
            ),
            (
                serde_json::json!("end-user-record"),
                serde_json::json!("alias"),
                serde_json::json!(2.5),
                serde_json::json!("eu"),
                serde_json::json!(["gpt-4o"]),
                serde_json::json!(true),
            )
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn end_user_record_without_links(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_end_user(&mut tx, "end-user-record-plain", 0.0, false, None).await?;

        let record = super::find_end_user_record(&mut *tx, "end-user-record-plain")
            .await?
            .ok_or("end user row missing")?;

        assert!(record.litellm_budget_table.is_none() && record.object_permission.is_none());
        assert!(
            super::find_end_user_record(&mut *tx, "end-user-record-missing")
                .await?
                .is_none()
        );
        Ok(())
    }

    #[rstest]
    #[case::unrestricted("restricted-ids-plain", None, false)]
    #[case::blocked(
        "restricted-ids-blocked",
        Some(r#"UPDATE "LiteLLM_EndUserTable" SET blocked = true WHERE user_id = $1"#),
        true
    )]
    #[case::region(
        "restricted-ids-region",
        Some(
            r#"UPDATE "LiteLLM_EndUserTable" SET allowed_model_region = 'eu' WHERE user_id = $1"#
        ),
        true
    )]
    #[case::default_model(
        "restricted-ids-default-model",
        Some(r#"UPDATE "LiteLLM_EndUserTable" SET default_model = 'gpt-4o' WHERE user_id = $1"#),
        true
    )]
    #[case::models(
        "restricted-ids-models",
        Some(r#"UPDATE "LiteLLM_EndUserTable" SET models = ARRAY['gpt-4o'] WHERE user_id = $1"#),
        true
    )]
    #[case::empty_models(
        "restricted-ids-empty-models",
        Some(r#"UPDATE "LiteLLM_EndUserTable" SET models = ARRAY[]::text[] WHERE user_id = $1"#),
        false
    )]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn restricted_end_user_ids_match_the_registry_predicate(
        #[case] user_id: &str,
        #[case] restriction: Option<&'static str>,
        #[case] restricted: bool,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture cleanup is outside the compile-checked query set"
        )]
        let _ = sqlx::query(r#"DELETE FROM "LiteLLM_EndUserTable""#)
            .execute(&mut *tx)
            .await?;
        insert_end_user(&mut tx, user_id, 0.0, false, None).await?;
        if let Some(restriction) = restriction {
            #[expect(
                clippy::disallowed_methods,
                reason = "Fixture updates are outside the compile-checked query set"
            )]
            let _ = sqlx::query(restriction)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }

        let ids = super::find_restricted_end_user_ids(&mut *tx, 10).await?;

        assert_eq!(ids.contains(&user_id.to_owned()), restricted);
        Ok(())
    }
}
