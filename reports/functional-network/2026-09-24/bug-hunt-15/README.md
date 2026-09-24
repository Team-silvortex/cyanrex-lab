# Runner administration browser lifecycle evidence

See [machine record](../bug-hunt-15.json), [English narrative](../../../../docs/en/functional-network-bug-hunt-15.md)
and [中文记录](../../../../docs/zh-CN/functional-network-bug-hunt-15.md).

Base: 0.4.1 at `f56349b4412cb72d2330a7fae11ad597cf6e82a1`, already containing uncommitted passes 13/14.
This is not a clean release, live Engine/Agent acceptance or a full-project gate. Nothing was committed,
tagged, pushed or deployed. Browser networking is synthetic and non-fixture destinations are blocked.

- `red-browser-final.log`: final pre-runtime-change baseline, seven intended failures.
- `red-browser-first.log`: initial baseline with four incorrect button-label selectors.
- `red-browser.log`: intermediate baseline with one insufficient acknowledgement assertion.
- `green-first.log`: initial seven browser and eight unit cases pass.
- `runner-unit.log`: initial eight parser cases pass, before expanded bounds tests.
- `runner-unit-bounds-red.log`: impossible calendar date and nested label bounds rejected only after tightening the decoder.
- `expanded-browser.log`: 51 pass, one navigation assertion needed to wait for React dialog detachment.
- `expanded-final.log`: all 66 Runner/unit/settings/metrics cases pass.
- `browser-repeat-{1,2,3}.log`: 24 Runner browser cases pass in each independent run.
- `production-page-subset.log`: two existing production-page safety checks pass using intercepted APIs.
- `frontend-gate.log`: 69 common + 125 frontend tests, production build/type checks and clean production audit.
- `final-preflight.log`: final mirrors/contracts/metadata/common tests/tooling and Rust formatting; no Rust build.
- `map-drift.log`: expected frozen-map drift (exit 1), version plus 26 source fingerprints only.

`log-manifest.json` hashes exact bytes, including carriage returns and failed diagnostics.
`source-fingerprints.json` identifies selected frontend inputs and inspected backend contracts.
`frozen-receipt.json` preserves all 361 earlier evidence/map/document files byte-for-byte.
`cleanup.json` records the owned local preview and recoverable generated-cache cleanup.

No real credentials, production data, SSH/LAN, Agent execution, database or kernel operations were used.
