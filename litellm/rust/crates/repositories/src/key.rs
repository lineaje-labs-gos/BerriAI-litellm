//! The virtual key repository: the `combined_view` read, the rotation grace-period fallback with its memory tier,
//! and the shaping of the row into the view the auth flow validates.

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, NaiveDateTime, Utc};
use litellm_db::{Call, KeyViewRow, RawJson, utc_timestamp};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use serde_json::value::RawValue;

use crate::{Database, Side};

const KEY_VIEW: Call = Call {
    call_type: "get_data",
    table: Some("combined_view"),
    operation: None,
};

/// How long a found grace-period mapping is served from memory before the database is asked again.
const DEPRECATED_KEY_TTL: Duration = Duration::from_secs(60);
/// How many grace-period mappings are held in memory; the oldest goes first.
const DEPRECATED_KEY_CAPACITY: usize = 1000;

/// Reads virtual keys by hash. A hash that is not a key but was rotated away from one still within its grace
/// period resolves to the key it was rotated into.
pub struct KeyRepository {
    database: Database,
    deprecated: Mutex<DeprecatedKeys>,
}

impl KeyRepository {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            deprecated: Mutex::new(DeprecatedKeys::default()),
        }
    }

    /// The view of the key whose hash is `token`, or `None` when neither a key nor a rotated-away hash in its
    /// grace period has it. `Side::Writer` reads the latest commit and skips the grace-period memory tier.
    pub async fn find(
        &self,
        token: &str,
        side: Side,
    ) -> Result<Option<KeyView>, litellm_db::Error> {
        let now = SystemTime::now();
        self.database
            .run(KEY_VIEW, || self.find_once(token, side, now))
            .await
    }

    async fn find_once(
        &self,
        token: &str,
        side: Side,
        now: SystemTime,
    ) -> Result<Option<KeyView>, litellm_db::Error> {
        let mut session = self.database.session(side).await?;
        if let Some(row) = litellm_db::find_key_view(session.executor(), token).await? {
            return Ok(Some(KeyView::new(row, now)));
        }
        let cached = match side {
            Side::Read => self.deprecated_keys().active_token(token, now),
            Side::Writer => None,
        };
        let active_token = match cached {
            Some(active_token) => active_token,
            None => {
                let at = DateTime::<Utc>::from(now).naive_utc();
                match litellm_db::find_deprecated_key(session.executor(), token, at).await {
                    Ok(Some(deprecated)) => {
                        self.deprecated_keys().insert(
                            token,
                            &deprecated.active_token_id,
                            now,
                            deprecated.revoke_at,
                        );
                        deprecated.active_token_id
                    }
                    Ok(None) => return Ok(None),
                    Err(error) => {
                        tracing::debug!(error = %litellm_error::Report::new(&error), "deprecated key lookup skipped");
                        return Ok(None);
                    }
                }
            }
        };
        Ok(litellm_db::find_key_view(session.executor(), &active_token)
            .await?
            .map(|row| KeyView::new(row, now)))
    }

    fn deprecated_keys(&self) -> std::sync::MutexGuard<'_, DeprecatedKeys> {
        self.deprecated
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Grace-period mappings found in the database, each served until its memory TTL or its revoke time,
/// whichever comes first. Only found mappings are held, so a miss never evicts a real one.
#[derive(Default)]
struct DeprecatedKeys {
    entries: HashMap<String, DeprecatedEntry>,
    order: VecDeque<String>,
}

struct DeprecatedEntry {
    active_token: String,
    serve_until: SystemTime,
}

impl DeprecatedKeys {
    fn active_token(&mut self, token: &str, now: SystemTime) -> Option<String> {
        let entry = self.entries.get(token)?;
        if now < entry.serve_until {
            return Some(entry.active_token.clone());
        }
        self.remove(token);
        None
    }

    fn insert(
        &mut self,
        token: &str,
        active_token: &str,
        now: SystemTime,
        revoke_at: NaiveDateTime,
    ) {
        let revoke_at = SystemTime::from(revoke_at.and_utc());
        let serve_until = (now + DEPRECATED_KEY_TTL).min(revoke_at);
        if self.entries.contains_key(token) {
            self.remove(token);
        }
        while self.entries.len() >= DEPRECATED_KEY_CAPACITY {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
        self.order.push_back(token.to_owned());
        self.entries.insert(
            token.to_owned(),
            DeprecatedEntry {
                active_token: active_token.to_owned(),
                serve_until,
            },
        );
    }

    fn remove(&mut self, token: &str) {
        if self.entries.remove(token).is_some() {
            self.order.retain(|held| held != token);
        }
    }
}

/// The key as the auth flow sees it: the key's own columns, with its budget row filling the limits the key
/// leaves unset, and the team, membership, project and organization values it is checked against.
#[derive(Clone, Debug, Serialize)]
pub struct KeyView {
    pub token: String,
    pub key_name: Option<String>,
    pub key_alias: Option<String>,
    pub soft_budget_cooldown: bool,
    pub spend: f64,
    pub total_spend: f64,
    #[serde(with = "utc_timestamp::option")]
    pub expires: Option<NaiveDateTime>,
    pub models: Vec<String>,
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
    pub soft_budget: Option<f64>,
    pub budget_duration: Option<String>,
    #[serde(with = "utc_timestamp::option")]
    pub budget_reset_at: Option<NaiveDateTime>,
    pub allowed_cache_controls: Option<Vec<String>>,
    pub allowed_routes: Option<Vec<String>>,
    pub key_type: Option<String>,
    pub access_group_ids: Option<Vec<String>>,
    pub model_spend: RawJson,
    pub model_max_budget: RawJson,
    pub budget_fallbacks: RawJson,
    pub budget_id: Option<String>,
    pub org_id: Option<String>,
    pub object_permission_id: Option<String>,
    #[serde(with = "utc_timestamp::option")]
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    #[serde(with = "utc_timestamp::option")]
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
    #[serde(with = "utc_timestamp::option")]
    pub settings_updated_at: Option<NaiveDateTime>,
    #[serde(with = "utc_timestamp::option")]
    pub last_active: Option<NaiveDateTime>,
    pub rotation_count: Option<i32>,
    pub auto_rotate: Option<bool>,
    pub rotation_interval: Option<String>,
    #[serde(with = "utc_timestamp::option")]
    pub last_rotation_at: Option<NaiveDateTime>,
    #[serde(with = "utc_timestamp::option")]
    pub key_rotation_at: Option<NaiveDateTime>,
    pub budget_limits: Option<RawJson>,
    pub team_spend: Option<f64>,
    pub team_alias: Option<String>,
    pub team_tpm_limit: Option<i64>,
    pub team_rpm_limit: Option<i64>,
    pub team_tpd_limit: Option<i64>,
    pub team_max_budget: Option<f64>,
    pub team_soft_budget: Option<f64>,
    pub team_model_max_budget: Option<RawJson>,
    pub team_models: Vec<String>,
    pub team_blocked: bool,
    pub team_model_aliases: Option<RawJson>,
    pub team_member: Option<Box<RawValue>>,
    pub team_metadata: Option<RawJson>,
    pub team_object_permission_id: Option<String>,
    pub team_member_spend: Option<f64>,
    pub team_member_tpm_limit: Option<i64>,
    pub team_member_rpm_limit: Option<i64>,
    pub organization_alias: Option<String>,
    pub organization_max_budget: Option<f64>,
    pub organization_tpm_limit: Option<i64>,
    pub organization_rpm_limit: Option<i64>,
    pub organization_metadata: Option<RawJson>,
    pub project_alias: Option<String>,
    pub last_refreshed_at: f64,
}

impl KeyView {
    fn new(row: KeyViewRow, now: SystemTime) -> Self {
        let team_member = row
            .team_members_with_roles
            .as_ref()
            .zip(row.user_id.as_deref())
            .and_then(|(members, user_id)| member_for_user(members, user_id));
        let model_max_budget = match row.litellm_budget_table_model_max_budget {
            Some(budget) if is_empty_object(&row.model_max_budget) => budget,
            _ => row.model_max_budget,
        };
        Self {
            token: row.token,
            key_name: row.key_name,
            key_alias: row.key_alias,
            soft_budget_cooldown: row.soft_budget_cooldown,
            spend: row.spend,
            total_spend: row.total_spend,
            expires: row.expires,
            models: row.models.unwrap_or_default(),
            aliases: row.aliases,
            config: row.config,
            router_settings: row.router_settings,
            user_id: row.user_id,
            team_id: row.team_id,
            agent_id: row.agent_id,
            project_id: row.project_id,
            permissions: row.permissions,
            max_parallel_requests: row.max_parallel_requests,
            metadata: row.metadata,
            blocked: row.blocked,
            tpm_limit: row.tpm_limit.or(row.litellm_budget_table_tpm_limit),
            rpm_limit: row.rpm_limit.or(row.litellm_budget_table_rpm_limit),
            tpd_limit: row.tpd_limit.or(row.litellm_budget_table_tpd_limit),
            max_budget: row.max_budget.or(row.litellm_budget_table_max_budget),
            soft_budget: row.litellm_budget_table_soft_budget,
            budget_duration: row.budget_duration,
            budget_reset_at: row.budget_reset_at,
            allowed_cache_controls: row.allowed_cache_controls,
            allowed_routes: row.allowed_routes,
            key_type: row.key_type,
            access_group_ids: row.access_group_ids,
            model_spend: row.model_spend,
            model_max_budget,
            budget_fallbacks: row.budget_fallbacks,
            budget_id: row.budget_id,
            org_id: row.organization_id.or(row.org_id),
            object_permission_id: row.object_permission_id,
            created_at: row.created_at,
            created_by: row.created_by,
            updated_at: row.updated_at,
            updated_by: row.updated_by,
            settings_updated_at: row.settings_updated_at,
            last_active: row.last_active,
            rotation_count: row.rotation_count,
            auto_rotate: row.auto_rotate,
            rotation_interval: row.rotation_interval,
            last_rotation_at: row.last_rotation_at,
            key_rotation_at: row.key_rotation_at,
            budget_limits: row.budget_limits,
            team_spend: row.team_spend,
            team_alias: row.team_alias,
            team_tpm_limit: row.team_tpm_limit,
            team_rpm_limit: row.team_rpm_limit,
            team_tpd_limit: row.team_tpd_limit,
            team_max_budget: row.team_max_budget,
            team_soft_budget: row.team_soft_budget,
            team_model_max_budget: row.team_model_max_budget,
            team_models: row.team_models.unwrap_or_default(),
            team_blocked: row.team_blocked.unwrap_or(false),
            team_model_aliases: row.team_model_aliases,
            team_member,
            team_metadata: row.team_metadata,
            team_object_permission_id: row.team_object_permission_id,
            team_member_spend: row.team_member_spend,
            team_member_tpm_limit: row.team_member_tpm_limit,
            team_member_rpm_limit: row.team_member_rpm_limit,
            organization_alias: row.organization_alias,
            organization_max_budget: row.organization_max_budget,
            organization_tpm_limit: row.organization_tpm_limit,
            organization_rpm_limit: row.organization_rpm_limit,
            organization_metadata: row.organization_metadata,
            project_alias: row.project_alias,
            last_refreshed_at: now
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        }
    }
}

/// The `user_id` of one `members_with_roles` entry, the only field the lookup reads.
#[derive(Deserialize)]
struct MemberUserId {
    user_id: Option<String>,
}

/// The last `members_with_roles` entry for `user_id`, as stored. A column that is not a list holds no members.
fn member_for_user(members: &RawJson, user_id: &str) -> Option<Box<RawValue>> {
    let entries: Vec<Box<RawValue>> = serde_json::from_str(members.get()).ok()?;
    entries.into_iter().rev().find(|entry| {
        serde_json::from_str::<MemberUserId>(entry.get())
            .is_ok_and(|member| member.user_id.as_deref() == Some(user_id))
    })
}

fn is_empty_object(value: &RawJson) -> bool {
    serde_json::from_str::<BTreeMap<String, IgnoredAny>>(value.get())
        .is_ok_and(|object| object.is_empty())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use chrono::{DateTime, NaiveDateTime, Utc};
    use litellm_db::{KeyViewRow, RawJson};
    use rstest::rstest;
    use serde_json::value::RawValue;

    use super::{
        DEPRECATED_KEY_CAPACITY, DeprecatedKeys, KeyView, is_empty_object, member_for_user,
    };

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds)
    }

    fn naive(time: SystemTime) -> NaiveDateTime {
        DateTime::<Utc>::from(time).naive_utc()
    }

    fn raw(text: &str) -> RawJson {
        RawJson(RawValue::from_string(text.to_owned()).expect("valid JSON"))
    }

    fn key_row(organization_id: Option<&str>, team_org_id: Option<&str>) -> KeyViewRow {
        KeyViewRow {
            token: "hashed-token".to_owned(),
            key_name: None,
            key_alias: None,
            soft_budget_cooldown: false,
            spend: 0.0,
            total_spend: 0.0,
            expires: None,
            models: None,
            aliases: raw("{}"),
            config: raw("{}"),
            router_settings: None,
            user_id: None,
            team_id: None,
            agent_id: None,
            project_id: None,
            permissions: raw("{}"),
            max_parallel_requests: None,
            metadata: raw("{}"),
            blocked: None,
            tpm_limit: None,
            rpm_limit: None,
            tpd_limit: None,
            max_budget: None,
            budget_duration: None,
            budget_reset_at: None,
            allowed_cache_controls: None,
            allowed_routes: None,
            key_type: None,
            policies: None,
            access_group_ids: None,
            model_spend: raw("{}"),
            model_max_budget: raw("{}"),
            budget_fallbacks: raw("{}"),
            budget_id: None,
            organization_id: organization_id.map(str::to_owned),
            object_permission_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            settings_updated_at: None,
            last_active: None,
            rotation_count: None,
            auto_rotate: None,
            rotation_interval: None,
            last_rotation_at: None,
            key_rotation_at: None,
            budget_limits: None,
            team_spend: None,
            team_max_budget: None,
            team_soft_budget: None,
            team_model_max_budget: None,
            team_tpm_limit: None,
            team_rpm_limit: None,
            team_tpd_limit: None,
            team_models: None,
            team_metadata: None,
            team_blocked: None,
            team_alias: None,
            team_members_with_roles: None,
            team_object_permission_id: None,
            org_id: team_org_id.map(str::to_owned),
            project_alias: None,
            team_member_spend: None,
            team_member_tpm_limit: None,
            team_member_rpm_limit: None,
            team_model_aliases: None,
            litellm_budget_table_max_budget: None,
            litellm_budget_table_tpm_limit: None,
            litellm_budget_table_rpm_limit: None,
            litellm_budget_table_tpd_limit: None,
            litellm_budget_table_model_max_budget: None,
            litellm_budget_table_soft_budget: None,
            organization_metadata: None,
            organization_alias: None,
            organization_max_budget: None,
            organization_tpm_limit: None,
            organization_rpm_limit: None,
        }
    }

    #[rstest]
    #[case::fresh(at(30), Some("active"))]
    #[case::memory_ttl_elapsed(at(60), None)]
    fn deprecated_mapping_is_served_for_sixty_seconds(
        #[case] now: SystemTime,
        #[case] expected: Option<&str>,
    ) {
        let mut keys = DeprecatedKeys::default();
        keys.insert("old", "active", at(0), naive(at(3600)));

        assert_eq!(keys.active_token("old", now).as_deref(), expected);
    }

    #[rstest]
    fn deprecated_mapping_stops_at_revoke_time() {
        let mut keys = DeprecatedKeys::default();
        keys.insert("old", "active", at(0), naive(at(10)));

        assert_eq!(
            (
                keys.active_token("old", at(9)).as_deref(),
                keys.active_token("old", at(10))
            ),
            (Some("active"), None)
        );
    }

    #[rstest]
    fn oldest_deprecated_mapping_is_evicted_at_capacity() {
        let mut keys = DeprecatedKeys::default();
        for index in 0..=DEPRECATED_KEY_CAPACITY {
            keys.insert(&format!("old-{index}"), "active", at(0), naive(at(3600)));
        }

        assert_eq!(
            (
                keys.active_token("old-0", at(1)),
                keys.active_token(&format!("old-{DEPRECATED_KEY_CAPACITY}"), at(1))
                    .as_deref(),
                keys.entries.len(),
            ),
            (None, Some("active"), DEPRECATED_KEY_CAPACITY)
        );
    }

    #[rstest]
    #[case::last_match_wins(
        r#"[{"user_id": "u", "role": "user"}, {"user_id": "v", "role": "admin"}, {"user_id": "u", "role": "admin"}]"#,
        Some(r#"{"user_id": "u", "role": "admin"}"#)
    )]
    #[case::no_match(r#"[{"user_id": "v", "role": "admin"}]"#, None)]
    #[case::not_a_list("{}", None)]
    fn team_member_is_the_last_entry_for_the_key_user(
        #[case] members: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(
            member_for_user(&raw(members), "u")
                .as_deref()
                .map(RawValue::get),
            expected
        );
    }

    #[rstest]
    #[case::empty_object("{}", true)]
    #[case::object(r#"{"gpt-4o": 1}"#, false)]
    #[case::list("[]", false)]
    fn only_an_empty_object_takes_the_budget_model_max_budget(
        #[case] value: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(is_empty_object(&raw(value)), expected);
    }

    #[rstest]
    #[case::key_organization(Some("key-org"), Some("team-org"), Some("key-org"))]
    #[case::team_organization(None, Some("team-org"), Some("team-org"))]
    #[case::no_organization(None, None, None)]
    fn key_organization_falls_back_to_the_team_organization(
        #[case] organization_id: Option<&str>,
        #[case] team_org_id: Option<&str>,
        #[case] expected: Option<&str>,
    ) {
        let view = KeyView::new(key_row(organization_id, team_org_id), at(0));

        assert_eq!(view.org_id.as_deref(), expected);
    }
}
