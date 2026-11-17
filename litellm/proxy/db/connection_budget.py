"""Split each Postgres pool's connection budget between Prisma and the Rust core's SQLx pool.

Migration scaffolding. Prisma and SQLx share a database only while the Python gateway still runs Prisma
queries next to ported ones. These invariants hold until Prisma is gone:

- Operators set one `connection_limit` per pool (`database_connection_pool_limit`, a URL pin or
  `database_extra_connection_params`). Nothing an operator sets names Prisma, SQLx or Rust. The Rust share
  is carved out of that limit, never added on top, so Prisma and the Rust pool together never open more
  connections than the limit.
- Prisma always keeps at least one connection. The Rust share is the hard-coded `RUST_RESERVED_CONNECTIONS`,
  sized for the auth lookups that read through the Rust stores on a cache miss, cut down to what is left
  after Prisma's one. A limit of 1 leaves the Rust pool nothing, so no Rust pool is opened beside it.
- Only a positive integer limit is split. Any other value goes to Prisma as it is, which judges it the same
  way it did before the split existed.
- The split is Python's. It is about Prisma and Prisma lives only in the Python gateway, so this module is
  deleted with Prisma. The CLI hands the worker the smallest Rust share among its pools through
  `RUST_CONNECTIONS_ENV_VAR`, and a worker started without it reserves the full `RUST_RESERVED_CONNECTIONS`.
- The Rust core takes a plain `max_connections` and never learns about the split. That is already its final
  interface, since the Rust gateway will pass `database_connection_pool_limit` straight to SQLx.
"""

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from types import MappingProxyType
from typing import Final

RUST_RESERVED_CONNECTIONS: Final = 2
RUST_CONNECTIONS_ENV_VAR: Final = "LITELLM_RUST_DATABASE_CONNECTIONS"


@dataclass(frozen=True, slots=True)
class ConnectionBudget:
    total: int
    python: int
    rust: int

    @classmethod
    def split(cls, total: int, rust_reserved: int = RUST_RESERVED_CONNECTIONS) -> "ConnectionBudget":
        """Prisma keeps at least one connection, and the Rust pool takes what is left of its reserve."""
        rust: Final = max(0, min(rust_reserved, total - 1))
        return cls(total=total, python=total - rust, rust=rust)

    @classmethod
    def of(
        cls, connection_limit: str | float, rust_reserved: int = RUST_RESERVED_CONNECTIONS
    ) -> "ConnectionBudget | None":
        """The split of a positive integer limit, or None for any other value Prisma is left to judge."""
        digits: Final = str(connection_limit)
        if not (digits.isascii() and digits.isdigit()) or int(digits) == 0:
            return None
        return cls.split(int(digits), rust_reserved)

    @property
    def prisma_limit(self) -> int | None:
        """Prisma's share once the Rust pool takes some of the total, or None to leave Prisma's limit untouched."""
        return None if self.rust == 0 else self.python

    def prisma_params(self, params: Mapping[str, str | int | float]) -> Mapping[str, str | int | float]:
        """``params`` with Prisma's share as its ``connection_limit``, untouched while the Rust pool takes none."""
        limit: Final = self.prisma_limit
        return params if limit is None else MappingProxyType({**params, "connection_limit": limit})


def shared_rust_connections(budgets: Iterable["ConnectionBudget | None"]) -> int | None:
    """The Rust share every split pool can afford, or None when no pool's limit was split."""
    return min((budget.rust for budget in budgets if budget is not None), default=None)


def rust_connections(environ: Mapping[str, str]) -> int:
    """The Rust pool size the CLI's split left this worker, or the full reserve when the CLI set none."""
    share: Final = environ.get(RUST_CONNECTIONS_ENV_VAR, "")
    return int(share) if share.isascii() and share.isdigit() else RUST_RESERVED_CONNECTIONS
