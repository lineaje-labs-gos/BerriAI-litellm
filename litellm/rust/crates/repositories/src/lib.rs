//! Rust repositories for the auth objects the proxy reads from Postgres when its auth cache misses.
//!
//! Each repository owns its queries, the shaping of their rows into what the auth flow consumes, and any memory
//! tier private to that object. The shared auth cache in front of them stays with the host.

mod access_group;
mod database;
mod jwt_key_mapping;
mod key;
mod model_access_group;
mod object_permission;
mod project;
mod tag;

pub use access_group::AccessGroupRepository;
pub use database::{Database, Side};
pub use jwt_key_mapping::JwtKeyMappingRepository;
pub use key::{KeyRepository, KeyView};
pub use model_access_group::{ModelAccessGroupBudget, ModelAccessGroupRepository};
pub use object_permission::ObjectPermissionRepository;
pub use project::ProjectRepository;
pub use tag::TagRepository;
