//! Reads `LiteLLM_TagTable` rows with their budgets, and the bounded list of tag names the registry caches.

use chrono::NaiveDateTime;
use sqlx::PgExecutor;

use crate::{Budget, Error, RawJson, budget::joined_budget};

/// One `LiteLLM_TagTable` row and the `LiteLLM_BudgetTable` row its `budget_id` names, if it exists.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Tag {
    pub tag_name: String,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    pub model_info: Option<RawJson>,
    pub spend: f64,
    pub budget_id: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub updated_at: NaiveDateTime,
    pub litellm_budget_table: Option<Budget>,
}

struct TagRow {
    tag_name: String,
    description: Option<String>,
    models: Option<Vec<String>>,
    model_info: Option<RawJson>,
    spend: f64,
    budget_id: Option<String>,
    created_at: NaiveDateTime,
    created_by: Option<String>,
    updated_at: NaiveDateTime,
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

impl From<TagRow> for Tag {
    fn from(row: TagRow) -> Self {
        let litellm_budget_table = joined_budget!(row);
        Self {
            tag_name: row.tag_name,
            description: row.description,
            models: row.models,
            model_info: row.model_info,
            spend: row.spend,
            budget_id: row.budget_id,
            created_at: row.created_at,
            created_by: row.created_by,
            updated_at: row.updated_at,
            litellm_budget_table,
        }
    }
}

/// The rows for `tag_names` that exist, in no particular order.
pub async fn find_tags(
    executor: impl PgExecutor<'_>,
    tag_names: &[String],
) -> Result<Vec<Tag>, Error> {
    let rows = sqlx::query_file_as!(TagRow, "queries/tag/find_tags.sql", tag_names)
        .fetch_all(executor)
        .await?;
    Ok(rows.into_iter().map(Tag::from).collect())
}

/// Up to `limit` tag names, in no particular order. The registry asks for one more than it can hold to
/// tell a full table from an overflowing one.
pub async fn find_tag_names(
    executor: impl PgExecutor<'_>,
    limit: i64,
) -> Result<Vec<String>, Error> {
    Ok(
        sqlx::query_file_scalar!("queries/tag/find_tag_names.sql", limit)
            .fetch_all(executor)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use rstest::rstest;
    use sqlx::{PgPool, Postgres, Transaction};

    use super::{find_tag_names, find_tags};
    use crate::{RawJson, test_support::database_pool};

    async fn insert_tags(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture inserts are outside the compile-checked query set"
        )]
        let result = sqlx::query(
            r#"
            WITH budget AS (
                INSERT INTO "LiteLLM_BudgetTable" (budget_id, max_budget, created_by, updated_by)
                VALUES ('tag-budget', 4.5, 'creator', 'updater')
            )
            INSERT INTO "LiteLLM_TagTable" (tag_name, description, models, model_info, spend, budget_id)
            VALUES ('tag-a', 'first', '{m1}', '{"id1": "m1"}', 1.5, 'tag-budget'),
                   ('tag-b', NULL, NULL, NULL, 0, NULL)
            "#,
        )
        .execute(&mut **tx)
        .await;
        result.map(|_| ())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_requested_tags_with_budgets(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        insert_tags(&mut tx).await?;

        let mut found = find_tags(
            &mut *tx,
            &[
                "tag-a".to_owned(),
                "tag-b".to_owned(),
                "tag-missing".to_owned(),
            ],
        )
        .await?;
        found.sort_by(|left, right| left.tag_name.cmp(&right.tag_name));

        assert_eq!(
            found
                .iter()
                .map(|tag| (
                    tag.tag_name.as_str(),
                    tag.description.as_deref(),
                    tag.models.clone(),
                    tag.model_info.as_ref().map(RawJson::get),
                    tag.spend,
                    tag.litellm_budget_table
                        .as_ref()
                        .map(|budget| (budget.budget_id.as_str(), budget.max_budget)),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "tag-a",
                    Some("first"),
                    Some(vec!["m1".to_owned()]),
                    Some(r#"{"id1": "m1"}"#),
                    1.5,
                    Some(("tag-budget", Some(4.5))),
                ),
                ("tag-b", None, None, None, 0.0, None),
            ]
        );
        Ok(())
    }

    #[rstest]
    #[case::under_limit(10, 2)]
    #[case::at_limit(1, 1)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn tag_names_stop_at_limit(
        #[case] limit: i64,
        #[case] expected: usize,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture cleanup is outside the compile-checked query set"
        )]
        sqlx::query(r#"DELETE FROM "LiteLLM_TagTable""#)
            .execute(&mut *tx)
            .await?;
        insert_tags(&mut tx).await?;

        assert_eq!(find_tag_names(&mut *tx, limit).await?.len(), expected);
        Ok(())
    }
}
