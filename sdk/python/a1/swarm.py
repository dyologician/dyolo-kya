"""
a1.swarm — SwarmPassport Python client.

Create and manage multi-agent swarms with role-based identity.

    from a1.swarm import SwarmClient, SwarmRole

    client = SwarmClient("http://localhost:8080")

    swarm_id = client.create_swarm(
        name="acme-trading-swarm",
        capabilities=["trade.equity", "portfolio.read"],
        ttl_days=30,
        signing_key_hex=ROOT_SK_HEX,
    )

    cert = client.add_member(
        swarm_id=swarm_id,
        agent_pk_hex=WORKER_PK_HEX,
        role=SwarmRole.WORKER,
        capabilities=["trade.equity"],
        ttl_seconds=3600,
        signing_key_hex=ROOT_SK_HEX,
    )
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional
import httpx


class SwarmRole(str, Enum):
    ORCHESTRATOR = "orchestrator"
    WORKER = "worker"
    SUPERVISOR = "supervisor"
    AUDITOR = "auditor"


@dataclass(frozen=True)
class SwarmInfo:
    swarm_id: str
    swarm_name: str
    swarm_id_hex: str
    member_count: int
    active_members: int


@dataclass(frozen=True)
class SwarmMemberInfo:
    agent_did: str
    agent_pk_hex: str
    role: str
    issued_at_unix: int
    expires_at_unix: int
    cert_fingerprint_hex: Optional[str]


class SwarmError(Exception):
    def __init__(self, msg: str, code: str = "SWARM_ERROR") -> None:
        super().__init__(msg)
        self.code = code


class SwarmClient:
    """HTTP client for the A1 swarm passport endpoints."""

    def __init__(self, gateway_url: str, *, timeout: float = 15.0, admin_secret: Optional[str] = None) -> None:
        self._base = gateway_url.rstrip("/")
        headers = {}
        if admin_secret:
            headers["Authorization"] = f"Bearer {admin_secret}"
        self._client = httpx.Client(timeout=timeout, headers=headers)

    def create_swarm(self, *, name: str, capabilities: list[str], ttl_days: int = 30, signing_key_hex: str) -> str:
        """Create a new swarm passport. Returns the swarm_id."""
        resp = self._post("/v1/swarm/create", {
            "swarm_name": name,
            "capabilities": capabilities,
            "ttl_days": ttl_days,
            "signing_key_hex": signing_key_hex,
        })
        return resp["swarm_id"]

    def add_member(self, *, swarm_id: str, agent_pk_hex: str, role: SwarmRole | str,
                   capabilities: list[str], ttl_seconds: int = 3600, signing_key_hex: str) -> SwarmMemberInfo:
        """Issue a role-scoped cert and register the agent as a swarm member."""
        role_name = role.value if isinstance(role, SwarmRole) else role
        resp = self._post("/v1/swarm/member/add", {
            "swarm_id": swarm_id,
            "agent_pk_hex": agent_pk_hex,
            "role": role_name,
            "capabilities": capabilities,
            "ttl_seconds": ttl_seconds,
            "signing_key_hex": signing_key_hex,
        })
        m = resp["member"]
        return SwarmMemberInfo(
            agent_did=m["agent_did"],
            agent_pk_hex=m["agent_pk_hex"],
            role=m["role"],
            issued_at_unix=m["issued_at_unix"],
            expires_at_unix=m["expires_at_unix"],
            cert_fingerprint_hex=m.get("cert_fingerprint_hex"),
        )

    def get_chain(self, *, swarm_id: str, agent_pk_hex: str, ttl_seconds: int = 3600, signing_key_hex: str) -> dict:
        """Build a ready-to-use delegation chain for a swarm member."""
        return self._post("/v1/swarm/member/chain", {
            "swarm_id": swarm_id,
            "agent_pk_hex": agent_pk_hex,
            "ttl_seconds": ttl_seconds,
            "signing_key_hex": signing_key_hex,
        })

    def list_members(self, swarm_id: str) -> list[SwarmMemberInfo]:
        """List all active members in a swarm."""
        resp = self._get(f"/v1/swarm/{swarm_id}/members")
        return [SwarmMemberInfo(**m) for m in resp.get("members", [])]

    def remove_member(self, *, swarm_id: str, agent_did: str) -> None:
        """Remove an agent from the swarm."""
        self._post("/v1/swarm/member/remove", {"swarm_id": swarm_id, "agent_did": agent_did})

    def _post(self, path: str, body: dict) -> dict:
        resp = self._client.post(f"{self._base}{path}", json=body)
        self._raise(resp)
        return resp.json()

    def _get(self, path: str) -> dict:
        resp = self._client.get(f"{self._base}{path}")
        self._raise(resp)
        return resp.json()

    def _raise(self, resp: httpx.Response) -> None:
        if resp.is_error:
            try:
                err = resp.json().get("error", resp.text)
            except Exception:
                err = resp.text
            raise SwarmError(err)