//! Virtual key reads: the `combined_view` row (the key joined with its team, membership, budgets, project and
//! organization) and the rotation grace-period lookup in `LiteLLM_DeprecatedVerificationToken`.

use chrono::NaiveDateTime;
use sqlx::PgExecutor;

use crate::{Error, RawJson};

/// One `combined_view` row, column for column. Shaping it into the auth view is the caller's job.
#[macro_rules_attribute::apply(crate::db_row)]
pub struct KeyViewRow {
    pub token: String,
    pub key_name: Option<String>,
    pub key_alias: Option<String>,
    pub soft_budget_cooldown: bool,
    pub spend: f64,
    pub total_spend: f64,
    pub expires: Option<NaiveDateTime>,
    pub models: Option<Vec<String>>,
    pub aliases: RawJson,
    pub config: RawJson,
    pub router_settings: Option<RawJson>,
    pub user_id: Option<String>,
    pub team_id: Option<String>,
    pub agent_id: Option<String>,
    pub project_id: Option<String>,
    pub permissions: RawJson,
    pub max_parallel_requests: Option<i32>,
    pub metadata: RawJson,
    pub blocked: Option<bool>,
    pub tpm_limit: Option<i64>,
    pub rpm_limit: Option<i64>,
    pub tpd_limit: Option<i64>,
    pub max_budget: Option<f64>,
    pub budget_duration: Option<String>,
    pub budget_reset_at: Option<NaiveDateTime>,
    pub allowed_cache_controls: Option<Vec<String>>,
    pub allowed_routes: Option<Vec<String>>,
    pub key_type: Option<String>,
    pub policies: Option<Vec<String>>,
    pub access_group_ids: Option<Vec<String>>,
    pub model_spend: RawJson,
    pub model_max_budget: RawJson,
    pub budget_fallbacks: RawJson,
    pub budget_id: Option<String>,
    pub organization_id: Option<String>,
    pub object_permission_id: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
    pub settings_updated_at: Option<NaiveDateTime>,
    pub last_active: Option<NaiveDateTime>,
    pub rotation_count: Option<i32>,
    pub auto_rotate: Option<bool>,
    pub rotation_interval: Option<String>,
    pub last_rotation_at: Option<NaiveDateTime>,
    pub key_rotation_at: Option<NaiveDateTime>,
    pub budget_limits: Option<RawJson>,
    pub team_spend: Option<f64>,
    pub team_max_budget: Option<f64>,
    pub team_soft_budget: Option<f64>,
    pub team_model_max_budget: Option<RawJson>,
    pub team_tpm_limit: Option<i64>,
    pub team_rpm_limit: Option<i64>,
    pub team_tpd_limit: Option<i64>,
    pub team_models: Option<Vec<String>>,
    pub team_metadata: Option<RawJson>,
    pub team_blocked: Option<bool>,
    pub team_alias: Option<String>,
    pub team_members_with_roles: Option<RawJson>,
    pub team_object_permission_id: Option<String>,
    pub org_id: Option<String>,
    pub project_alias: Option<String>,
    pub team_member_spend: Option<f64>,
    pub team_member_tpm_limit: Option<i64>,
    pub team_member_rpm_limit: Option<i64>,
    pub team_model_aliases: Option<RawJson>,
    pub litellm_budget_table_max_budget: Option<f64>,
    pub litellm_budget_table_tpm_limit: Option<i64>,
    pub litellm_budget_table_rpm_limit: Option<i64>,
    pub litellm_budget_table_tpd_limit: Option<i64>,
    pub litellm_budget_table_model_max_budget: Option<RawJson>,
    pub litellm_budget_table_soft_budget: Option<f64>,
    pub organization_metadata: Option<RawJson>,
    pub organization_alias: Option<String>,
    pub organization_max_budget: Option<f64>,
    pub organization_tpm_limit: Option<i64>,
    pub organization_rpm_limit: Option<i64>,
}

/// The key a rotated-away token still stands for, and when that grace period ends.
#[macro_rules_attribute::apply(crate::db_row)]
pub struct DeprecatedKey {
    pub active_token_id: String,
    pub revoke_at: NaiveDateTime,
}

/// The `combined_view` row of the key whose hash is `token`, or `None` when no such key exists.
pub async fn find_key_view(
    executor: impl PgExecutor<'_>,
    token: &str,
) -> Result<Option<KeyViewRow>, Error> {
    Ok(
        sqlx::query_file_as!(KeyViewRow, "queries/key/find_key_view.sql", token)
            .fetch_optional(executor)
            .await?,
    )
}

/// The active key that the rotated-away `token` maps to while its grace period runs past `now`.
pub async fn find_deprecated_key(
    executor: impl PgExecutor<'_>,
    token: &str,
    now: NaiveDateTime,
) -> Result<Option<DeprecatedKey>, Error> {
    Ok(sqlx::query_file_as!(
        DeprecatedKey,
        "queries/key/find_deprecated_key.sql",
        token,
        now
    )
    .fetch_optional(executor)
    .await?)
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{find_deprecated_key, find_key_view};
    use crate::test_support::database_pool;
    use chrono::{Duration, NaiveDateTime, Utc};
    use rstest::rstest;
    use sqlx::{PgPool, Postgres, Transaction};

    async fn execute(
        tx: &mut Transaction<'_, Postgres>,
        sql: &'static str,
        binds: &[&str],
    ) -> Result<(), sqlx::Error> {
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let mut query = sqlx::query(sql);
        for bind in binds {
            query = query.bind(*bind);
        }
        query.execute(&mut **tx).await.map(|_| ())
    }

    async fn insert_key_with_team(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
        execute(
            tx,
            r#"INSERT INTO "LiteLLM_BudgetTable" (budget_id, max_budget, tpm_limit, soft_budget, model_max_budget, created_by, updated_by)
               VALUES ('kv-budget', 9.5, 77, 4.5, '{"gpt-4o": 1}', 'db-query-test', 'db-query-test')"#,
            &[],
        )
        .await?;
        execute(
            tx,
            r#"INSERT INTO "LiteLLM_TeamTable" (team_id, team_alias, models, blocked, max_budget, members_with_roles, metadata)
               VALUES ('kv-team', 'Team KV', ARRAY['gpt-4o'], true, 12.0,
                       '[{"user_id": "kv-user", "role": "user"}, {"user_id": "other", "role": "admin"}, {"user_id": "kv-user", "role": "admin"}]',
                       '{"tier": "gold"}')"#,
            &[],
        )
        .await?;
        execute(
            tx,
            r#"INSERT INTO "LiteLLM_VerificationToken" (token, user_id, team_id, budget_id, models, rpm_limit, expires, metadata)
               VALUES ('kv-token', 'kv-user', 'kv-team', 'kv-budget', ARRAY['m1'], 5, '2030-01-02 03:04:05.678', '{"a": [1, 2]}')"#,
            &[],
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn key_view_joins_team_member_and_budget(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_key_with_team(&mut tx).await?;

        let row = find_key_view(&mut *tx, "kv-token")
            .await?
            .ok_or("key view row")?;

        assert_eq!(
            (
                row.team_alias.as_deref(),
                row.team_blocked,
                row.team_models.as_deref(),
                row.rpm_limit,
                row.litellm_budget_table_tpm_limit,
                row.litellm_budget_table_max_budget,
                row.litellm_budget_table_soft_budget,
                row.metadata.get(),
                row.team_metadata.as_ref().map(crate::RawJson::get),
                row.expires,
            ),
            (
                Some("Team KV"),
                Some(true),
                Some(&["gpt-4o".to_owned()][..]),
                Some(5),
                Some(77),
                Some(9.5),
                Some(4.5),
                r#"{"a": [1, 2]}"#,
                Some(r#"{"tier": "gold"}"#),
                NaiveDateTime::parse_from_str("2030-01-02 03:04:05.678", "%Y-%m-%d %H:%M:%S%.f")
                    .ok(),
            )
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_key_has_no_view(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;

        assert_eq!(find_key_view(&mut *tx, "no-such-token").await?, None);
        Ok(())
    }

    #[rstest]
    #[case::within_grace_period(Duration::hours(1), Some("kv-active"))]
    #[case::after_revoke(Duration::hours(-1), None)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn deprecated_key_resolves_until_revoked(
        #[case] revoke_in: Duration,
        #[case] expected: Option<&str>,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        let now = Utc::now().naive_utc();
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        sqlx::query(
            r#"INSERT INTO "LiteLLM_DeprecatedVerificationToken" (id, token, active_token_id, revoke_at)
               VALUES ('kv-deprecated', 'kv-old', 'kv-active', $1)"#,
        )
        .bind(now + revoke_in)
        .execute(&mut *tx)
        .await?;

        let found = find_deprecated_key(&mut *tx, "kv-old", now).await?;

        assert_eq!(found.map(|key| key.active_token_id).as_deref(), expected);
        Ok(())
    }
}
