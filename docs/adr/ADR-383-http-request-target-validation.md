# ADR-383: Reject malformed HTTP request targets without stopping the transport

Status: Proposed; maintainer acceptance pending

## Context

ADR-375 introduces the authenticated Streamable HTTP transport. Node accepts an HTTP request target that the WHATWG URL parser rejects. The current request listener lets that parser exception escape, terminating the process before route selection. A bounded loopback fixture using the public `startMcpHttp` API reproduces this with `http://[`; the original code resets the connection and exits.

## Decision

Catch URL construction failures at the request boundary and return HTTP 400 with the generic `invalid_request_target` error. Do not include the request target or parser exception in the response, since the target can carry a secret path. Keep route selection, token checks, origin policy, grants, protocol negotiation and JSON handling unchanged.

## Alternatives

Relying on Node's HTTP parser does not suffice: a syntactically accepted request target can still fail URL construction. A process-level exception handler would obscure unrelated faults and would not provide a request-specific response.

## Verification

MEASURED software fixture: the actual child-process HTTP transport fails the new regression on the original source (nine existing controls pass); the patched transport passes all ten focused tests. The new test also verifies subsequent health, rejected unauthenticated calls and successful authenticated initialization. Broader package checks are recorded separately against the final implementation revision. No external service, device, hardware, throughput or production deployment claim is made.

## Consequences

Malformed URL targets receive a stable client error and do not interrupt later requests. This change does not add SSE or sessions, change authentication, or authorize additional operations.
