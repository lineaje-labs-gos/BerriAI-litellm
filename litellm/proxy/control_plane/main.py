"""Entrypoint for `litellm --control-plane`: the same image and CLI, a different app. Refuses to serve unlicensed."""

from collections.abc import Callable
from dataclasses import dataclass
from typing import Final, Protocol

from fastapi import FastAPI

from litellm.proxy.auth.litellm_license import LicenseCheck
from litellm.proxy.control_plane.app import License, create_app
from litellm.proxy.control_plane.container import build_container
from litellm.proxy.control_plane.settings import ControlPlaneSettings, load_settings


@dataclass(frozen=True, slots=True)
class Tls:
    keyfile: str
    certfile: str


class Runner(Protocol):
    def __call__(
        self, app: FastAPI, *, host: str, port: int, tls: Tls | None, limit_concurrency: int | None
    ) -> None: ...


def _uvicorn(app: FastAPI, *, host: str, port: int, tls: Tls | None, limit_concurrency: int | None) -> None:
    import uvicorn

    keyfile: Final = None if tls is None else tls.keyfile
    certfile: Final = None if tls is None else tls.certfile
    uvicorn.run(
        app,
        host=host,
        port=port,
        ssl_keyfile=keyfile,
        ssl_certfile=certfile,
        limit_concurrency=limit_concurrency,
    )


def build_app(settings: ControlPlaneSettings, license_check: License) -> FastAPI:
    return create_app(build_container(settings), license_check)


def serve(
    host: str,
    port: int,
    tls: Tls | None = None,
    settings: Callable[[], ControlPlaneSettings] = load_settings,
    license_check: Callable[[], License] = LicenseCheck,
    run: Runner = _uvicorn,
    limit_concurrency: int | None = None,
) -> None:
    app: Final = build_app(settings(), license_check())
    run(app, host=host, port=port, tls=tls, limit_concurrency=limit_concurrency)
