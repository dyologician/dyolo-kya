<!-- SECURITY.md -->
# Security Policy

## Supported Versions
We currently support the latest major release of `dyolo-kya`.

## Reporting a Vulnerability
Please do not report security vulnerabilities through public GitHub issues. If you discover a cryptographic bypass, scope escalation flaw, or temporal validation bug, please email security@dyolo.com directly. We maintain a 48-hour triage SLA.

## Threat Model Boundary

**What this library protects against:**
*   **Scope Escalation:** Cryptographic enforcement that child scopes are strict subsets of parent scopes via `SubScopeProof`.
*   **Temporal Abuse:** Enforcement that child certificates cannot outlive their parents, and actions cannot be executed outside the `[issued_at, expiration_unix]` window.
*   **Replay Attacks:** Strict uniqueness checks per certificate hop utilizing a 128-bit nonce via the provided `NonceStore`.
*   **Revoked Authority:** Cryptographic exclusion via the provided `RevocationStore`.
*   **Hash Injection:** Prefix-free canonical encoding of action/parameter sets prevents collision and length-extension attacks.

**What this library DOES NOT protect against:**
*   **Storage Layer Compromise:** The integrity of the `RevocationStore` and `NonceStore` is the responsibility of the implementing application (e.g., Redis security). If an attacker gains write access to these stores, replay and revocation protections are bypassed.
*   **Key Compromise:** If an agent's private Ed25519 key is leaked, actions taken by the attacker using that key cannot be distinguished from legitimate actions prior to certificate revocation.