# 0.5.7 source version validation

Date: **2026-10-12**. Previous release: **v0.5.6**. This compatible patch includes independent
publisher/recipient ledgers, current-Session grant management/publication/sharing, exact recipient
manifest and verified chunk reads, an unmounted binary HTTP adapter, a controlled native client and
private whole-file assembly. A test-fixture correction keeps both queued reservation callers polled.
The normal application remains unmounted for this distribution path; no live authentication cutover,
automatic migration, multi-chunk network scheduler, final user-selected save, resume or peer pairing is added.

## Fresh release gate

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on version **0.5.7**:

| Selection | Actual result |
|---|---|
| Rust defaults and documentation | **936 passed**, **841 opt-in ignored**, **88 result groups**, zero warnings/failures; includes 12 compile-fail contracts |
| Common checks | **301 passed**; file limits, synchronized versions/course documents, generated OpenAPI/SDK/agent contracts, compatibility and management-tool checks |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **208 tests passed** |
| SDK | Runtime/build/types passed; **39 runtime + 3 package-consumer tests passed** |
| Production npm audits | Frontend and SDK both reported **zero vulnerabilities**; no exceptions or dependency-version changes |

Installed dependency trees were checked first; only root package versions changed in npm lockfiles,
so the gate reused the existing dependencies rather than reinstalling them. Rust dependencies and
their lockfile entries are unchanged apart from the root Engine version. Inherited database,
fallback, PG connection, dotenv, Node and native-release-tool settings were removed for the gate.
No deployed database, configuration or service was used; no privileged live-kernel workflow was exercised.

The **31 explicit PostgreSQL cases** from [private assembly verification](../../../docs/en/large-file-chunk-assembly.md)
and the earlier client SQL/OpenSSL proof retain their original **0.5.6** date/version/scope; they are
not fresh 0.5.7 SQL or TLS acceptance. The ignored inventory is not counted as passing execution.
No new RustSec audit, browser suite, current LAN/SSH/TLS deployment, live-kernel acceptance,
offline Docker archive build/extraction/smoke, disk-full/fsync/power-loss or throughput/RSS benchmark
was performed locally for this release. The tag workflow separately builds and validates its exact
offline candidate; a pushed tag is not a successful candidate result or a GitHub Release.

## Release identity and retained evidence

Canonical Engine, root Cargo/npm locks, frontend/SDK packages, OpenAPI info, README and runner-agent
documents are synchronized to **0.5.7**. API compatibility remains against the frozen **0.3.0** baseline;
no public operation or SDK surface is added. Changelog entries moved into the dated 0.5.7 section.
Current bilingual source-inclusion notes and generated course copies distinguish release inclusion
from earlier implementation evidence. All **106 earlier status blocks**, **36 detailed execution tails**
and the capability tensor are preserved unchanged.

Raw gate logs remain outside the repository at `/tmp/cyanrex-release-0.5.7-LJzz0U4x`.
This record precedes the version commit. Clean-candidate and annotated-tag preflight must bind
`v0.5.7` to the exact version commit before branch/tag publication; existing tags are never moved.
No remote CI success, offline-artifact acceptance or deployment is claimed by this local record.
