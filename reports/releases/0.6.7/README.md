# 0.6.7 source version validation

Release date and local validation: **2026-11-13**. Previous release: **v0.6.6**.
This patch includes federation stages F1c–F1i: explicit Linux discovery, frozen wire preparation,
exclusive unicast reception, bounded probing, finite conflict cooldown and retained quiet review.
Discovery claims remain unverified. Code remains an optional Task payload; the teaching application,
private loopback host and existing authorization boundaries are unchanged.

## Complete local quality gate

`scripts/quality-gate.sh --no-npm-install` completed successfully with **0.6.7** metadata using
Node **22.23.3**, Rust/Cargo **1.95.0**, cached locked Cargo dependencies offline, zero dev/test debug
information and two build jobs. Inherited database, PostgreSQL and Node preload configuration was
removed from the gate's environment. Runtime source stayed unchanged during final documentation updates.

| Selection | Actual result |
|---|---|
| Common checks | **891 passed**, including version/course synchronization, generated contracts, compatibility and management-tool fixtures |
| Rust defaults and documentation | **1,918 passed**, **1,108 opt-in ignored**, 167 successful result groups; zero failures or Rust warnings |
| Frontend | Next.js **15.5.28** production build and TypeScript passed; **323 tests passed** |
| SDK | Build, runtime, type and package-consumer checks passed; **42 tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, without exceptions or dependency upgrades |

Ignored tests are not passes. Node emitted 31 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings;
module policy was not changed to suppress them. Lockfile changes affect only the project's root
package version. The dependency graph, API routes/schemas and **0.3.0** compatibility baseline stay
unchanged. Management-tool fixtures are not installations or live-kernel acceptance.

## Isolated federation networking

The 0.6.7 binaries separately passed all **55** distinct cases through
`scripts/test-federation-mdns-netns.sh --with-cli --with-unicast --with-probe --with-controller --with-retained`.
The run combined 7 original discovery, 6 CLI, 10 unicast, 13 probing, 8 controller and 11 retained
cases. All traffic stayed inside two fresh disconnected anonymous network namespaces, with only
owned virtual links and no default route or host-LAN attachment.

Ordinary cases ran with all five capability sets zero. Separate interface/address mutation cases
retained only owned-netns NET_ADMIN; the parent restored exact fixture names/addresses before
subsequent cases. The runner stopped its owned peer and discarded the namespaces. No application
database, host account, system daemon or host network interface was modified.

The cases check packet layout/source/destination/TTL rejection, port exclusivity, descriptor release,
probe timing, conflict cooldown, retained socket identity, late queued claims, cancellation, fixed
review expiry and interface changes. Peers use synthetic claims. See the
[restricted discovery profile](../../../docs/en/federation-discovery.md) for exact API and timing limits.

## Source and deployment boundaries

F1h history is per controller, not a process-wide rate limit. F1i is a separate one-attempt entry:
it retains original sockets and accounting for receive-only review, without inheriting the F1h guard
or renewing the original lease/timeout. Idle expired handles retain descriptors until close/Drop
or a closing review. Quiet observations and retained handles grant no publication or resource access.

No announcement/defense/withdrawal service, startup discovery, verified pairing, traversal, relay
transport or cross-authority workspace access is enabled. Isolated fixture success is not physical-LAN
interoperability or decentralized deployment acceptance. Earlier browser/SQL/TLS evidence retains
its dated scope in the [testing guide](../../../docs/en/testing-guide.md); it was not rerun by this gate.

Remote CI, offline images/packages, extracted-package/kernel smoke and installed-service upgrades
have separate acceptance boundaries. Publishing this source tag does not create a GitHub Release
or deploy a service. No new live RustSec audit, benchmark or opt-in SQL/browser/kernel run is claimed.
