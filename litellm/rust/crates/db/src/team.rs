//! Reads `LiteLLM_TeamTable` rows for auth, by id or by alias, with each team's model aliases from `LiteLLM_ModelTable`.

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgExecutor;

use crate::{Error, RawJson};

/// The team's `LiteLLM_ModelTable` row, which holds its model aliases.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TeamModelAliases {
    pub id: i32,
    pub model_aliases: Option<RawJson>,
    #[serde(with = "crate::utc_timestamp")]
    pub created_at: NaiveDateTime,
    pub created_by: String,
    #[serde(with = "crate::utc_timestamp")]
    pub updated_at: NaiveDateTime,
    pub updated_by: String,
}

/// One `LiteLLM_TeamTable` row as auth reads it, with its JSON columns kept opaque for the host's codec.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Team {
    pub team_id: String,
    pub team_alias: Option<String>,
    pub organization_id: Option<String>,
    pub admins: Vec<String>,
    pub members: Vec<String>,
    pub members_with_roles: RawJson,
    pub metadata: RawJson,
    pub max_budget: Option<f64>,
    pub soft_budget: Option<f64>,
    pub spend: f64,
    pub models: Vec<String>,
    pub max_parallel_requests: Option<i32>,
    pub tpm_limit: Option<i64>,
    pub rpm_limit: Option<i64>,
    pub tpd_limit: Option<i64>,
    pub budget_duration: Option<String>,
    #[serde(with = "crate::utc_timestamp::option")]
    pub budget_reset_at: Option<NaiveDateTime>,
    pub blocked: bool,
    #[serde(with = "crate::utc_timestamp")]
    pub created_at: NaiveDateTime,
    #[serde(with = "crate::utc_timestamp")]
    pub updated_at: NaiveDateTime,
    pub model_spend: RawJson,
    pub model_max_budget: RawJson,
    pub router_settings: Option<RawJson>,
    pub team_member_permissions: Vec<String>,
    pub access_group_ids: Vec<String>,
    pub policies: Vec<String>,
    pub default_team_member_models: Vec<String>,
    pub budget_limits: Option<RawJson>,
    pub model_id: Option<i32>,
    pub allow_team_guardrail_config: bool,
    pub object_permission_id: Option<String>,
    pub litellm_model_table: Option<TeamModelAliases>,
}

#[macro_rules_attribute::apply(crate::db_row)]
struct TeamRow {
    team_id: String,
    team_alias: Option<String>,
    organization_id: Option<String>,
    admins: Vec<String>,
    members: Vec<String>,
    members_with_roles: RawJson,
    metadata: RawJson,
    max_budget: Option<f64>,
    soft_budget: Option<f64>,
    spend: f64,
    models: Vec<String>,
    max_parallel_requests: Option<i32>,
    tpm_limit: Option<i64>,
    rpm_limit: Option<i64>,
    tpd_limit: Option<i64>,
    budget_duration: Option<String>,
    budget_reset_at: Option<NaiveDateTime>,
    blocked: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    model_spend: RawJson,
    model_max_budget: RawJson,
    router_settings: Option<RawJson>,
    team_member_permissions: Vec<String>,
    access_group_ids: Vec<String>,
    policies: Vec<String>,
    default_team_member_models: Vec<String>,
    budget_limits: Option<RawJson>,
    model_id: Option<i32>,
    allow_team_guardrail_config: bool,
    object_permission_id: Option<String>,
    aliases_id: Option<i32>,
    aliases: Option<RawJson>,
    aliases_created_at: Option<NaiveDateTime>,
    aliases_created_by: Option<String>,
    aliases_updated_at: Option<NaiveDateTime>,
    aliases_updated_by: Option<String>,
}

impl TeamRow {
    fn model_aliases(&mut self) -> Option<TeamModelAliases> {
        Some(TeamModelAliases {
            id: self.aliases_id?,
            model_aliases: self.aliases.take(),
            created_at: self.aliases_created_at?,
            created_by: self.aliases_created_by.take()?,
            updated_at: self.aliases_updated_at?,
            updated_by: self.aliases_updated_by.take()?,
        })
    }
}

impl From<TeamRow> for Team {
    fn from(mut row: TeamRow) -> Self {
        let litellm_model_table = row.model_aliases();
        Team {
            team_id: row.team_id,
            team_alias: row.team_alias,
            organization_id: row.organization_id,
            admins: row.admins,
            members: row.members,
            members_with_roles: row.members_with_roles,
            metadata: row.metadata,
            max_budget: row.max_budget,
            soft_budget: row.soft_budget,
            spend: row.spend,
            models: row.models,
            max_parallel_requests: row.max_parallel_requests,
            tpm_limit: row.tpm_limit,
            rpm_limit: row.rpm_limit,
            tpd_limit: row.tpd_limit,
            budget_duration: row.budget_duration,
            budget_reset_at: row.budget_reset_at,
            blocked: row.blocked,
            created_at: row.created_at,
            updated_at: row.updated_at,
            model_spend: row.model_spend,
            model_max_budget: row.model_max_budget,
            router_settings: row.router_settings,
            team_member_permissions: row.team_member_permissions,
            access_group_ids: row.access_group_ids,
            policies: row.policies,
            default_team_member_models: row.default_team_member_models,
            budget_limits: row.budget_limits,
            model_id: row.model_id,
            allow_team_guardrail_config: row.allow_team_guardrail_config,
            object_permission_id: row.object_permission_id,
            litellm_model_table,
        }
    }
}

pub async fn find_team(
    executor: impl PgExecutor<'_>,
    team_id: &str,
) -> Result<Option<Team>, Error> {
    let row = sqlx::query_file_as!(TeamRow, "queries/team/find_team.sql", team_id)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(Team::from))
}

/// Up to two teams whose alias is `team_alias`, with their model aliases, so a caller can tell a unique alias
/// from a shared one.
pub async fn find_teams_by_alias(
    executor: impl PgExecutor<'_>,
    team_alias: &str,
) -> Result<Vec<Team>, Error> {
    let rows = sqlx::query_file_as!(TeamRow, "queries/team/find_teams_by_alias.sql", team_alias)
        .fetch_all(executor)
        .await?;
    Ok(rows.into_iter().map(Team::from).collect())
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{find_team, find_teams_by_alias};
    use crate::test_support::database_pool;
    use rstest::rstest;
    use sqlx::PgPool;

    #[rstest]
    #[case::with_model_aliases(true)]
    #[case::without_model_aliases(false)]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_team_with_its_model_aliases(
        #[case] with_aliases: bool,
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let model_id: Option<i32> = if with_aliases {
            Some(
                sqlx::query_scalar(
                    r#"INSERT INTO "LiteLLM_ModelTable" (aliases, created_by, updated_by)
                       VALUES ('{"gpt": "gpt-4o"}', 'a', 'b') RETURNING id"#,
                )
                .fetch_one(&mut *tx)
                .await?,
            )
        } else {
            None
        };
        #[expect(
            clippy::disallowed_methods,
            reason = "Fixture writes are outside the compile-checked query set"
        )]
        let _inserted = sqlx::query(
            r#"INSERT INTO "LiteLLM_TeamTable"
                   (team_id, team_alias, models, members_with_roles, metadata, tpm_limit,
                    budget_reset_at, created_at, updated_at, model_id, admins)
               VALUES ('find-team', 'alias', '{gpt-4o}', '[{"role": "admin", "user_id": "u"}]',
                       '{"k": 1}', 9007199254740993, '2026-01-02 03:04:05.678',
                       '2026-01-01', '2026-01-01', $1, NULL)"#,
        )
        .bind(model_id)
        .execute(&mut *tx)
        .await?;

        let team = find_team(&mut *tx, "find-team")
            .await?
            .ok_or("team missing")?;

        assert_eq!(team.team_alias.as_deref(), Some("alias"));
        assert_eq!(team.models, ["gpt-4o"]);
        assert!(team.admins.is_empty());
        assert_eq!(team.tpm_limit, Some(9_007_199_254_740_993));
        assert_eq!(
            team.members_with_roles.get(),
            r#"[{"role": "admin", "user_id": "u"}]"#
        );
        assert_eq!(team.model_id, model_id);
        assert_eq!(
            team.litellm_model_table
                .map(|aliases| aliases.model_aliases.map(|json| json.get().to_owned())),
            with_aliases.then(|| Some(r#"{"gpt": "gpt-4o"}"#.to_owned()))
        );
        let encoded = serde_json::to_value(
            find_team(&mut *tx, "find-team")
                .await?
                .ok_or("team missing")?,
        )?;
        assert_eq!(encoded["budget_reset_at"], "2026-01-02T03:04:05.678+00:00");
        assert_eq!(encoded["created_at"], "2026-01-01T00:00:00+00:00");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn missing_team_is_none(
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        assert!(find_team(&pool, "no-such-team").await?.is_none());
        Ok(())
    }

    #[rstest]
    #[case::unique("unique-alias", &["by-alias-a"])]
    #[case::shared("shared-alias", &["by-alias-b", "by-alias-c"])]
    #[case::missing("no-such-alias", &[])]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to a Prisma-migrated PostgreSQL database"]
    async fn reads_teams_by_alias(
        #[case] alias: &str,
        #[case] expected: &[&str],
        #[future(awt)] database_pool: Result<PgPool, Box<dyn StdError>>,
    ) -> Result<(), Box<dyn StdError>> {
        let pool = database_pool?;
        let mut tx = pool.begin().await?;
        for (team_id, team_alias) in [
            ("by-alias-a", "unique-alias"),
            ("by-alias-b", "shared-alias"),
            ("by-alias-c", "shared-alias"),
            ("by-alias-d", "shared-alias"),
        ] {
            #[expect(
                clippy::disallowed_methods,
                reason = "Fixture writes are outside the compile-checked query set"
            )]
            let _inserted = sqlx::query(
                r#"INSERT INTO "LiteLLM_TeamTable" (team_id, team_alias, created_at, updated_at)
                   VALUES ($1, $2, '2026-01-01', '2026-01-01')"#,
            )
            .bind(team_id)
            .bind(team_alias)
            .execute(&mut *tx)
            .await?;
        }

        let teams = find_teams_by_alias(&mut *tx, alias).await?;

        assert_eq!(
            teams
                .iter()
                .map(|team| team.team_id.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        Ok(())
    }
}
