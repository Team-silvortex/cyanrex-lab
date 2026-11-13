# 0.6.8 source version validation

Release date and local validation: **2026-11-13**. Previous release: **v0.6.7**.
This patch includes F1j–F1o: original-bound controller quiet handoff, the explicit single-round
`probe-lan` diagnostic, pure announcement timing, frozen response comparison, direct-query planning
and response-byte preparation. Private DNS wire/record helpers and architecture/disk-use guidance
are consolidated without enabling a responder or changing the teaching and loopback-host boundaries.

## Complete local quality gate

`scripts/quality-gate.sh --no-npm-install` completed successfully with **0.6.8** metadata using
Node **22.23.3**, Rust/Cargo **1.95.0**, cached locked Cargo dependencies offline, zero dev/test debug
information and two build jobs. Database, PostgreSQL and Node preload configuration was removed
from the gate's environment. Only documentation changed while validation ran; runtime source did not.

| Selection | Actual result |
|---|---|
| Common checks | **898 passed**, including version/course synchronization, generated contracts, compatibility and management-tool fixtures |
| Rust defaults and documentation | **2,195 passed**, **1,128 opt-in ignored**, 176 successful result groups; zero failures or Rust warnings |
| Frontend | Next.js **15.5.28** production build and TypeScript passed; **323 tests passed** |
| SDK | Build, runtime, type and package-consumer checks passed; **42 tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, without exceptions or dependency upgrades |

Ignored tests are not passes. Node emitted 31 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings;
module policy was not changed to suppress them. Lockfile changes affect only the project's root
package version; the unrelated `zlib-rs` 0.6.7 dependency is unchanged. The dependency graph, API
routes/schemas and **0.3.0** compatibility baseline remain unchanged. The final common/format pass
also checked the completed report and synchronized documentation. Earlier scoped results retain
their exact versions and selection in the [federation guide](../../../docs/en/federation-discovery.md).

## Isolated federation networking

The **0.6.8** binaries separately passed **75 distinct cases** with:

```bash
scripts/test-federation-mdns-netns.sh --with-cli --with-unicast --with-probe \
  --with-controller --with-retained --with-controller-retained --with-probe-cli
```

The selection combines 7 original discovery, 6 browse CLI, 10 unicast, 13 probing, 8 controller,
11 retained, 10 controller-to-retained and 10 probe CLI cases. All traffic stayed inside two fresh
disconnected anonymous network namespaces, with only owned virtual links and no default route
or host-LAN attachment. Ordinary cases ran with all five capability sets zero. Separate interface/
address mutation cases retained only owned-netns NET_ADMIN; exact fixture names and addresses
were restored before subsequent cases. The runner stopped its owned peer and discarded both namespaces.

These checks include retained original bounds, late queued claims, cancellation, exact port release,
explicit CLI process gates and proposal layout, plus existing destination/interface/TTL and probe
timing boundaries. Peers use synthetic claims. They validate neither automatic publication nor
physical-LAN interoperability. No application database, host account, daemon or host interface was modified.

## Source and deployment boundaries

F1j retains original interface, clocks, declarations and spent ingress bounds for short receive-only
review; no renewed lease or publication authority is created. F1k explicitly selects only one probe
round and reports inert unverified observations, not announcements or proof of ownership. F1l–F1o
are pure preparation/comparison/encoding models; there is no new live sending or startup path.

No announcement/defense/withdrawal service, verified pairing, traversal, relay transport or
cross-authority workspace grant is enabled. Supplied query metadata and prepared bytes are not
ingress proof, live MTU acceptance, send permission or delivery receipts. Existing F0 expiry and
per-owner bounds remain in force; names, fingerprints and endpoints remain unverified claims.

Default tests do not run opt-in SQL/browser/kernel acceptance. Mock management-tool checks are not
installations. Isolated synthetic networking is not physical-LAN interoperability or decentralized
deployment acceptance. Earlier SQL/browser/TLS evidence retains its dated scope and was not rerun
as part of source-release preparation. No application database or host LAN is used.

Remote CI, offline image/package builds, extracted-package/kernel smoke and installed-service
upgrades have separate acceptance boundaries. Pushing an annotated source tag does not create a
GitHub Release, publish npm packages or deploy a service. No new live RustSec audit or benchmark
is claimed by the default gate.
