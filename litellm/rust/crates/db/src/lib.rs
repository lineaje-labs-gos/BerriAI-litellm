macro_rules_attribute::attribute_alias! {
    #[apply(db_input)] =
        #[derive(Clone, Debug, PartialEq)];

    #[apply(db_row)] =
        #[derive(Clone, Debug, PartialEq, sqlx::FromRow)];
}

mod budget;
mod daily_spend;
mod deadline;
mod end_user;
mod endpoint;
mod error;
mod failure;
mod health;
mod jwt_key_mapping;
mod key;
mod model_access_group;
mod object_permission;
mod observe;
mod pools;
mod raw_json;
mod spend;
mod statements;
#[cfg(test)]
mod test_support;
mod user;
pub mod utc_timestamp;

pub use budget::{Budget, find_budget};
pub use daily_spend::{DailyEndUserRow, DailyTeamRow, DailyUserRow};
pub use deadline::within;
pub use end_user::{
    EndUser, EndUserRecord, RateLimits, find_end_user, find_end_user_record,
    find_restricted_end_user_ids,
};
pub use endpoint::{ClientIdentity, Endpoint, PoolLimits, SslMode};
pub use error::{Error, ErrorKind};
pub use failure::DbFailure;
pub use health::health_check;
pub use jwt_key_mapping::{MappedClaim, find_mapped_token, mapped_claims_for_tokens};
pub use key::{DeprecatedKey, KeyViewRow, find_deprecated_key, find_key_view};
pub use model_access_group::{
    ModelAccessGroupBudgetRow, find_model_access_group_budgets, model_access_group_names,
};
pub use object_permission::ObjectPermission;
pub use observe::{Call, Observer, Outcome, QueryEvent, observe};
pub use pools::Pools;
pub use raw_json::RawJson;
pub use spend::{
    DailySpend, DailyTable, EntitySpend, SpendLog, SpendLogRow, TeamMemberSpend,
    add_end_user_spend, add_key_spend, add_team_member_spend, add_team_spend, add_user_spend,
    begin, insert_spend_logs, lock_team, upsert_daily_end_user_spend, upsert_daily_spend,
    upsert_daily_team_spend, upsert_daily_user_spend,
};
pub use statements::Statements;
pub use user::{
    OrganizationMembership, User, UserRecord, backfill_null_user_email,
    email_belongs_to_another_user, find_organization_memberships, find_user_by_email,
    find_user_by_id, find_user_by_sso_id, find_user_email, link_null_sso_user_id, set_sso_user_id,
};
