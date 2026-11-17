from typing import Final

import pytest

from litellm.proxy.db.connection_budget import (
    RUST_CONNECTIONS_ENV_VAR,
    RUST_RESERVED_CONNECTIONS,
    ConnectionBudget,
    rust_connections,
    shared_rust_connections,
)


@pytest.mark.parametrize("total", range(1, 41))
def test_prisma_and_rust_never_open_more_than_the_total(total: int) -> None:
    budget: Final = ConnectionBudget.split(total)

    assert budget.python + budget.rust == total
    assert budget.python >= 1


@pytest.mark.parametrize(
    ("total", "python", "rust"),
    [(1, 1, 0), (2, 1, 1), (3, 1, 2), (4, 2, 2), (100, 98, 2)],
    ids=["single_connection", "two_connections", "three_connections", "four_connections", "large"],
)
def test_rust_takes_what_is_left_of_its_reserve_after_prisma_keeps_one(total: int, python: int, rust: int) -> None:
    budget: Final = ConnectionBudget.split(total, rust_reserved=2)

    assert (budget.python, budget.rust) == (python, rust)


def test_a_single_connection_stays_with_prisma_untouched() -> None:
    params: Final = {"connection_limit": "1", "pool_timeout": 30}
    budget: Final = ConnectionBudget.of("1")

    assert budget is not None
    assert budget.prisma_limit is None
    assert budget.prisma_params(params) == params


@pytest.mark.parametrize("connection_limit", ["7", 7])
def test_a_positive_integer_limit_is_split(connection_limit: str | int) -> None:
    assert ConnectionBudget.of(connection_limit, rust_reserved=2) == ConnectionBudget.split(7, rust_reserved=2)


@pytest.mark.parametrize("connection_limit", ["0", 0, "-1", -1, "abc", "10.0", 10.5, "", "²"])
def test_a_limit_that_is_not_a_positive_integer_is_left_to_prisma(connection_limit: str | float) -> None:
    assert ConnectionBudget.of(connection_limit, rust_reserved=2) is None


def test_prisma_params_change_only_beside_a_rust_pool() -> None:
    params: Final = {"connection_limit": "010", "pool_timeout": 30}

    assert ConnectionBudget.split(10, rust_reserved=0).prisma_params(params) == params
    assert ConnectionBudget.split(10).prisma_params(params) == {
        "connection_limit": 8,
        "pool_timeout": 30,
    }


@pytest.mark.parametrize(
    ("limits", "expected"),
    [(("10", "50"), 2), (("2", "50"), 1), (("50", "1"), 0), (("abc", "2"), 1), (("abc", None), None), ((), None)],
    ids=["both_full", "writer_short", "reader_single", "writer_left_to_prisma", "nothing_split", "no_pools"],
)
def test_the_shared_rust_share_fits_every_split_pool(limits: tuple[str | None, ...], expected: int | None) -> None:
    budgets: Final = tuple(None if limit is None else ConnectionBudget.of(limit) for limit in limits)

    assert shared_rust_connections(budgets) == expected


@pytest.mark.parametrize(
    ("environ", "expected"),
    [
        ({RUST_CONNECTIONS_ENV_VAR: "0"}, 0),
        ({RUST_CONNECTIONS_ENV_VAR: "1"}, 1),
        ({}, RUST_RESERVED_CONNECTIONS),
        ({RUST_CONNECTIONS_ENV_VAR: "-1"}, RUST_RESERVED_CONNECTIONS),
        ({RUST_CONNECTIONS_ENV_VAR: "two"}, RUST_RESERVED_CONNECTIONS),
    ],
    ids=["cli_left_none", "cli_left_one", "started_without_cli", "negative", "not_a_number"],
)
def test_a_worker_sizes_its_rust_pool_from_the_cli_split(environ: dict[str, str], expected: int) -> None:
    assert rust_connections(environ) == expected
