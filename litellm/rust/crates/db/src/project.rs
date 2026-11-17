//! Reads one `LiteLLM_ProjectTable` row with its budget, as project auth checks see it.

use chrono::NaiveDateTime;
use sqlx::PgExecutor;

use crate::{Budget, Error, RawJson, budget::joined_budget};

/// One `LiteLLM_ProjectTable` row and the `LiteLLM_BudgetTable` row its `budget_id` names, if it exists.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Project {
    pub project_id: String,
    pub project_alias: Option<String>,
    pub description: Option<String>,
    pub team_id: Option<String>,
    pub budget_id: Option<String>,
    pub metadata: RawJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    pub spend: f64,
    pub model_spend: RawJson,
    pub model_rpm_limit: RawJson,
    pub model_tpm_limit: RawJson,
    pub blocked: bool,
    pub object_permission_id: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub created_at: NaiveDateTime,
    pub created_by: String,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub updated_at: NaiveDateTime,
    pub updated_by: String,
    pub litellm_budget_table: Option<Budget>,
}

struct ProjectRow {
    project_id: String,
    project_alias: Option<String>,
    description: Option<String>,
    team_id: Option<String>,
    budget_id: Option<String>,
    metadata: RawJson,
    models: Option<Vec<String>>,
    spend: f64,
    model_spend: RawJson,
    model_rpm_limit: RawJson,
    model_tpm_limit: RawJson,
    blocked: bool,
    object_permission_id: Option<String>,
    created_at: NaiveDateTime,
    created_by: String,
    updated_at: NaiveDateTime,
    updated_by: String,
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
}

impl From<ProjectRow> for Project {
    fn from(row: ProjectRow) -> Self {
        let litellm_budget_table = joined_budget!(row);
        Self {
            project_id: row.project_id,
            project_alias: row.project_alias,
            description: row.description,
            team_id: row.team_id,
            budget_id: row.budget_id,
            metadata: row.metadata,
            models: row.models,
            spend: row.spend,
            model_spend: row.model_spend,
            model_rpm_limit: row.model_rpm_limit,
            model_tpm_limit: row.model_tpm_limit,
            blocked: row.blocked,
            object_permission_id: row.object_permission_id,
            created_at: row.created_at,
            created_by: row.created_by,
            updated_at: row.updated_at,
            updated_by: row.updated_by,
            litellm_budget_table,
        }
    }
}

pub async fn find_project(
    executor: impl PgExecutor<'_>,
    project_id: &str,
) -> Result<Option<Project>, Error> {
    let row = sqlx::query_file_as!(ProjectRow, "queries/project/find_project.sql", project_id)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(Project::from))
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use rstest::rstest;
    use sqlx::{PgPool, Postgres, Transaction};

    use super::find_project;
    use crate::{RawJson, test_support::database_pool};

    async fn insert_project(
        tx: &mut Transaction<'_, Postgres>,
        project_id: &str,
        budget_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        if let Some(budget_id) = budget_id {
            #[expect(
                clippy::disallowed_methods,
                reason = "Fixture inserts are outside the compile-checked query set"
            )]
            sqlx::query(
                r#"
                INSERT INTO "LiteLLM_BudgetTable"
                    (budget_id, max_budget, rpm_limit, model_max_budget, budget_reset_at, created_by, updated_by)
                VALUES ($1, 12.5, 7, '{"gpt": 1.5}', '2026-03-04 05:06:07.890', 'creator', 'updater')
                "#,
            )
            .bind(budget_id)
            .execute(&mut **tx)
            .await?;
        }
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let result = sqlx::query(
            r#"
            INSERT INTO "LiteLLM_ProjectTable"
                (project_id, project_alias, team_id, budget_id, metadata, models, spend, model_spend, blocked,
                 created_at, created_by, updated_at, updated_by)
            VALUES ($1, 'alias', NULL, $2, '{"k": "v"}', '{m1,m2}', 3.25, '{"m1": 1.0}', true,
                    '2026-01-02 03:04:05.678', 'creator', '2026-02-03 04:05:06.789', 'updater')
            "#,
        )
        .bind(project_id)
        .bind(budget_id)
        .execute(&mut **tx)
        .await;
        result.map(|_| ())
    }

    #[rstest]
    #[case::with_budget(Some("project-budget"))]
    #[case::without_budget(None)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_project_with_its_budget(
        #[case] budget_id: Option<&str>,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_project(&mut tx, "project-1", budget_id).await?;

        let found = find_project(&mut *tx, "project-1")
            .await?
            .ok_or("row missing")?;

        assert_eq!(found.project_id, "project-1");
        assert_eq!(found.project_alias.as_deref(), Some("alias"));
        assert_eq!(found.budget_id.as_deref(), budget_id);
        assert_eq!(found.metadata.get(), r#"{"k": "v"}"#);
        assert_eq!(found.models, Some(vec!["m1".to_owned(), "m2".to_owned()]));
        assert_eq!(found.spend, 3.25);
        assert_eq!(found.model_spend.get(), r#"{"m1": 1.0}"#);
        assert!(found.blocked);
        assert_eq!(found.created_at.to_string(), "2026-01-02 03:04:05.678");
        assert_eq!(
            found.litellm_budget_table.as_ref().map(|budget| (
                budget.budget_id.as_str(),
                budget.max_budget,
                budget.rpm_limit,
                budget.model_max_budget.as_ref().map(RawJson::get),
                budget.budget_reset_at.map(|at| at.to_string()),
            )),
            budget_id.map(|budget_id| (
                budget_id,
                Some(12.5),
                Some(7),
                Some(r#"{"gpt": 1.5}"#),
                Some("2026-03-04 05:06:07.890".to_owned()),
            ))
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
        assert!(find_project(&pool, "project-missing").await?.is_none());
        Ok(())
    }
}
