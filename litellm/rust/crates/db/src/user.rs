//! `LiteLLM_UserTable` reads and the conditional writes the user store makes while resolving a sign-in

use serde::Serialize;
use sqlx::{PgExecutor, types::chrono::NaiveDateTime};

use crate::{Error, RawJson};

/// One `LiteLLM_UserTable` row without its password hash
#[derive(Clone, Debug, Serialize)]
pub struct User {
    pub user_id: String,
    pub user_alias: Option<String>,
    pub team_id: Option<String>,
    pub sso_user_id: Option<String>,
    pub organization_id: Option<String>,
    pub object_permission_id: Option<String>,
    pub password_reset_required: Option<bool>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub last_breach_check_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams: Option<Vec<String>>,
    pub user_role: Option<String>,
    pub max_budget: Option<f64>,
    pub budget_limits: Option<RawJson>,
    pub spend: f64,
    pub user_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    pub metadata: RawJson,
    pub max_parallel_requests: Option<i32>,
    pub tpm_limit: Option<i64>,
    pub rpm_limit: Option<i64>,
    pub budget_duration: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub budget_reset_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_cache_controls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<String>>,
    pub model_spend: RawJson,
    pub model_max_budget: RawJson,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub created_at: Option<NaiveDateTime>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub updated_at: Option<NaiveDateTime>,
}

/// One `LiteLLM_OrganizationMembership` row, as a user row includes it
#[derive(Clone, Debug, Serialize)]
pub struct OrganizationMembership {
    pub user_id: String,
    pub organization_id: String,
    pub user_role: Option<String>,
    pub spend: Option<f64>,
    pub budget_id: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub created_at: Option<NaiveDateTime>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub updated_at: Option<NaiveDateTime>,
}

/// A user row with its organization memberships, as the user store hands it out
#[derive(Clone, Debug, Serialize)]
pub struct UserRecord {
    #[serde(flatten)]
    pub user: User,
    pub organization_memberships: Vec<OrganizationMembership>,
}

pub async fn find_user_by_id(
    executor: impl PgExecutor<'_>,
    user_id: &str,
) -> Result<Option<User>, Error> {
    Ok(
        sqlx::query_file_as!(User, "queries/user/find_user_by_id.sql", user_id)
            .fetch_optional(executor)
            .await?,
    )
}

pub async fn find_user_by_sso_id(
    executor: impl PgExecutor<'_>,
    sso_user_id: &str,
) -> Result<Option<User>, Error> {
    Ok(
        sqlx::query_file_as!(User, "queries/user/find_user_by_sso_id.sql", sso_user_id)
            .fetch_optional(executor)
            .await?,
    )
}

/// A user whose email matches `user_email` ignoring case; with `unlinked_only`, only one no sign-in has linked yet
pub async fn find_user_by_email(
    executor: impl PgExecutor<'_>,
    user_email: &str,
    unlinked_only: bool,
) -> Result<Option<User>, Error> {
    Ok(sqlx::query_file_as!(
        User,
        "queries/user/find_user_by_email.sql",
        user_email,
        unlinked_only
    )
    .fetch_optional(executor)
    .await?)
}

pub async fn find_organization_memberships(
    executor: impl PgExecutor<'_>,
    user_id: &str,
) -> Result<Vec<OrganizationMembership>, Error> {
    Ok(sqlx::query_file_as!(
        OrganizationMembership,
        "queries/user/find_organization_memberships.sql",
        user_id
    )
    .fetch_all(executor)
    .await?)
}

/// Whether a user other than `user_id` holds `user_email`, ignoring case
pub async fn email_belongs_to_another_user(
    executor: impl PgExecutor<'_>,
    user_email: &str,
    user_id: &str,
) -> Result<bool, Error> {
    Ok(sqlx::query_file_scalar!(
        "queries/user/email_belongs_to_another_user.sql",
        user_email,
        user_id
    )
    .fetch_one(executor)
    .await?)
}

/// Links `sso_user_id` to `user_id` only while the row has none; false when it already had one, the row is
/// missing, or another row holds that sign-in id
pub async fn link_null_sso_user_id(
    executor: impl PgExecutor<'_>,
    user_id: &str,
    sso_user_id: &str,
) -> Result<bool, Error> {
    let result = sqlx::query_file!(
        "queries/user/link_null_sso_user_id.sql",
        user_id,
        sso_user_id
    )
    .execute(executor)
    .await;
    match result {
        Ok(done) => Ok(done.rows_affected() == 1),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub async fn set_sso_user_id(
    executor: impl PgExecutor<'_>,
    user_id: &str,
    sso_user_id: &str,
) -> Result<(), Error> {
    sqlx::query_file!("queries/user/set_sso_user_id.sql", user_id, sso_user_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Sets `user_email` only while the row has none; false when another writer set one first
pub async fn backfill_null_user_email(
    executor: impl PgExecutor<'_>,
    user_id: &str,
    user_email: &str,
) -> Result<bool, Error> {
    let done = sqlx::query_file!(
        "queries/user/backfill_null_user_email.sql",
        user_id,
        user_email
    )
    .execute(executor)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The stored email of `user_id`, `None` when the row is missing or has no email
pub async fn find_user_email(
    executor: impl PgExecutor<'_>,
    user_id: &str,
) -> Result<Option<String>, Error> {
    Ok(
        sqlx::query_file_scalar!("queries/user/find_user_email.sql", user_id)
            .fetch_optional(executor)
            .await?
            .flatten(),
    )
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use rstest::rstest;
    use sqlx::{PgPool, Postgres, Transaction};

    use crate::test_support::database_pool;

    async fn insert_user(
        tx: &mut Transaction<'_, Postgres>,
        user_id: &str,
        user_email: Option<&str>,
        sso_user_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let result = sqlx::query(
            r#"
            INSERT INTO "LiteLLM_UserTable"
                (user_id, user_email, sso_user_id, password, models, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, 'hash', ARRAY['gpt-4o'], '{"team": "a"}', '2026-11-01 00:00:00',
                    '2026-11-01 00:00:00')
            "#,
        )
        .bind(user_id)
        .bind(user_email)
        .bind(sso_user_id)
        .execute(&mut **tx)
        .await;
        result.map(|_| ())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn user_by_id_omits_password_and_marks_timestamps_utc(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_user(&mut tx, "user-store-by-id", Some("a@example.com"), None).await?;

        let user = super::find_user_by_id(&mut *tx, "user-store-by-id")
            .await?
            .ok_or("user row missing")?;
        let encoded = serde_json::to_value(super::UserRecord {
            user,
            organization_memberships: Vec::new(),
        })?;

        assert_eq!(
            encoded,
            serde_json::json!({
                "user_id": "user-store-by-id",
                "user_alias": null,
                "team_id": null,
                "sso_user_id": null,
                "organization_id": null,
                "object_permission_id": null,
                "password_reset_required": null,
                "last_breach_check_at": null,
                "teams": [],
                "user_role": null,
                "max_budget": null,
                "budget_limits": null,
                "spend": 0.0,
                "user_email": "a@example.com",
                "models": ["gpt-4o"],
                "metadata": {"team": "a"},
                "max_parallel_requests": null,
                "tpm_limit": null,
                "rpm_limit": null,
                "budget_duration": null,
                "budget_reset_at": null,
                "allowed_cache_controls": [],
                "policies": [],
                "model_spend": {},
                "model_max_budget": {},
                "created_at": "2026-11-01T00:00:00+00:00",
                "updated_at": "2026-11-01T00:00:00+00:00",
                "organization_memberships": [],
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::exact("Mixed@Example.com", false, Some("user-store-email"))]
    #[case::ignores_case("mixed@EXAMPLE.com", false, Some("user-store-email"))]
    #[case::wildcards_are_literal("mixed@example.co_", false, None)]
    #[case::unlinked_only_skips_linked("mixed@example.com", true, None)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn user_by_email(
        #[case] lookup: &str,
        #[case] unlinked_only: bool,
        #[case] expected: Option<&str>,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_user(
            &mut tx,
            "user-store-email",
            Some("Mixed@Example.com"),
            Some("sso-linked"),
        )
        .await?;

        let found = super::find_user_by_email(&mut *tx, lookup, unlinked_only).await?;

        assert_eq!(found.map(|user| user.user_id).as_deref(), expected);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn sso_link_only_fills_an_empty_slot(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_user(&mut tx, "user-store-link-a", None, None).await?;
        insert_user(&mut tx, "user-store-link-b", None, Some("sso-held")).await?;

        let linked = super::link_null_sso_user_id(&mut *tx, "user-store-link-a", "sso-new").await?;
        let relinked =
            super::link_null_sso_user_id(&mut *tx, "user-store-link-a", "sso-other").await?;
        let found = super::find_user_by_sso_id(&mut *tx, "sso-new").await?;

        assert_eq!(
            (linked, relinked, found.map(|user| user.user_id)),
            (true, false, Some("user-store-link-a".to_owned()))
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn sso_link_to_a_held_id_is_refused(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_user(&mut tx, "user-store-held-a", None, None).await?;
        insert_user(&mut tx, "user-store-held-b", None, Some("sso-held")).await?;

        assert!(!super::link_null_sso_user_id(&mut *tx, "user-store-held-a", "sso-held").await?);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn email_backfill_and_ownership(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_user(&mut tx, "user-store-backfill", None, None).await?;
        insert_user(&mut tx, "user-store-owner", Some("Owner@Example.com"), None).await?;

        let taken = super::email_belongs_to_another_user(
            &mut *tx,
            "owner@example.com",
            "user-store-backfill",
        )
        .await?;
        let own =
            super::email_belongs_to_another_user(&mut *tx, "owner@example.com", "user-store-owner")
                .await?;
        let filled =
            super::backfill_null_user_email(&mut *tx, "user-store-backfill", "new@example.com")
                .await?;
        let refilled =
            super::backfill_null_user_email(&mut *tx, "user-store-backfill", "other@example.com")
                .await?;

        assert_eq!((taken, own, filled, refilled), (true, false, true, false));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn organization_memberships_of_a_user(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_user(&mut tx, "user-store-member", None, None).await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let _ = sqlx::query(
            r#"
            WITH budget AS (
                INSERT INTO "LiteLLM_BudgetTable" (budget_id, created_by, updated_by)
                VALUES ('user-store-org-budget', 'db-query-test', 'db-query-test')
            ), organization AS (
                INSERT INTO "LiteLLM_OrganizationTable"
                    (organization_id, organization_alias, budget_id, created_by, updated_by)
                VALUES ('user-store-org', 'org', 'user-store-org-budget', 'db-query-test',
                        'db-query-test')
            )
            INSERT INTO "LiteLLM_OrganizationMembership"
                (user_id, organization_id, user_role, created_at, updated_at)
            VALUES ('user-store-member', 'user-store-org', 'org_admin', '2026-11-03 00:00:00',
                    '2026-11-03 00:00:00')
            "#,
        )
        .execute(&mut *tx)
        .await?;

        let memberships =
            super::find_organization_memberships(&mut *tx, "user-store-member").await?;

        assert_eq!(
            serde_json::to_value(&memberships)?,
            serde_json::json!([{
                "user_id": "user-store-member",
                "organization_id": "user-store-org",
                "user_role": "org_admin",
                "spend": 0.0,
                "budget_id": null,
                "created_at": "2026-11-03T00:00:00+00:00",
                "updated_at": "2026-11-03T00:00:00+00:00",
            }])
        );
        Ok(())
    }
}
