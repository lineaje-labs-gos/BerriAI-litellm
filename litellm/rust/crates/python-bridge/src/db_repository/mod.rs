//! Python handles on the Rust auth repositories. Each wraps one repository built over a `NativeDatabase`'s pools and
//! hands its answers to Python as plain values or JSON text for `model_validate_json`.

mod access_group;
mod jwt_key_mapping;
mod key;
mod model_access_group;
mod object_permission;
mod project;
mod tag;

pub(crate) use access_group::RustAccessGroupRepository;
pub(crate) use jwt_key_mapping::RustJwtKeyMappingRepository;
pub(crate) use key::RustKeyRepository;
pub(crate) use model_access_group::RustModelAccessGroupRepository;
pub(crate) use object_permission::RustObjectPermissionRepository;
pub(crate) use project::RustProjectRepository;
pub(crate) use tag::RustTagRepository;

fn json<T: serde::Serialize>(value: Option<&T>) -> pyo3::PyResult<Option<String>> {
    value.map(litellm_host_python::json_dumps).transpose()
}
