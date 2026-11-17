//! The budget repository: `LiteLLM_BudgetTable` rows read by id, for an end user's default budget and a team's
//! default per-member budget

use litellm_db::{Budget, Call};

use crate::{Database, Side};

const FIND_BUDGET: Call = Call {
    call_type: "find_unique",
    table: Some("LiteLLM_BudgetTable"),
    operation: Some("select"),
};

const FIND_TEAM_MEMBER_DEFAULT_BUDGET: Call = Call {
    call_type: "get_team_member_default_budget",
    table: None,
    operation: None,
};

/// Reads `LiteLLM_BudgetTable`.
pub struct BudgetRepository {
    database: Database,
}

impl BudgetRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    /// The budget row `budget_id`, reported as the Prisma `find_unique` it replaces.
    pub async fn find(&self, budget_id: &str) -> Result<Option<Budget>, litellm_db::Error> {
        self.find_as(FIND_BUDGET, budget_id).await
    }

    /// The budget row `budget_id`, reported as the team member default budget lookup it serves.
    pub async fn find_team_member_default(
        &self,
        budget_id: &str,
    ) -> Result<Option<Budget>, litellm_db::Error> {
        self.find_as(FIND_TEAM_MEMBER_DEFAULT_BUDGET, budget_id)
            .await
    }

    async fn find_as(
        &self,
        call: Call,
        budget_id: &str,
    ) -> Result<Option<Budget>, litellm_db::Error> {
        self.database
            .run(call, || async {
                let mut session = self.database.session(Side::Read).await?;
                litellm_db::find_budget(session.executor(), budget_id).await
            })
            .await
    }
}
