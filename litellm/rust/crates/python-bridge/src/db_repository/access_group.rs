use std::sync::Arc;

use litellm_repositories::{AccessGroupRepository, Side};
use pyo3::prelude::*;

use crate::db::{NativeDatabase, map_error};

/// The access group repository. `find` answers an access group as JSON text, or `None`.
#[pyclass(frozen)]
pub(crate) struct RustAccessGroupRepository(Arc<AccessGroupRepository>);

#[pymethods]
impl RustAccessGroupRepository {
    #[new]
    fn new(database: &NativeDatabase) -> PyResult<Self> {
        Ok(Self(Arc::new(AccessGroupRepository::new(
            database.repository_database()?,
        ))))
    }

    #[pyo3(signature = (access_group_id, *, use_writer=false))]
    fn find<'py>(
        &self,
        py: Python<'py>,
        access_group_id: String,
        use_writer: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let repository = Arc::clone(&self.0);
        let side = if use_writer { Side::Writer } else { Side::Read };
        crate::execution::run_async_value(
            py,
            crate::db::for_caller(py, async move {
                let group = repository
                    .find(&access_group_id, side)
                    .await
                    .map_err(map_error)?;
                super::json(group.as_ref())
            })?,
        )
    }
}
