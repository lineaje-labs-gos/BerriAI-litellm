//! The user repository: resolves a sign-in to its `LiteLLM_UserTable` row by id, then by SSO id, then by email,
//! linking the SSO id the way the Python `get_user_object` did, and backfills a missing email.

use std::sync::Arc;

use litellm_db::{Call, User, UserRecord};
use serde::Deserialize;

use crate::{Database, Side};

const GET_USER_OBJECT: Call = Call {
    call_type: "get_user_object",
    table: None,
    operation: None,
};

const FIND_FIRST_USER: Call = Call {
    call_type: "find_first",
    table: Some("LiteLLM_UserTable"),
    operation: Some("select"),
};

const UPDATE_MANY_USERS: Call = Call {
    call_type: "update_many",
    table: Some("LiteLLM_UserTable"),
    operation: Some("update"),
};

const UPDATE_USER: Call = Call {
    call_type: "update",
    table: Some("LiteLLM_UserTable"),
    operation: Some("update"),
};

const FIND_UNIQUE_USER: Call = Call {
    call_type: "find_unique",
    table: Some("LiteLLM_UserTable"),
    operation: Some("select"),
};

/// Who is signing in: the requested id plus the sign-in's SSO id and email. `links_any_user` is
/// `LITELLM_EMAIL_FALLBACK_LINKS_ANY_USER`, which lets an email match any row and skips the SSO link guards.
#[derive(Clone, Debug, Deserialize)]
pub struct SignIn {
    pub user_id: String,
    pub sso_user_id: Option<String>,
    pub user_email: Option<String>,
    pub email_verified: Option<bool>,
    pub links_any_user: bool,
}

/// Reads and links `LiteLLM_UserTable` rows.
pub struct UserRepository {
    database: Arc<Database>,
}

impl UserRepository {
    pub fn new(database: Database) -> Self {
        Self {
            database: Arc::new(database),
        }
    }

    /// The user `sign_in` resolves to, with its organization memberships, or `None` when no row matches.
    /// `side` is where the id lookup reads, the writer for callers that must see their own writes.
    pub async fn resolve(
        &self,
        sign_in: &SignIn,
        side: Side,
    ) -> Result<Option<UserRecord>, litellm_db::Error> {
        let user = match self.find_by_id(side, &sign_in.user_id).await? {
            Some(user) => {
                self.link_own_sign_in_id(&user, sign_in).await?;
                Some(user)
            }
            None => self.find_by_sign_in(sign_in).await?,
        };
        let Some(user) = user else {
            return Ok(None);
        };
        let organization_memberships = self
            .database
            .run(GET_USER_OBJECT, || async {
                let mut session = self.database.session(side).await?;
                litellm_db::find_organization_memberships(session.executor(), &user.user_id).await
            })
            .await?;
        Ok(Some(UserRecord {
            user,
            organization_memberships,
        }))
    }

    /// Whether a user other than `user_id` holds `user_email`, ignoring case.
    pub async fn email_belongs_to_another_user(
        &self,
        user_email: &str,
        user_id: &str,
    ) -> Result<bool, litellm_db::Error> {
        self.database
            .run(FIND_FIRST_USER, || async {
                let mut session = self.database.session(Side::Read).await?;
                litellm_db::email_belongs_to_another_user(session.executor(), user_email, user_id)
                    .await
            })
            .await
    }

    /// Sets `user_email` on a user with none, unless another user holds it. Returns the email now stored, or
    /// `None` when another user holds it or the row is gone.
    pub async fn backfill_email(
        &self,
        user_id: &str,
        user_email: &str,
        links_any_user: bool,
    ) -> Result<Option<String>, litellm_db::Error> {
        if !links_any_user
            && self
                .email_belongs_to_another_user(user_email, user_id)
                .await?
        {
            return Ok(None);
        }
        self.database
            .run(UPDATE_MANY_USERS, || async {
                let mut session = self.database.session(Side::Writer).await?;
                litellm_db::backfill_null_user_email(session.executor(), user_id, user_email).await
            })
            .await?;
        self.database
            .run(FIND_UNIQUE_USER, || async {
                let mut session = self.database.session(Side::Writer).await?;
                litellm_db::find_user_email(session.executor(), user_id).await
            })
            .await
    }

    async fn find_by_id(
        &self,
        side: Side,
        user_id: &str,
    ) -> Result<Option<User>, litellm_db::Error> {
        self.database
            .run(GET_USER_OBJECT, || async {
                let mut session = self.database.session(side).await?;
                litellm_db::find_user_by_id(session.executor(), user_id).await
            })
            .await
    }

    async fn find_by_sso_id(
        &self,
        side: Side,
        sso_user_id: &str,
    ) -> Result<Option<User>, litellm_db::Error> {
        self.database
            .run(GET_USER_OBJECT, || async {
                let mut session = self.database.session(side).await?;
                litellm_db::find_user_by_sso_id(session.executor(), sso_user_id).await
            })
            .await
    }

    async fn find_by_email(
        &self,
        user_email: &str,
        unlinked_only: bool,
    ) -> Result<Option<User>, litellm_db::Error> {
        self.database
            .run(GET_USER_OBJECT, || async {
                let mut session = self.database.session(Side::Read).await?;
                litellm_db::find_user_by_email(session.executor(), user_email, unlinked_only).await
            })
            .await
    }

    async fn link_null_sso_user_id(
        &self,
        user_id: &str,
        sso_user_id: &str,
    ) -> Result<bool, litellm_db::Error> {
        self.database
            .run(GET_USER_OBJECT, || async {
                let mut session = self.database.session(Side::Writer).await?;
                litellm_db::link_null_sso_user_id(session.executor(), user_id, sso_user_id).await
            })
            .await
    }

    /// Links the requested id as its own SSO id when the sign-in carries it and the row has none yet.
    async fn link_own_sign_in_id(
        &self,
        user: &User,
        sign_in: &SignIn,
    ) -> Result<(), litellm_db::Error> {
        if user.sso_user_id.is_some()
            || sign_in.sso_user_id.as_deref() != Some(user.user_id.as_str())
            || sign_in.links_any_user
        {
            return Ok(());
        }
        self.link_null_sso_user_id(&user.user_id, &user.user_id)
            .await
            .map(|_| ())
    }

    /// The row an email match links to `sso_user_id`, or whichever row another sign-in linked to it first.
    async fn link_by_email(
        &self,
        user: Option<User>,
        sso_user_id: &str,
    ) -> Result<Option<User>, litellm_db::Error> {
        if let Some(mut user) = user
            && self
                .link_null_sso_user_id(&user.user_id, sso_user_id)
                .await?
        {
            user.sso_user_id = Some(sso_user_id.to_owned());
            return Ok(Some(user));
        }
        self.find_by_sso_id(Side::Writer, sso_user_id).await
    }

    /// Records `sso_user_id` on `user_id` in the background, as Python did for an email match under
    /// `links_any_user`; the sign-in does not wait for it.
    fn set_sso_user_id_later(&self, user_id: String, sso_user_id: String) {
        let database = Arc::clone(&self.database);
        tokio::spawn(async move {
            let outcome = database
                .run(UPDATE_USER, || async {
                    let mut session = database.session(Side::Writer).await?;
                    litellm_db::set_sso_user_id(session.executor(), &user_id, &sso_user_id).await
                })
                .await;
            if let Err(error) = outcome {
                tracing::warn!(
                    error = %litellm_error::Report::new(&error),
                    "linking an SSO id to a user matched by email failed"
                );
            }
        });
    }

    /// A user with no row for the requested id, found by SSO id or, unless the email is unverified, by email.
    async fn find_by_sign_in(&self, sign_in: &SignIn) -> Result<Option<User>, litellm_db::Error> {
        let by_sso_id = match &sign_in.sso_user_id {
            Some(sso_user_id) => self.find_by_sso_id(Side::Read, sso_user_id).await?,
            None => None,
        };
        let Some(user_email) = &sign_in.user_email else {
            return Ok(by_sso_id);
        };
        if by_sso_id.is_some() || !(sign_in.links_any_user || sign_in.email_verified != Some(false))
        {
            return Ok(by_sso_id);
        }
        let unlinked_only = !sign_in.links_any_user && sign_in.sso_user_id.is_some();
        let by_email = self.find_by_email(user_email, unlinked_only).await?;
        match (&sign_in.sso_user_id, sign_in.links_any_user) {
            (Some(sso_user_id), false) => self.link_by_email(by_email, sso_user_id).await,
            (Some(sso_user_id), true) => {
                if let Some(found) = &by_email {
                    self.set_sso_user_id_later(found.user_id.clone(), sso_user_id.clone());
                }
                Ok(by_email)
            }
            (None, _) => Ok(by_email),
        }
    }
}
