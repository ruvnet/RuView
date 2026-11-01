# ADR-384: Preserve explicitly supplied SDK search limits

Status: Proposed; maintainer acceptance pending

## Context

The public `memorySearch(query, limit)` helper uses a truthiness check before forwarding its optional limit. Explicit zero and NaN therefore disappear, even though the shared registry rejects them. The helper silently returns success where the equivalent raw SDK call returns `invalid_arguments`; strict SDK users also miss the expected `RuViewError`.

## Decision

Omit the optional limit only when it is undefined. Forward every explicitly supplied value to the existing shared validator. Do not duplicate validation or change the registry range, search algorithm, result count, strict error policy or defaults.

## Alternatives

Validating separately in the helper would duplicate schema rules and risk drift. Retaining truthiness makes invalid caller inputs indistinguishable from an omitted option.

## Verification

MEASURED local software regression exercises the public SDK and actual registry. Original code fails for zero; the corrected helper rejects zero, NaN, negative and excessive limits, preserves strict errors, and matches raw registry results for omitted and valid limits. Package gate receipts are recorded separately against the final revision. No external retrieval, training, hardware or production service claim is made.
