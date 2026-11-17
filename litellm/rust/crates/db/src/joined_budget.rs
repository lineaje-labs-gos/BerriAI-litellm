//! Builds a [`Budget`] from the `b_`-prefixed columns of a `LEFT JOIN "LiteLLM_BudgetTable" AS b`.

use chrono::NaiveDateTime;

use crate::{Budget, RawJson};

/// The nullable `b_` columns a left-joined budget selects, `None` throughout when no budget row matched.
#[derive(Clone, Debug, Default)]
pub(crate) struct JoinedBudget {
    pub(crate) budget_id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) max_budget: Option<f64>,
    pub(crate) soft_budget: Option<f64>,
    pub(crate) max_parallel_requests: Option<i32>,
    pub(crate) tpm_limit: Option<i64>,
    pub(crate) rpm_limit: Option<i64>,
    pub(crate) tpd_limit: Option<i64>,
    pub(crate) model_max_budget: Option<RawJson>,
    pub(crate) model_tpm_limit: Option<RawJson>,
    pub(crate) model_rpm_limit: Option<RawJson>,
    pub(crate) budget_duration: Option<String>,
    pub(crate) budget_limits: Option<RawJson>,
    pub(crate) cap_policy: Option<String>,
    pub(crate) budget_reset_at: Option<NaiveDateTime>,
    pub(crate) budget_reset_count: Option<i32>,
    pub(crate) allowed_models: Option<Vec<String>>,
    pub(crate) temp_budget_increase: Option<f64>,
    pub(crate) temp_budget_expiry: Option<NaiveDateTime>,
    pub(crate) created_at: Option<NaiveDateTime>,
    pub(crate) created_by: Option<String>,
    pub(crate) updated_at: Option<NaiveDateTime>,
    pub(crate) updated_by: Option<String>,
}

impl JoinedBudget {
    pub(crate) fn into_budget(self) -> Option<Budget> {
        Some(Budget {
            budget_id: self.budget_id?,
            name: self.name,
            max_budget: self.max_budget,
            soft_budget: self.soft_budget,
            max_parallel_requests: self.max_parallel_requests,
            tpm_limit: self.tpm_limit,
            rpm_limit: self.rpm_limit,
            tpd_limit: self.tpd_limit,
            model_max_budget: self.model_max_budget,
            model_tpm_limit: self.model_tpm_limit,
            model_rpm_limit: self.model_rpm_limit,
            budget_duration: self.budget_duration,
            budget_limits: self.budget_limits,
            cap_policy: self.cap_policy,
            budget_reset_at: self.budget_reset_at,
            budget_reset_count: self.budget_reset_count.unwrap_or_default(),
            allowed_models: self.allowed_models,
            temp_budget_increase: self.temp_budget_increase,
            temp_budget_expiry: self.temp_budget_expiry,
            created_at: self.created_at?,
            created_by: self.created_by?,
            updated_at: self.updated_at?,
            updated_by: self.updated_by?,
        })
    }
}

/// Moves the `b_` fields of a query row into a [`JoinedBudget`].
macro_rules! joined_budget {
    ($row:expr) => {
        $crate::joined_budget::JoinedBudget {
            budget_id: $row.b_budget_id,
            name: $row.b_name,
            max_budget: $row.b_max_budget,
            soft_budget: $row.b_soft_budget,
            max_parallel_requests: $row.b_max_parallel_requests,
            tpm_limit: $row.b_tpm_limit,
            rpm_limit: $row.b_rpm_limit,
            tpd_limit: $row.b_tpd_limit,
            model_max_budget: $row.b_model_max_budget,
            model_tpm_limit: $row.b_model_tpm_limit,
            model_rpm_limit: $row.b_model_rpm_limit,
            budget_duration: $row.b_budget_duration,
            budget_limits: $row.b_budget_limits,
            cap_policy: $row.b_cap_policy,
            budget_reset_at: $row.b_budget_reset_at,
            budget_reset_count: $row.b_budget_reset_count,
            allowed_models: $row.b_allowed_models,
            temp_budget_increase: $row.b_temp_budget_increase,
            temp_budget_expiry: $row.b_temp_budget_expiry,
            created_at: $row.b_created_at,
            created_by: $row.b_created_by,
            updated_at: $row.b_updated_at,
            updated_by: $row.b_updated_by,
        }
    };
}
pub(crate) use joined_budget;
