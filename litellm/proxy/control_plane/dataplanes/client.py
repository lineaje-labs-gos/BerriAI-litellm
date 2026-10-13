"""One HTTP call to one dataplane: credential injection, timeout, bounded retry with jitter, failure as a value.

The transport is a plain ``httpx.AsyncClient`` (the pool behind ``get_async_httpx_client``), not ``AsyncHTTPHandler``:
the handler retries writes on connection errors, replays PATCH as POST and raises ``litellm.Timeout``, none of which
a management passthrough can tolerate. Retrying is this module's job and only ever happens for GET."""

import asyncio
import random
from collections.abc import Awaitable, Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Final, Literal, Protocol, TypeAlias

import httpx
from pydantic import TypeAdapter

from litellm._logging import verbose_proxy_logger
from litellm.proxy.control_plane.dataplanes.credentials import Caller, CredentialKind, CredentialUnavailable
from litellm.proxy.control_plane.dataplanes.registry import Dataplane

Method: TypeAlias = Literal["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"]
WRITE_METHODS: Final = frozenset({"POST", "PUT", "PATCH", "DELETE"})
Sleeper: TypeAlias = Callable[[float], Awaitable[None]]
RETRYABLE_STATUSES: Final = frozenset({502, 503, 504})
_JSON: Final = TypeAdapter(object)


class HttpTransport(Protocol):
    """The slice of ``httpx.AsyncClient.request`` the client needs, so tests inject a fake. Raises only httpx errors
    and never retries on its own."""

    async def request(
        self,
        method: str,
        url: str,
        *,
        params: Sequence[tuple[str, str]] | None = None,
        json: dict[str, object] | None = None,  # mutable-ok: httpx takes dict
        headers: dict[str, str] | None = None,  # mutable-ok: httpx takes dict
        timeout: float,
    ) -> httpx.Response: ...


@dataclass(frozen=True, slots=True)
class DataplaneReply:
    status: int
    body: object
    credential: CredentialKind
    raw_content: bytes | None = None
    content_type: str | None = None
    content_disposition: str | None = None
    cache_control: str | None = None


@dataclass(frozen=True, slots=True)
class DataplaneUnreachable:
    detail: str
    attempts: int


CallResult: TypeAlias = DataplaneReply | DataplaneUnreachable | CredentialUnavailable


@dataclass(frozen=True, slots=True)
class _RawReply:
    status: int
    body: object
    raw_content: bytes | None
    content_type: str | None
    content_disposition: str | None
    cache_control: str | None


@dataclass(frozen=True, slots=True)
class RetryPolicy:
    attempts: int = 3
    base_delay_seconds: float = 0.2
    max_delay_seconds: float = 2.0

    def delay(self, attempt: int, jitter: float) -> float:
        capped: Final = min(self.max_delay_seconds, self.base_delay_seconds * (2.0**attempt))
        return capped * (0.5 + jitter / 2)


def _parse_body(response: httpx.Response) -> object:
    try:
        return _JSON.validate_python(response.json())
    except ValueError:
        return response.text


@dataclass(frozen=True, slots=True)
class DataplaneClient:
    transport: HttpTransport
    timeout_seconds: float
    retry: RetryPolicy = RetryPolicy()
    sleep: Sleeper = asyncio.sleep
    jitter: Callable[[], float] = random.random

    async def get(
        self,
        dataplane: Dataplane,
        path: str,
        caller: Caller,
        params: Sequence[tuple[str, str]] | None = None,
    ) -> CallResult:
        return await self._call("GET", dataplane, path, caller, params=params, json=None)

    async def post(self, dataplane: Dataplane, path: str, caller: Caller, json: Mapping[str, object]) -> CallResult:
        return await self._call("POST", dataplane, path, caller, params=None, json=json)

    async def request(
        self,
        method: Method,
        dataplane: Dataplane,
        path: str,
        caller: Caller,
        params: Sequence[tuple[str, str]] | None = None,
        json: Mapping[str, object] | None = None,
    ) -> CallResult:
        """Any management verb. Only GET is retried, every write goes out exactly once."""
        return await self._call(method, dataplane, path, caller, params=params, json=json)

    async def _call(
        self,
        method: Method,
        dataplane: Dataplane,
        path: str,
        caller: Caller,
        params: Sequence[tuple[str, str]] | None,
        json: Mapping[str, object] | None,
    ) -> CallResult:
        credential: Final = await dataplane.credential.headers(caller)
        if isinstance(credential, CredentialUnavailable):
            return credential
        url: Final = f"{dataplane.url}/{path.lstrip('/')}"
        attempts: Final = self.retry.attempts if method == "GET" else 1
        outcome: Final = await self._attempt(
            method, url, credential.headers, params, json, attempt=0, attempts=attempts
        )
        return (
            outcome
            if isinstance(outcome, DataplaneUnreachable)
            else DataplaneReply(
                outcome.status,
                outcome.body,
                credential.kind,
                outcome.raw_content,
                outcome.content_type,
                outcome.content_disposition,
                outcome.cache_control,
            )
        )

    async def _attempt(
        self,
        method: Method,
        url: str,
        headers: Mapping[str, str],
        params: Sequence[tuple[str, str]] | None,
        json: Mapping[str, object] | None,
        attempt: int,
        attempts: int,
    ) -> _RawReply | DataplaneUnreachable:
        outcome: Final = await self._send(method, url, headers, params, json)
        last: Final = attempt + 1 >= attempts
        if last or not _retryable(outcome):
            return outcome if isinstance(outcome, _RawReply) else DataplaneUnreachable(outcome.detail, attempt + 1)
        await self.sleep(self.retry.delay(attempt, self.jitter()))
        return await self._attempt(method, url, headers, params, json, attempt + 1, attempts)

    async def _send(
        self,
        method: Method,
        url: str,
        headers: Mapping[str, str],
        params: Sequence[tuple[str, str]] | None,
        json: Mapping[str, object] | None,
    ) -> _RawReply | DataplaneUnreachable:
        try:
            response: Final = await self.transport.request(
                method,
                url,
                params=params,
                json=None if json is None else dict(json),
                headers=dict(headers),
                timeout=self.timeout_seconds,
            )
        except httpx.HTTPError as error:
            verbose_proxy_logger.debug("Control-plane dataplane transport failed: %s", error)
            return DataplaneUnreachable(_describe(error), attempts=1)
        content_type: Final = response.headers.get("content-type")
        is_json: Final = content_type is not None and "json" in content_type.lower()
        preserve_raw: Final = not is_json
        return _RawReply(
            response.status_code,
            _parse_body(response),
            response.content if preserve_raw else None,
            content_type if preserve_raw else None,
            response.headers.get("content-disposition") if preserve_raw else None,
            response.headers.get("cache-control"),
        )


def _describe(error: httpx.HTTPError) -> str:
    return type(error).__name__


def _retryable(outcome: _RawReply | DataplaneUnreachable) -> bool:
    return isinstance(outcome, DataplaneUnreachable) or outcome.status in RETRYABLE_STATUSES
