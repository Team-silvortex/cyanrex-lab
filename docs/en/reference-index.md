# Cyanrex topic reference index

[简体中文](../zh-CN/reference-index.md)

Start with the [role based documentation paths](README.md). This index groups implementation
topics, operational guides and historical evidence without moving their files.
Task payload, code and execution are all optional.

## Read the capability status correctly

| Status | Meaning |
|---|---|
| Connected workflow | Teaching runtime or local authoring capability, within its own authority and saving boundaries. |
| Explicit host only | Requires separately configured low-privilege platform hosting, not normal teaching-app startup. |
| Explicit preparation | A trusted caller can compose a module, command or adapter; UI, SDK or live-service mounting is not implied. |
| Design target | Planned direction and intended boundaries, not an implemented or deployed capability. |
| Historical evidence | Supports only the recorded version, date and scope, not a fresh execution. |

Individual topics may retain their original implementation-stage labels. Use the current host
entry points and project status to determine actual runtime wiring.

## Project and architecture

Use the map for current wiring, the full architecture for implementation detail, and the target architecture for design direction. Dated maturity scores remain evidence, not a deployment claim.

- [Current project status and next decisions](project-status.md)
- [Current architecture and change boundaries](architecture-map.md)
- [Detailed system architecture](architecture.md)
- [Target platform architecture](next-architecture.md)
- [Current platform feature and module network](platform-network.md)
- [Capability tensor and dated maturity evidence](capability-maturity.md)
- [Platform and eBPF concepts](concepts.md)
- [Desktop and LAN classroom isolation design](classroom-isolation.md)

## Platform hosts and storage

The separate loopback hosts are callable only through explicit private configuration. They do not mount a generic backend in the teaching application, migrate authentication or establish LAN/browser acceptance.

### Explicit host entry points

- [Plain loopback Session and private Task host](platform-loopback-host.md)
- [Native HTTPS loopback host](platform-loopback-tls-host.md)
- [Optional service state notifications](platform-host-notifications.md)
- [Limited login over retained storage](platform-login-connection.md)
- [Confirmed login and dedicated Cookie projection](platform-login-http.md)
- [Shared login and Session route composition](platform-session-router.md)
- [Owner private Task index connection](platform-task-index-connection.md)
- [Exact Task content reopening](platform-task-content-connection.md)
- [Empty manual Draft creation](platform-task-create-connection.md)
- [Expected revision Draft manifest replacement](platform-task-replace-connection.md)

### Startup and retained storage gates

- [Linux process identity and privilege checks](platform-process-startup.md)
- [Private startup input validation](platform-startup-input.md)
- [Host catalog preflight](platform-host-preflight.md)
- [Original catalog identity declarations](platform-catalog-expectation.md)
- [Original storage marker declarations](platform-marker-expectation.md)
- [Private Artifact root observations](platform-artifact-root.md)
- [Storage version and scope observations](platform-storage-markers.md)
- [Explicit local database connector](platform-database-connection.md)
- [One shot read only storage diagnosis](platform-storage-check.md)
- [Fresh connections with retained storage bindings](platform-storage-connection.md)
- [Expected storage opener and capability handoff](platform-storage-open.md)

## Identity and Session authority

These contracts and explicit services distinguish trusted stores from current-Session authorization. Only the selected host adapters connect the supported commands; teaching login remains its own runtime path.

- [Collaboration foundation and legacy baseline](collaboration-foundation.md)
- [Durable collaboration identity staging](collaboration-identity-store.md)
- [Membership and deployment policy staging](collaboration-access-store.md)
- [Attributed policy commands and audit](collaboration-policy-audit.md)
- [Identity binding and retirement](collaboration-identity-lifecycle.md)
- [Durable account incarnations and Session source](collaboration-auth-source.md)
- [Session authorized collaboration transactions](collaboration-session-commands.md)
- [Restricted account deletion](collaboration-account-deletion.md)
- [Password change and Session revocation](collaboration-password-change.md)
- [Atomic fresh authority initialization](collaboration-bootstrap.md)
- [Controlled local provisioning CLI](collaboration-provisioning.md)
- [Read only lifecycle reconciliation](collaboration-reconciliation.md)
- [Explicit Session stock limits](durable-session-stock.md)
- [Current Session query and logout](platform-session-lifecycle.md)
- [Session lifecycle database privileges](platform-session-permissions.md)
- [Existing Sessions over retained storage](platform-session-connection.md)

## Tasks Artifacts and Reviews

Code and execution are optional. Resource publication, input replacement, a Run and a Review are different operations. The separate adapters below are preparation boundaries, not a generic browser save path.

### Core contracts and resource commands

- [Domain neutral Task definitions and typed assessment](task-domain-boundary.md)
- [Durable Task instances](task-instance-store.md)
- [Immutable Artifact revisions](artifact-revision-store.md)
- [Revision bound Review records](review-record-store.md)
- [Private manual Task commands](session-task-commands.md)
- [Private Artifact commands](session-artifact-commands.md)
- [Exact private Task inputs](session-task-inputs.md)
- [Draft input replacement](session-task-revisions.md)
- [Catalogue Task admission](session-catalog-tasks.md)
- [Private human Review commands](session-review-commands.md)

### Content and local authoring

- [Task content manifests and exact references](task-content-manifest.md)
- [Content persistence and atomic editing](task-content-store.md)
- [Owner private content metadata index](task-content-index.md)
- [Current Session content and byte verification](session-task-content.md)
- [Local Task payload editor](editor.md)
- [Reviewed local multi file text import](local-text-file-import.md)

### Separate HTTP and publication adapters

- [Explicit manual content HTTP adapter](task-content-http.md)
- [Private content index HTTP adapter](task-content-index-http.md)
- [Exact catalogue content HTTP adapter](catalog-task-content-http.md)
- [Text Artifact publication and exact revision HTTP](text-artifact-http.md)
- [Retained text publication, HTTP and explicit host composition](platform-text-artifact-connection.md)
- [Strict draft mapping and unmounted content-save attempt](task-draft-publication.md)
- [Manual one step draft publication](session-task-draft-publication.md)
- [Read only observation of unknown draft targets](session-task-draft-observation.md)
- [Draft metadata checkpoints](session-task-draft-checkpoint.md)
- [Explicit checkpoint target inspection](session-task-draft-inspection.md)
- [Immutable Session draft intent records](session-task-draft-intent.md)
- [Journaled draft publication dispatch](session-task-draft-dispatch.md)
- [Read only draft journal inspection](session-task-draft-journal-inspection.md)
- [Recorded step resource observation](session-task-draft-journal-observation.md)
- [Frozen catalogue draft publication attempt](session-catalog-task-draft-publication.md)
- [Catalogue attempt target observation](session-catalog-task-draft-observation.md)

## Large file preparation

The proposal covers centralized distribution first and later desktop transfers. Implemented components can be composed explicitly, including controlled Source/HTTPS delivery checks; normal-app distribution, UI saving, resume and desktop transfer are not connected.

### Upload and publication

- [Distribution proposal and scope](large-file-distribution.md)
- [Manifest and streaming core](large-file-core.md)
- [Private chunk staging](large-file-staging.md)
- [Upload intent and logical quotas](large-file-upload-intent.md)
- [Upload bound private staging receipts](large-file-upload-staging.md)
- [Durable chunk write ahead journal](large-file-chunk-journal.md)
- [Current upload whole byte verification](large-file-whole-verification.md)
- [Persistent whole file publication metadata](large-file-publication.md)
- [Current Session private metadata reads](large-file-session-metadata.md)
- [Current Session authorized publication](large-file-session-publication.md)

### Grants and authorized reads

- [Pure publisher and recipient grant contracts](large-file-access-contract.md)
- [Persistent publisher grant ledger](large-file-publisher-grants.md)
- [Explicit manager grant commands](large-file-grant-management.md)
- [Persistent exact file recipient grants](large-file-recipient-grants.md)
- [Publisher owned sharing and revocation](large-file-sharing.md)
- [Authorized exact download manifest observation](large-file-download-manifest.md)
- [Authorized verified chunk snapshots](large-file-chunk-read.md)

### Transport and saving

- [Private single chunk binary HTTP adapter](large-file-chunk-http.md)
- [Verified response reception and private staging](large-file-chunk-receiver.md)
- [Fixed origin controlled HTTP chunk client](large-file-chunk-client.md)
- [Ordered private whole file assembly](large-file-chunk-assembly.md)
- [Ordered download client](large-file-download-client.md)
- [Recipient manifest HTTP adapter](large-file-manifest-http.md)
- [Exact manifest client and granted download](large-file-manifest-client.md)
- [Explicit private file saving](large-file-save.md)
- [Authorized download and private saving](large-file-delivery-client.md)
- [Explicit owned ordinary directory target](large-file-user-save.md)
- [Latest delivery progress observation](large-file-delivery-progress.md)
- [Cooperative cancellation of the waiting call](large-file-delivery-cancellation.md)
- [Controlled HTTPS delivery verification](large-file-delivery-https.md)
- [Explicit manifest and chunk router composition](large-file-distribution-router.md)
- [Actual Source authorization with HTTPS delivery](large-file-source-https.md)

## Teaching runtime and operations

These guides describe the connected teaching domain and operational controls. They do not turn general private resources into teacher-owned coursework or isolate untrusted kernel execution.

- [Teacher workflow and deployment authority](teacher-guide.md)
- [Student and personal learning workflow](student-guide.md)
- [Launchers offline packages APIs and persistence](runtime-reference.md)
- [SSH deployment and student classroom entry](classroom-connection.md)
- [Runner Agent operations](runner-agent.md)
- [Security and deployment boundaries](security.md)
- [Troubleshooting by operational boundary](troubleshooting.md)
- [Learning records and query costs](learning-storage.md)
- [Event streams and recovery](event-stream.md)
- [Interface translation and course fallback](localization.md)
- [Lab 1 execution pipeline](labs/01-first-program.md)
- [Lab 2 execve observations](labs/02-trace-execve.md)
- [Lab 3 Map counters](labs/03-map-counter.md)
- [Lab 4 Ring Buffer events](labs/04-ring-buffer.md)
- [Lab 5 verifier diagnostics](labs/05-verifier-debugging.md)

## SDK and developer tools

The SDK and AI Agent helpers adapt their supported Engine surface. They are not autonomous orchestration, generic platform save integration or an execution grant.

- [AI Agent profiles and explicit tool adapters](ai-agent-integration.md)
- [JavaScript SDK](../../sdk-js/README.md)
- [Declarative module catalogue](../../modules/README.md)
- [Contribution workflow and boundaries](../../CONTRIBUTING.md)
- [TDD policy](../../TDD.md)
- [Validation and operational scripts](../../scripts/README.md)

## Testing and history

Current test instructions are separate from dated outcomes. Archives retain their original implementation stages, versions and failure counts; a historical pass is not fresh acceptance.

### Current verification

- [Current test matrix and explicit boundary suites](testing-guide.md)
- [Release contents and Unreleased changes](../../CHANGELOG.md)

### Archived status and evidence

- [Original 0.6.3 status and boundary snapshot](history/project-status-snapshot-0-6-3.md)
- [Validation records from October 8 through 11](history/project-status-validation-2026-10-08-11.md)
- [Validation records from October 7 and earlier](history/project-status-validation-2026-10-07-and-earlier.md)
- [Development history](development-history.md)
- [Historical acceptance checklist](acceptance.md)
- [Historical module and boundary test network](testing-network.md)
- [Historical functional network baseline](functional-network.md)

### Chain guided bug hunts

- [01 Learning and persistence](functional-network-bug-hunt-01.md)
- [02 Runner and Agent lifecycle](functional-network-bug-hunt-02.md)
- [03 Browser compiler diagnostics](functional-network-bug-hunt-03.md)
- [04 Completion and header checks](functional-network-bug-hunt-04.md)
- [05 Manual run and attachment cleanup](functional-network-bug-hunt-05.md)
- [06 Debug sessions and event recovery](functional-network-bug-hunt-06.md)
- [07 Event history and acknowledgement](functional-network-bug-hunt-07.md)
- [08 Event retention and persistence ordering](functional-network-bug-hunt-08.md)
- [09 Cold admission and writer lifetime](functional-network-bug-hunt-09.md)
- [10 Honest reads and repair retry](functional-network-bug-hunt-10.md)
- [11 Mutation confirmation and cancellation](functional-network-bug-hunt-11.md)
- [12 Retention and overflow confirmation](functional-network-bug-hunt-12.md)
- [13 Settings reads and partial saves](functional-network-bug-hunt-13.md)
- [14 Metrics lifecycle and panel isolation](functional-network-bug-hunt-14.md)
- [15 Runner inventory and cancellation](functional-network-bug-hunt-15.md)
