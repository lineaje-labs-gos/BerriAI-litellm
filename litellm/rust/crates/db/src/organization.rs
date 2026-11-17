//! Reads `LiteLLM_OrganizationTable` rows for auth: one by id, optionally with its budget from
//! `LiteLLM_BudgetTable`, or the rows sharing an alias.

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgExecutor;

use crate::{Budget, Error, RawJson, joined_budget::joined_budget};

/// One `LiteLLM_OrganizationTable` row, with its budget only when the caller asked for it.
#[derive(Clone, Debug, Serialize)]
pub struct Organization {
    pub organization_id: String,
    pub organization_alias: String,
    pub budget_id: String,
    pub metadata: RawJson,
    pub models: Vec<String>,
    pub spend: f64,
    pub model_spend: RawJson,
    pub object_permission_id: Option<String>,
    #[serde(with = "crate::utc_timestamp")]
    pub created_at: NaiveDateTime,
    pub created_by: String,
    #[serde(with = "crate::utc_timestamp")]
    pub updated_at: NaiveDateTime,
    pub updated_by: String,
    pub litellm_budget_table: Option<Budget>,
}

#[macro_rules_attribute::apply(crate::db_row)]
struct OrganizationRow {
    organization_id: String,
    organization_alias: String,
    budget_id: String,
    metadata: RawJson,
    models: Vec<String>,
    spend: f64,
    model_spend: RawJson,
    object_permission_id: Option<String>,
    created_at: NaiveDateTime,
    created_by: String,
    updated_at: NaiveDateTime,
    updated_by: String,
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

pub async fn find_organization(
    executor: impl PgExecutor<'_>,
    organization_id: &str,
    with_budget: bool,
) -> Result<Option<Organization>, Error> {
    let row = sqlx::query_file_as!(
        OrganizationRow,
        "queries/organization/find_organization.sql",
        organization_id,
        with_budget,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(Organization::from))
}

/// Up to two organizations whose alias is `organization_alias`, without their budgets, so a caller can tell a
/// unique alias from a shared one.
pub async fn find_organizations_by_alias(
    executor: impl PgExecutor<'_>,
    organization_alias: &str,
) -> Result<Vec<Organization>, Error> {
    let rows = sqlx::query_file_as!(
        OrganizationRow,
        "queries/organization/find_organizations_by_alias.sql",
        organization_alias,
        false,
    )
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(Organization::from).collect())
}

impl From<OrganizationRow> for Organization {
    fn from(row: OrganizationRow) -> Self {
        Self {
            organization_id: row.organization_id,
            organization_alias: row.organization_alias,
            budget_id: row.budget_id,
            metadata: row.metadata,
            models: row.models,
            spend: row.spend,
            model_spend: row.model_spend,
            object_permission_id: row.object_permission_id,
            created_at: row.created_at,
            created_by: row.created_by,
            updated_at: row.updated_at,
            updated_by: row.updated_by,
            litellm_budget_table: joined_budget!(row).into_budget(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{find_organization, find_organizations_by_alias};
    use crate::test_support::database_pool;
    use rstest::rstest;
    use sqlx::PgPool;

    #[rstest]
    #[case::with_budget(true, Some(Some(7.5)))]
    #[case::without_budget(false, None)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_organization_and_budget_on_request(
        #[case] with_budget: bool,
        #[case] expected_max_budget: Option<Option<f64>>,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let _inserted = sqlx::query(
            r#"INSERT INTO "LiteLLM_BudgetTable" (budget_id, max_budget, created_by, updated_by)
               VALUES ('org-budget', 7.5, 'a', 'b')"#,
        )
        .execute(&mut *tx)
        .await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let _inserted = sqlx::query(
            r#"INSERT INTO "LiteLLM_OrganizationTable"
                   (organization_id, organization_alias, budget_id, models, metadata, created_by, updated_by)
               VALUES ('find-org', 'Org', 'org-budget', NULL, '{"k": [1]}', 'a', 'b')"#,
        )
        .execute(&mut *tx)
        .await?;

        let organization = find_organization(&mut *tx, "find-org", with_budget)
            .await?
            .ok_or("organization missing")?;

        assert_eq!(organization.organization_alias, "Org");
        assert!(organization.models.is_empty());
        assert_eq!(organization.metadata.get(), r#"{"k": [1]}"#);
        assert_eq!(
            organization
                .litellm_budget_table
                .map(|budget| budget.max_budget),
            expected_max_budget
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_organization_is_none(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        assert!(
            find_organization(&pool, "no-such-org", true)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[rstest]
    #[case::unique("Unique", &["org-alias-a"])]
    #[case::shared("Shared", &["org-alias-b", "org-alias-c"])]
    #[case::missing("No such org", &[])]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_organizations_by_alias_without_budgets(
        #[case] alias: &str,
        #[case] expected: &[&str],
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let _inserted = sqlx::query(
            r#"INSERT INTO "LiteLLM_BudgetTable" (budget_id, max_budget, created_by, updated_by)
               VALUES ('org-alias-budget', 7.5, 'a', 'b')"#,
        )
        .execute(&mut *tx)
        .await?;
        for (organization_id, organization_alias) in [
            ("org-alias-a", "Unique"),
            ("org-alias-b", "Shared"),
            ("org-alias-c", "Shared"),
            ("org-alias-d", "Shared"),
        ] {
            #[expect(
                clippy::disallowed_methods,
                reason = "Fixture writes are outside the compile-checked query set"
            )]
            let _inserted = sqlx::query(
                r#"INSERT INTO "LiteLLM_OrganizationTable"
                       (organization_id, organization_alias, budget_id, created_by, updated_by)
                   VALUES ($1, $2, 'org-alias-budget', 'a', 'b')"#,
            )
            .bind(organization_id)
            .bind(organization_alias)
            .execute(&mut *tx)
            .await?;
        }

        let organizations = find_organizations_by_alias(&mut *tx, alias).await?;

        assert_eq!(
            organizations
                .iter()
                .map(|organization| organization.organization_id.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            organizations
                .iter()
                .all(|organization| organization.litellm_budget_table.is_none())
        );
        Ok(())
    }
}
