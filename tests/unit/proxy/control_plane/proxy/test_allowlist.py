from typing import Final

import pytest

from litellm.proxy.control_plane.proxy.allowlist import Allowed, Denied, decide


@pytest.mark.parametrize(
    "path",
    [
        "/key/generate",
        "key/list",
        "/team/new",
        "/user/info",
        "/model/info",
        "/v1/model/info",
        "/model_group/info",
        "/management/v1/keys",
        "/audit",
        "/spend/logs",
    ],
)
def test_management_routes_are_allowed_with_a_normalised_leading_slash(path: str) -> None:
    assert decide(path) == Allowed("/" + path.lstrip("/"))


@pytest.mark.parametrize(
    ("path", "fragment"),
    [
        ("/v1/chat/completions", "inference"),
        ("/chat/completions", "inference"),
        ("/v1/embeddings", "inference"),
        ("/v1/messages", "inference"),
        ("/guardrails/apply_guardrail", "guardrail apply"),
        ("/guardrails/apply_guardrail/123", "guardrail apply"),
        ("/mcp/tools/call", "inference"),
        ("/login", "auth bootstrap"),
        ("/sso/key/generate", "auth bootstrap"),
        ("/key/service-account/generate", "auth bootstrap"),
        ("/key%2Fservice-account%2Fgenerate", "auth bootstrap"),
        ("/key/service-account/generate/", "auth bootstrap"),
        ("/key/service-account/generate%2F", "auth bootstrap"),
        ("/config/yaml", "auth bootstrap"),
        ("/key/../config/yaml", "traversal"),
        ("/key//info", "traversal"),
        ("/health/drain", "lifecycle"),
        ("/health/drain/", "lifecycle"),
    ],
)
def test_inference_auth_and_traversal_are_denied(path: str, fragment: str) -> None:
    decision: Final = decide(path)
    assert isinstance(decision, Denied), f"{path} was allowed"
    assert fragment in decision.reason


@pytest.mark.parametrize(
    "path",
    [
        "/key%2F..%2Flogin",
        "/key/%2e%2e/login",
        "/key/%252e%252e/login",
        "/key/./info",
        "/key//info",
        "/key/list/.",
        "/key/list/..",
        "/key/\\../login",
        "/credentialsx",
        "/auditor/logs",
        "/healthz",
        "/settings_export",
    ],
)
def test_encoded_dotted_or_boundary_breaking_paths_are_denied(path: str) -> None:
    assert isinstance(decide(path), Denied), path


@pytest.mark.parametrize("path", ["/credentials", "/credentials/by_name/x", "/audit", "/audit/1", "/settings"])
def test_bare_management_routes_match_only_on_a_route_boundary(path: str) -> None:
    assert decide(path) == Allowed(path)


@pytest.mark.parametrize("path", ["/health/readiness", "/health/liveliness", "/health/liveness", "/health/latest"])
def test_safe_health_routes_are_allowed(path: str) -> None:
    assert decide(path) == Allowed(path)


@pytest.mark.parametrize(
    "path",
    [
        "/health",
        "/health/services",
        "/health/test_connection",
        "/health/readiness/details",
        "/health/liveness/details",
    ],
)
def test_health_routes_that_probe_or_expose_details_are_denied(path: str) -> None:
    assert isinstance(decide(path), Denied)


def test_decided_path_is_the_decoded_path_the_dataplane_will_receive() -> None:
    assert decide("key/info%3Fx") == Denied("/key/info%3Fx", "encoded or reserved path characters are not allowed")
    assert decide("/team/my%20team/info") == Allowed("/team/my team/info")
