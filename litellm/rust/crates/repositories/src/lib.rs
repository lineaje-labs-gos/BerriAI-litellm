//! Rust repositories for the auth objects the proxy reads from Postgres when its auth cache misses.
//!
//! Each repository owns its queries, the shaping of their rows into what the auth flow consumes, and any memory
//! tier private to that object. The shared auth cache in front of them stays with the host.

mod database;
mod jwt_key_mapping;
mod key;
mod model_access_group;
mod organization;
mod team;
mod team_membership;

pub use database::{Database, Side};
pub use jwt_key_mapping::JwtKeyMappingRepository;
pub use key::{KeyRepository, KeyView};
pub use model_access_group::{ModelAccessGroupBudget, ModelAccessGroupRepository};
pub use organization::OrganizationRepository;
pub use team::TeamRepository;
pub use team_membership::TeamMembershipRepository;
