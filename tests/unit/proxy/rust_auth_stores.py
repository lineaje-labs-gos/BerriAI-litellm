"""In-memory stand-ins for the Rust auth repositories, patched in through ``rust_database.repository``.

Each fake keeps separate reader and writer rows (a writer read sees ``writer`` when it is set, else ``rows``),
logs every lookup, and raises a row that is an exception. Threads porting more auth objects add their fake as
another field of ``RustAuthStores``.
"""

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Final

import pytest
from pydantic import BaseModel

from litellm.proxy._types import (
    LiteLLM_OrganizationTable,
    LiteLLM_TeamMembership,
    LiteLLM_TeamTable,
    LiteLLM_VerificationTokenView,
)
from litellm.repositories import rust_database
from litellm.rust_bridge._native import (
    RustJwtKeyMappingRepository,
    RustKeyRepository,
    RustModelAccessGroupRepository,
    RustOrganizationRepository,
    RustTeamMembershipRepository,
    RustTeamRepository,
)


@dataclass
class FakeKeyStore:
    """``combined_view`` rows by hash, written the way the Prisma row was, shaped through the view model."""

    rows: dict[str, Mapping[str, object] | Exception] = field(default_factory=dict)
    writer: dict[str, Mapping[str, object] | Exception] | None = None
    lookups: list[tuple[str, bool]] = field(default_factory=list)

    async def find(self, token: str, *, use_writer: bool = False) -> str | None:
        self.lookups.append((token, use_writer))
        source = self.writer if use_writer and self.writer is not None else self.rows
        row = source.get(token)
        if isinstance(row, Exception):
            raise row
        return None if row is None else LiteLLM_VerificationTokenView(**row).model_dump_json()


@dataclass(frozen=True)
class JwtMapping:
    jwt_claim_name: str
    jwt_claim_value: str
    token: str
    jwt_issuer: str = ""
    is_active: bool = True


@dataclass
class FakeJwtKeyMappingStore:
    mappings: list[JwtMapping] = field(default_factory=list)
    error: Exception | None = None
    lookups: list[tuple[str, str, str | None]] = field(default_factory=list)

    async def mapped_token(
        self, jwt_claim_name: str, jwt_claim_value: str, jwt_issuer: str | None = None
    ) -> str | None:
        self.lookups.append((jwt_claim_name, jwt_claim_value, jwt_issuer))
        if self.error is not None:
            raise self.error
        return next(
            (
                m.token
                for m in self.mappings
                if m.is_active
                and (m.jwt_claim_name, m.jwt_claim_value, m.jwt_issuer)
                == (jwt_claim_name, jwt_claim_value, jwt_issuer or "")
            ),
            None,
        )

    async def mapped_claims(self, tokens: Sequence[str]) -> list[tuple[str, str, str]]:
        if self.error is not None:
            raise self.error
        return [(m.jwt_issuer, m.jwt_claim_name, m.jwt_claim_value) for m in self.mappings if m.token in tokens]


@dataclass(frozen=True)
class GroupBudget:
    spend: float = 0.0
    max_budget: float | None = None
    has_budget_windows: bool = False


@dataclass
class FakeModelAccessGroupStore:
    groups: dict[str, GroupBudget] = field(default_factory=dict)
    error: Exception | None = None
    budget_lookups: list[tuple[str, ...]] = field(default_factory=list)

    async def names(self, limit: int) -> list[str]:
        if self.error is not None:
            raise self.error
        return list(self.groups)[:limit]

    async def budgets(self, names: Sequence[str]) -> list[tuple[str, float, float | None, bool]]:
        self.budget_lookups.append(tuple(names))
        if self.error is not None:
            raise self.error
        return [
            (name, group.spend, group.max_budget, group.has_budget_windows)
            for name, group in self.groups.items()
            if name in names
        ]


def _answer(row: Mapping[str, object] | BaseModel | Exception | None, model: type) -> str | None:
    if isinstance(row, Exception):
        raise row
    if isinstance(row, BaseModel):
        row = row.model_dump()
    return None if row is None else model.model_validate(row).model_dump_json()


def _answers_by_alias(
    rows: Mapping[str, Mapping[str, object] | BaseModel | Exception], column: str, alias: str, model: type
) -> list[str]:
    """Up to two rows whose ``column`` is ``alias``, ordered by id, as the alias stores answer."""
    matching: Final[list[str | None]] = []
    for _, row in sorted(rows.items()):
        if isinstance(row, Exception):
            raise row
        fields = row.model_dump() if isinstance(row, BaseModel) else row
        if fields.get(column) == alias:
            matching.append(_answer(row, model))
    return [answer for answer in matching[:2] if answer is not None]


@dataclass
class FakeTeamStore:
    """Team rows by id, shaped through ``LiteLLM_TeamTable``."""

    rows: dict[str, Mapping[str, object] | BaseModel | Exception] = field(default_factory=dict)
    writer: dict[str, Mapping[str, object] | BaseModel | Exception] | None = None
    lookups: list[tuple[str, bool]] = field(default_factory=list)

    async def find(self, team_id: str, *, use_writer: bool = False) -> str | None:
        self.lookups.append((team_id, use_writer))
        source = self.writer if use_writer and self.writer is not None else self.rows
        return _answer(source.get(team_id), LiteLLM_TeamTable)

    async def find_by_alias(self, alias: str) -> list[str]:
        return _answers_by_alias(self.rows, "team_alias", alias, LiteLLM_TeamTable)


@dataclass
class FakeTeamMembershipStore:
    """Membership rows by ``(user_id, team_id)``, shaped through ``LiteLLM_TeamMembership``."""

    rows: dict[tuple[str, str], Mapping[str, object] | BaseModel | Exception] = field(default_factory=dict)
    writer: dict[tuple[str, str], Mapping[str, object] | BaseModel | Exception] | None = None
    lookups: list[tuple[str, str, bool]] = field(default_factory=list)

    async def find(self, user_id: str, team_id: str, *, use_writer: bool = False) -> str | None:
        self.lookups.append((user_id, team_id, use_writer))
        source = self.writer if use_writer and self.writer is not None else self.rows
        return _answer(source.get((user_id, team_id)), LiteLLM_TeamMembership)


@dataclass
class FakeOrganizationStore:
    """Organization rows by id, shaped through ``LiteLLM_OrganizationTable``. The budget is dropped unless
    the lookup asks for it, as the store only joins it on request."""

    rows: dict[str, Mapping[str, object] | BaseModel | Exception] = field(default_factory=dict)
    lookups: list[tuple[str, bool]] = field(default_factory=list)

    async def find(self, organization_id: str, *, include_budget_table: bool = False) -> str | None:
        self.lookups.append((organization_id, include_budget_table))
        row = self.rows.get(organization_id)
        if isinstance(row, Mapping) and not include_budget_table:
            row = {**row, "litellm_budget_table": None}
        return _answer(row, LiteLLM_OrganizationTable)

    async def find_by_alias(self, alias: str) -> list[str]:
        rows: Final = {
            organization_id: {**row, "litellm_budget_table": None} if isinstance(row, Mapping) else row
            for organization_id, row in self.rows.items()
        }
        return _answers_by_alias(rows, "organization_alias", alias, LiteLLM_OrganizationTable)


@dataclass
class RustAuthStores:
    key: FakeKeyStore = field(default_factory=FakeKeyStore)
    jwt_key_mapping: FakeJwtKeyMappingStore = field(default_factory=FakeJwtKeyMappingStore)
    model_access_group: FakeModelAccessGroupStore = field(default_factory=FakeModelAccessGroupStore)
    team: FakeTeamStore = field(default_factory=FakeTeamStore)
    team_membership: FakeTeamMembershipStore = field(default_factory=FakeTeamMembershipStore)
    organization: FakeOrganizationStore = field(default_factory=FakeOrganizationStore)

    def by_kind(self) -> Mapping[Callable[..., object], object]:
        return {
            RustKeyRepository: self.key,
            RustJwtKeyMappingRepository: self.jwt_key_mapping,
            RustModelAccessGroupRepository: self.model_access_group,
            RustTeamRepository: self.team,
            RustTeamMembershipRepository: self.team_membership,
            RustOrganizationRepository: self.organization,
        }


def install_rust_auth_stores(monkeypatch: pytest.MonkeyPatch, stores: RustAuthStores | None = None) -> RustAuthStores:
    """Answers `rust_database.repository` with these fakes; a kind with no fake answers None, so "No db connected" still fires."""
    installed = stores or RustAuthStores()
    monkeypatch.setattr(rust_database, "repository", lambda kind: installed.by_kind().get(kind))
    return installed


@pytest.fixture
def rust_auth_stores(monkeypatch: pytest.MonkeyPatch) -> RustAuthStores:
    return install_rust_auth_stores(monkeypatch)
