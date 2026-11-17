//! Reads one `LiteLLM_TeamMembership` row for auth, with the member's budget from `LiteLLM_BudgetTable`.

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgExecutor;

use crate::{Budget, Error, RawJson, joined_budget::joined_budget};

/// A user's membership of a team, with the per-member budget when one is assigned.
#[derive(Clone, Debug, Serialize)]
pub struct TeamMembership {
    pub user_id: String,
    pub team_id: String,
    pub budget_id: Option<String>,
    pub spend: f64,
    pub total_spend: f64,
    pub litellm_budget_table: Option<Budget>,
}

#[macro_rules_attribute::apply(crate::db_row)]
struct TeamMembershipRow {
    user_id: String,
    team_id: String,
    budget_id: Option<String>,
    spend: f64,
    total_spend: f64,
    b_budget_id: Option<String>,
    b_name: Option<String>,
    b_max_budget: Option<f64>,
    b_soft_budget: Option<f64>,
    b_max_parallel_requests: Option<i32>,
    b_tpm_limit: Option<i64>,
    b_rpm_limit: Option<i64>,
    b_tpd_limit: Option<i64>,
    b_model_max_budget: Option<RawJson>,
    b_model_tpm_limit: Option<RawJson>,
    b_model_rpm_limit: Option<RawJson>,
    b_budget_duration: Option<String>,
    b_budget_limits: Option<RawJson>,
    b_cap_policy: Option<String>,
    b_budget_reset_at: Option<NaiveDateTime>,
    b_budget_reset_count: Option<i32>,
    b_allowed_models: Option<Vec<String>>,
    b_temp_budget_increase: Option<f64>,
    b_temp_budget_expiry: Option<NaiveDateTime>,
    b_created_at: Option<NaiveDateTime>,
    b_created_by: Option<String>,
    b_updated_at: Option<NaiveDateTime>,
    b_updated_by: Option<String>,
}

pub async fn find_team_membership(
    executor: impl PgExecutor<'_>,
    user_id: &str,
    team_id: &str,
) -> Result<Option<TeamMembership>, Error> {
    let row = sqlx::query_file_as!(
        TeamMembershipRow,
        "queries/team_membership/find_team_membership.sql",
        user_id,
        team_id,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|row| TeamMembership {
        user_id: row.user_id,
        team_id: row.team_id,
        budget_id: row.budget_id,
        spend: row.spend,
        total_spend: row.total_spend,
        litellm_budget_table: joined_budget!(row).into_budget(),
    }))
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::find_team_membership;
    use crate::test_support::database_pool;
    use rstest::rstest;
    use sqlx::PgPool;

    #[rstest]
    #[case::with_member_budget(Some("member-budget"), Some(Some(42)))]
    #[case::without_member_budget(None, None)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_membership_with_member_budget(
        #[case] budget_id: Option<&str>,
        #[case] expected_rpm: Option<Option<i64>>,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let _inserted = sqlx::query(
            r#"INSERT INTO "LiteLLM_BudgetTable" (budget_id, rpm_limit, allowed_models, created_by, updated_by)
               VALUES ('member-budget', 42, '{gpt-4o}', 'a', 'b')"#,
        )
        .execute(&mut *tx)
        .await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let _inserted = sqlx::query(
            r#"INSERT INTO "LiteLLM_TeamMembership" (user_id, team_id, spend, total_spend, budget_id)
               VALUES ('member', 'member-team', 1.5, 2.5, $1)"#,
        )
        .bind(budget_id)
        .execute(&mut *tx)
        .await?;

        let membership = find_team_membership(&mut *tx, "member", "member-team")
            .await?
            .ok_or("membership missing")?;

        assert_eq!(membership.budget_id.as_deref(), budget_id);
        assert_eq!(membership.spend, 1.5);
        assert_eq!(membership.total_spend, 2.5);
        assert_eq!(
            membership
                .litellm_budget_table
                .as_ref()
                .map(|budget| budget.rpm_limit),
            expected_rpm
        );
        if let Some(budget) = membership.litellm_budget_table {
            assert_eq!(
                budget.allowed_models.as_deref(),
                Some(["gpt-4o".to_owned()].as_slice())
            );
        }
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_membership_is_none(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        assert!(
            find_team_membership(&pool, "no-such-user", "no-such-team")
                .await?
                .is_none()
        );
        Ok(())
    }
}
