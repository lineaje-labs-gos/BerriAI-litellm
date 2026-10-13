from dataclasses import dataclass, field
from typing import Final

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from litellm.proxy.control_plane.app import LicenseRequired
from litellm.proxy.control_plane.main import Tls, serve
from litellm.proxy.control_plane.settings import ControlPlaneSettings
from tests.unit.proxy.control_plane.support import MASTER_KEY


@dataclass(frozen=True)
class _License:
    premium: bool

    def is_premium(self) -> bool:
        return self.premium


@dataclass
class _Runner:  # mutable-ok: records the app it was asked to serve
    served: list[tuple[FastAPI, str, int, Tls | None, int | None]] = field(default_factory=list)

    def __call__(self, app: FastAPI, *, host: str, port: int, tls: Tls | None, limit_concurrency: int | None) -> None:
        self.served.append((app, host, port, tls, limit_concurrency))


def _settings() -> ControlPlaneSettings:
    return ControlPlaneSettings.model_validate({"LITELLM_MASTER_KEY": MASTER_KEY})


def test_serve_builds_the_control_plane_app_and_hands_it_to_the_runner() -> None:
    runner: Final = _Runner()
    serve("127.0.0.1", 8100, settings=_settings, license_check=lambda: _License(True), run=runner)
    [(app, host, port, _tls, _limit_concurrency)] = runner.served
    assert (host, port) == ("127.0.0.1", 8100)
    client: Final = TestClient(app)
    assert client.get("/healthz").json() == {"status": "ok"}
    assert client.get("/api/v1/dataplanes", headers={"Authorization": f"Bearer {MASTER_KEY}"}).json() == {"items": []}
    assert client.get("/v1/chat/completions").status_code == 404, "the proxy's routes must not be on this app"


def test_serve_refuses_without_a_license_and_never_binds() -> None:
    runner: Final = _Runner()
    with pytest.raises(LicenseRequired):
        serve("0.0.0.0", 4000, settings=_settings, license_check=lambda: _License(False), run=runner)
    assert runner.served == []


def test_serve_hands_configured_tls_to_the_runner() -> None:
    runner: Final = _Runner()
    tls: Final = Tls(keyfile="/certs/tls.key", certfile="/certs/tls.crt")
    serve("0.0.0.0", 8443, tls=tls, settings=_settings, license_check=lambda: _License(True), run=runner)
    assert [entry[3] for entry in runner.served] == [tls]


def test_serve_hands_limit_concurrency_to_the_runner() -> None:
    runner: Final = _Runner()
    serve(
        "0.0.0.0",
        8100,
        settings=_settings,
        license_check=lambda: _License(True),
        run=runner,
        limit_concurrency=250,
    )
    assert [entry[4] for entry in runner.served] == [250]
