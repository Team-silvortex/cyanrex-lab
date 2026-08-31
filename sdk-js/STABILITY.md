# SDK Stability Policy

`@cyanrex/sdk-js` is a pre-1.0 internal package. Its compatibility rules are explicit so internal
consumers can upgrade safely while the package is prepared for independent publication.

## Protected surfaces

The following surfaces are compatibility-protected:

- the public `CyanrexClient` root methods and task-oriented namespace paths recorded in
  `compatibility/public-surface.json`;
- browser-facing operation IDs, request inputs, successful responses, and access tiers represented by
  the OpenAPI compatibility baseline;
- generated operation input/response types and the `/openapi` and `/operations` package exports;
- `CyanrexApiError` status, details, method, URL, and message behavior.

The namespace baseline protects member existence and nesting. OpenAPI compatibility checks protect the
wire contract, while compile-time fixtures protect representative generated-operation call shapes.

## Change rules

Patch releases within the `0.3.x` line are additive-only for protected surfaces. They may add optional
fields, operations, namespaces, or methods, but must not remove or rename existing members, require a
previously optional input, narrow an accepted input, weaken a successful response, or raise an access
tier without an explicitly reviewed breaking release.

To retire a protected SDK member:

1. mark it `@deprecated` in declarations and document the replacement in the changelog;
2. retain it for at least the complete following minor release line;
3. remove it only in a later minor release with a migration note and an intentional baseline update.

For example, a member deprecated during `0.4.x` remains callable throughout `0.4.x` and may be removed
no earlier than `0.5.0`. A critical security issue may shorten this window, but the release must state
the impact and safe migration explicitly.

## Intentional baseline updates

The normal quality gate never rewrites compatibility baselines. After an additive public SDK member is
accepted, capture it for future removal protection with:

```bash
node ../scripts/sdk-surface-compatibility.mjs --write-baseline
```

A baseline update that accepts a removal belongs only in the reviewed breaking release that documents
the migration. OpenAPI baseline replacement follows the separate compatibility command described in
the SDK README.

## Not yet guaranteed

Private implementation details, exact internal request ordering, undocumented error text, and the five
signed Runner Agent protocol operations are not public SDK compatibility surfaces. Registry publication,
long-term support ownership, and 1.0 criteria remain separate release decisions; the package stays private
until those are defined.
