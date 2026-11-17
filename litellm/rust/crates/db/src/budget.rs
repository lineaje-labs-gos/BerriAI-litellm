//! The `LiteLLM_BudgetTable` row that end users, members, projects and tags share.

use chrono::NaiveDateTime;

use crate::RawJson;

/// One `LiteLLM_BudgetTable` row with every scalar column, JSON columns kept as the stored text.
#[macro_rules_attribute::apply(crate::db_row)]
#[derive(serde::Serialize)]
pub struct Budget {
    pub budget_id: String,
    pub name: Option<String>,
    pub max_budget: Option<f64>,
    pub soft_budget: Option<f64>,
    pub max_parallel_requests: Option<i32>,
    pub tpm_limit: Option<i64>,
    pub rpm_limit: Option<i64>,
    pub tpd_limit: Option<i64>,
    pub model_max_budget: Option<RawJson>,
    pub model_tpm_limit: Option<RawJson>,
    pub model_rpm_limit: Option<RawJson>,
    pub budget_duration: Option<String>,
    pub budget_limits: Option<RawJson>,
    pub cap_policy: Option<String>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub budget_reset_at: Option<NaiveDateTime>,
    pub budget_reset_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    pub temp_budget_increase: Option<f64>,
    #[serde(serialize_with = "crate::utc_timestamp::option::serialize")]
    pub temp_budget_expiry: Option<NaiveDateTime>,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub created_at: NaiveDateTime,
    pub created_by: String,
    #[serde(serialize_with = "crate::utc_timestamp::serialize")]
    pub updated_at: NaiveDateTime,
    pub updated_by: String,
}

/// Builds the joined [`Budget`] from a row whose budget columns carry a `budget_` prefix and are all nullable
/// because of the outer join, `None` when the join found no row. It moves only the budget columns out, so
/// the caller keeps the rest of the row.
macro_rules! joined_budget {
    ($row:ident) => {{
        match (
            $row.budget_budget_id,
            $row.budget_created_at,
            $row.budget_created_by,
            $row.budget_updated_at,
            $row.budget_updated_by,
        ) {
            (
                Some(budget_id),
                Some(created_at),
                Some(created_by),
                Some(updated_at),
                Some(updated_by),
            ) => Some($crate::Budget {
                budget_id,
                name: $row.budget_name,
                max_budget: $row.budget_max_budget,
                soft_budget: $row.budget_soft_budget,
                max_parallel_requests: $row.budget_max_parallel_requests,
                tpm_limit: $row.budget_tpm_limit,
                rpm_limit: $row.budget_rpm_limit,
                tpd_limit: $row.budget_tpd_limit,
                model_max_budget: $row.budget_model_max_budget,
                model_tpm_limit: $row.budget_model_tpm_limit,
                model_rpm_limit: $row.budget_model_rpm_limit,
                budget_duration: $row.budget_budget_duration,
                budget_limits: $row.budget_budget_limits,
                cap_policy: $row.budget_cap_policy,
                budget_reset_at: $row.budget_budget_reset_at,
                budget_reset_count: $row.budget_budget_reset_count.unwrap_or_default(),
                allowed_models: $row.budget_allowed_models,
                temp_budget_increase: $row.budget_temp_budget_increase,
                temp_budget_expiry: $row.budget_temp_budget_expiry,
                created_at,
                created_by,
                updated_at,
                updated_by,
            }),
            _ => None,
        }
    }};
}

pub(crate) use joined_budget;
