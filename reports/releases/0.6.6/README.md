# 0.6.6 source version validation

Release date and local validation: **2026-11-13**. Previous release: **v0.6.5**.
This patch source release connects fixed-slot text saving in the explicitly enabled Session page,
repairs native Fetch completion and passive response-observer deadlines, and adds bounded federation
discovery contracts plus an explicit Linux IPv4 single-query adapter. Code remains an optional Task
payload; discovered node claims do not grant trust or resource access.

## Complete local quality gate

`scripts/quality-gate.sh --no-npm-install` completed successfully with **0.6.6** metadata. It used
Node **22.23.3**, Rust/Cargo **1.95.0**, existing locked dependencies, zero Rust dev/test debug
information and two build jobs. Inherited database and Node preload configuration was removed;
Cargo used its cached dependencies offline. Runtime source remained unchanged during final release
documentation updates.

| Selection | Actual result |
|---|---|
| Common checks | **884 passed**, including synchronized versions/course copies, generated contracts, compatibility and management-tool fixtures |
| Rust defaults and documentation | **1,633 passed**, **1,060 opt-in ignored**, 156 successful result groups; no failures or Rust warnings |
| Frontend | Next.js **15.5.28** production build and TypeScript passed; **323 tests passed** |
| SDK | Build, runtime, type and package-consumer checks passed; **42 tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, without audit exceptions or dependency upgrades |

Ignored cases are not passes. Node emitted 31 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings;
module policy was not changed to hide them. Cargo/npm lock changes affect only the project's root
version metadata. The dependency graph, public API schemas and **0.3.0** compatibility baseline
remain unchanged. Management-tool fixtures are not installation or live-kernel acceptance.

## Isolated native discovery

`scripts/test-federation-mdns-netns.sh` separately passed all **7** actual UDP cases on the 0.6.6
binary. Two fresh anonymous network namespaces and disposable virtual links had no host-network
attachment or default route. Six ordinary cases ran with zero capabilities; the address-removal
case retained `net_admin` only inside its disposable namespace. Python 3.14.4 supplied the test-only
peer, not an application dependency.

The cases cover the exact query source, complete DNS/TXT → F0 → cache chain, TTL/source-port/
destination/size rejection, another interface's traffic, same-source goodbye, cancellation,
descriptor cleanup and address removal. The runner stopped its owned peer; destroying the
namespaces removed their links/routes. No application database or physical LAN peer was used.
See the [limited discovery profile](../../../docs/en/federation-discovery.md).

## Browser and deployment boundaries

The mounted Session page saves existing fixed slots only with both `--task-text-publication` and
`--session-ui`. Artifact publication and Task replacement remain separate, strictly confirmed
operations; confirmed prefixes and unknown outcomes are preserved without automatic retry or
recovery. The normal Next.js editor and SDK do not gain this saving workflow.

Earlier real browser, SQL-prefix and isolated-TLS checks retain their dated scope in
[project status](../../../docs/en/project-status.md) and the
[testing guide](../../../docs/en/testing-guide.md). They are not new executions by this default
release gate. The earlier remote CI pass applies to `f0b5048`, not this candidate; its original
failure's unique cause remains unknown. Late acknowledgements, BFCache and deployed browser/TLS
acceptance remain separate work.

Federation has no mounted startup discovery, advertisement service, verified node pairing,
traversal, relay transport or cross-authority workspace grants. The isolated one-query fixture
does not establish generic mDNS interoperability or decentralized deployment acceptance.

Remote CI, offline-package/image creation, extracted-package/kernel smoke and installed-service
upgrades require their own results. Publishing this source tag does not create a GitHub Release or
deploy a service. The live RustSec audit, benchmarks and remaining opt-in SQL/browser/kernel cases
were not rerun by this release gate.

Raw local logs remain in `/tmp/cyanrex-release-0.6.6-i0310R`: `quality-gate-full-verified.log` and
`mdns-netns.log`. The namespace build evidence remains in `/tmp/cyanrex-mdns-netns-sDIGmm`.
Only this result summary is committed; private protocol data and raw logs are not uploaded.
