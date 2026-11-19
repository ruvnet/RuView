# Security Policy

RuView processes radio signals that can reveal whether people are present,
how they move, and their vital signs. We treat security and privacy bugs as
high priority and appreciate responsible disclosure.

## Reporting a vulnerability

**Please do not report security vulnerabilities through public issues,
discussions, or pull requests.**

Report privately through GitHub's private vulnerability reporting:

1. Go to the repository's **Security** tab.
2. Select **Report a vulnerability**.
3. Fill in the advisory form.

Direct link: <https://github.com/ruvnet/RuView/security/advisories/new>

If private reporting isn't available, open a public issue that says only that
you have a security report and need a private contact. Leave out all technical
details.

### What to include

- the affected component (crate, firmware target, package, workflow) and
  version or commit
- the impact: what an attacker can read, inject, or control, and from where
  (local, LAN, internet, physical access to a node)
- step-by-step reproduction, a proof of concept, or a captured log
- any suggested fix or mitigation

Never include real CSI captures, personal data, WiFi credentials, or API
tokens in a report. Use synthetic data or redact it.

### What to expect

- An acknowledgement once a maintainer has triaged the report.
- Coordination on a fix and a disclosure timeline. Please give us reasonable
  time to release a fix before any public disclosure.
- Credit in the advisory and release notes, unless you prefer to stay
  anonymous.

## Supported versions

Security fixes land on `main` and ship in the next release of each affected
component (Rust crates and sensing server, ESP32 firmware, the `wifi-densepose`
PyPI wheel, and the npm packages). Older releases are not patched; upgrade to
the latest release.

`archive/v1/` is a frozen reference implementation and is not supported for
deployment.

## Scope

In scope:

- the sensing server's HTTP, WebSocket, and UDP CSI interfaces
- authentication and authorization (API tokens, OAuth)
- ESP32 firmware, provisioning, OTA updates, and mesh security
- privacy boundaries, such as raw CSI or BFI leaving a node, identity
  leakage, or bypassing privacy modes
- MCP servers and the contributor harnesses (permission or sandbox escapes)
- CI/CD and release workflows (supply-chain risks)
- Home Assistant, Matter, and Apple Home bridges

Out of scope:

- risks that come from running without authentication on a network you chose
  to expose (for example, setting `RUVIEW_ALLOW_UNAUTHENTICATED=1` or a
  routable `--udp-insecure-lan` bind). These are documented operator choices.
- vulnerabilities in third-party dependencies that are already public and
  tracked upstream. A report is still welcome if RuView's use of the
  dependency makes it exploitable.
- reports from automated scanners without a demonstrated impact

## Secure deployment guidance

- Keep the sensing server bound to `127.0.0.1` unless you enable API
  authentication. See the
  [sensing server security notes](v2/crates/wifi-densepose-sensing-server/SECURITY.md)
  for the HTTP and UDP bind rules and their residual risks.
- Never commit `.env` files or credentials. Start from
  [`example.env`](example.env).
- Provision WiFi credentials onto nodes without echoing them into shell
  history, logs, or issues.
