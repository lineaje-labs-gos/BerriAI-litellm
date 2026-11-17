//! The end user repository: one end user with its budget and object permission, and the bounded set of end users
//! whose row restricts something auth enforces.

use litellm_db::{Call, EndUserRecord};

use crate::{Database, Side};

const GET_END_USER_OBJECT: Call = Call {
    call_type: "get_end_user_object",
    table: None,
    operation: None,
};

/// Reads `LiteLLM_EndUserTable`.
pub struct EndUserRepository {
    database: Database,
}

impl EndUserRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    /// The end user `end_user_id` with `litellm_budget_table` and `object_permission`, or `None`.
    pub async fn find(
        &self,
        end_user_id: &str,
    ) -> Result<Option<EndUserRecord>, litellm_db::Error> {
        self.database
            .run(GET_END_USER_OBJECT, || async {
                let mut session = self.database.session(Side::Read).await?;
                litellm_db::find_end_user_record(session.executor(), end_user_id).await
            })
            .await
    }

    /// Ids of end users that restrict something, at most `limit` of them.
    pub async fn restricted_ids(&self, limit: i64) -> Result<Vec<String>, litellm_db::Error> {
        self.database
            .run(GET_END_USER_OBJECT, || async {
                let mut session = self.database.session(Side::Read).await?;
                litellm_db::find_restricted_end_user_ids(session.executor(), limit).await
            })
            .await
    }
}
