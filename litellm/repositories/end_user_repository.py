"""
End user repository for database operations on LiteLLM_EndUserTable.
"""

from typing import TYPE_CHECKING, Final

from litellm.models.end_user import LiteLLM_EndUserTable
from litellm.repositories import rust_database
from litellm.repositories.table_repositories import PrismaTableRepository
from litellm.rust_bridge._native import RustEndUserRepository

if TYPE_CHECKING:
    from prisma import models as prisma_models  # noqa: F401  # used by quoted base-class subscripts


class EndUserRepository(PrismaTableRepository["prisma_models.LiteLLM_EndUserTable"]):
    table_name = "litellm_endusertable"

    async def find_for_auth(self, end_user_id: str) -> LiteLLM_EndUserTable | None:
        """The end user with its budget and object permission, read by the Rust repository."""
        payload: Final = await rust_database.required_repository(RustEndUserRepository).find_end_user(end_user_id)
        return rust_database.decode(LiteLLM_EndUserTable, payload)

    async def restricted_ids(self, limit: int) -> tuple[str, ...]:
        """Ids of end users whose row restricts something auth enforces, at most ``limit`` of them."""
        return tuple(await rust_database.required_repository(RustEndUserRepository).restricted_end_user_ids(limit))
