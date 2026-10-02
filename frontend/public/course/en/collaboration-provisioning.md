# ADR-011: Controlled local authority provisioning

Status: **C1-J internal staging, included in 0.4.7**. This adds the native
`cyanrex-provision` operator interface to [C1-I atomic bootstrap](collaboration-bootstrap.md).
It makes a fresh authority deliberately provisionable without creating a public setup endpoint,
changing the running Engine, migrating accounts or selecting a first public registrant as teacher.

## Operator boundary

Use this tool only on an explicitly prepared **isolated staging database and fresh named schema**.
The schema must already exist and contain no relations, functions or types. `public`, system schemas,
multi-schema paths, missing schemas, existing data and partial installations are rejected. The CLI
does not create/drop a schema or adopt/repair an old installation. Do not point the legacy live
AuthService at these activated tables: public authentication/CSRF integration is still pending.

The tool is Unix-only (Linux/WSL2); it accepts a local PostgreSQL socket or exactly `127.0.0.1`/`::1`.
TCP uses the explicitly supplied port and no TLS, so it is for a trusted local endpoint, including an
independently established, verified SSH forward; there is no remote-host/DNS option or automatic tunnel.
Endpoint selection is not server authentication. Verify the socket/forward, database and PostgreSQL
role independently. The physical fingerprint is a review safeguard, not a certificate or defense
against a hostile database administrator or a cloned database with identical identifiers.

All connection settings come from a private JSON file. The standalone process clears inherited `PG*`
before starting Tokio and uses SQLx without pgpass lookup; it does not load `.env`, `DATABASE_URL`,
`CYANREX_*`, Engine state or old home-account configuration. Statements are not logged by the CLI.
PostgreSQL server logging, swap, process memory, a compromised OS/root or another process with the
same user identity remain outside this secret-delivery boundary.

The selected database role needs USAGE/CREATE on the fresh schema and permission to execute
`pg_catalog.pg_control_system()` for its system identifier. A trusted database administrator must
prepare those privileges; the CLI does not grant itself permissions. Source builds expose the new
binary through Cargo; release packaging, launchers, Compose and service startup are unchanged.

## Prepare and review

Build with `cargo build --manifest-path engine/Cargo.toml --locked --bin cyanrex-provision`, or use
`cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-provision --` before each
command's arguments. No build or help invocation initializes a database.

Prepare the files using a trusted local editor in a user-owned `0700` directory. Input files must be
owned regular files with no group/other access, no special permission bits and one hard link
(normally `0600` or read-only `0400`). Paths must be absolute, with trusted directory ownership and
no symlink traversal or parent-directory escape. Configuration is limited to 16 KiB. This example is
synthetic: choose and retain fresh canonical UUIDs for the actual authority and Workspace, and fill
in the intended database credentials; never commit the file or paste it into logs.

```json
{
  "format_version": 1,
  "transport": { "kind": "unix", "directory": "/var/run/postgresql" },
  "port": 5432,
  "database": "cyanrex_staging",
  "database_user": "cyanrex_operator",
  "database_password": "<DB password, or empty string for peer authentication>",
  "schema": "c1_trial",
  "authority_id": "1223c6e9-823d-4cea-998a-a92f019b35fc",
  "workspace_id": "60a9845d-0687-4f19-a4f5-2ebf64bed4ba",
  "username": "first-teacher"
}
```

For a verified local TCP endpoint, replace `transport` with
`{"kind":"loopback","address":"127.0.0.1"}`. Database/role names are bounded ASCII identifiers;
schema names use lowercase letters, digits and underscores and cannot start with a digit.
Unknown/duplicate JSON fields, unsupported format versions and malformed IDs fail closed.

```bash
cyanrex-provision plan --config /absolute/private/target.json
```

`plan` and `inspect` use read-only, repeatable-read catalog transactions. A plan shows the exact
endpoint, database/role/schema, authority/Workspace/username, physical system/database/schema
identifiers and fixed initial policy: active owner/teacher membership, an explicit deployment grant,
two audit baselines and **no Session**. It returns a SHA-256 `confirmation` only for an empty namespace
with schema privileges. The digest binds those non-secret targets and policy, not passwords.
Neither a plan nor its digest reserves the namespace or authorizes an untrusted caller.

## Explicit apply and enrollment delivery

Create a separate private password file containing 8–4096 UTF-8 bytes. One final LF/CRLF is a delimiter;
spaces are preserved, while embedded newline/CR/NUL is rejected. Review the plan yourself, then copy
its digest into the explicit command; do not turn planning and execution into an unattended pipe.

```bash
cyanrex-provision apply --config /absolute/private/target.json \
  --confirm <reviewed-64-character-digest> \
  --password-file /absolute/private/password \
  --enrollment-file /absolute/private/NEW-enrollment.json
```

Apply independently rechecks the target/emptiness and confirmation. Replacement pool connections
recheck physical identity; C1-I remains the transaction authority and rechecks absence behind all
three installer fences. A changed plan or occupied schema cannot initialize an account. Concurrent
processes have at most one winner; a plan does not bypass current database checks.

Before dispatch, the tool exclusively creates a `0600` enrollment file under an owned `0700` parent
and syncs a **non-secret pending-reconciliation marker** and directory. Existing paths, symlinks,
hard links, nonregular files and permissive output parents are not adopted or overwritten. File and
parent descriptors are checked against their visible identities when writing.

Only after C1-I confirms COMMIT does the held file receive the account/Principal coordinates and TOTP
enrollment material. File contents and directory are synced before success. Console JSON reports
`committed_enrollment_saved` and non-secret coordinates; it never contains the DB password, account
password, TOTP secret, enrollment URI or a Session token. Import the private enrollment material into
the intended authenticator and retain it according to your secret-storage policy. Normal password/TOTP
login is still required; this does not switch the live Engine to the new account.

## Failure handling and inspection limits

Database commit and filesystem delivery are **not one atomic transaction**. The CLI never removes an
output marker, retries bootstrap, resets credentials or creates a replacement manager automatically.

| Outcome | Meaning | Required response |
|---|---|---|
| Input/preflight error, exit 2 | Bootstrap was not dispatched; output reservation failure can leave an empty/partial marker | Correct explicit inputs; review a new plan and preserve any file for inspection |
| `bootstrap_unconfirmed`, exit 4 | Bootstrap was dispatched but commit was not confirmed; a marker is not proof of rollback | Preserve evidence and reconcile the exact target before any deliberate retry |
| `committed_delivery_unconfirmed`, exit 3 | Database commit was confirmed, but file delivery or final console acknowledgement failed | Do not reinitialize; preserve complete/partial private output and investigate delivery |
| Successful exit 0 | Read-only observation, or confirmed bootstrap plus saved enrollment for `apply` | Interpret the command's status; success is not live deployment acceptance |

```bash
cyanrex-provision inspect --config /absolute/private/target.json
```

Inspect reports catalog occupancy and physical target identifiers. `empty` is only a read-time view:
another initialization can still be uncommitted/in flight. `occupied` does not prove a healthy
initialized authority, which command committed, who acted or whether a secret was delivered. It does
not read or resend stored TOTP, validate the complete audit graph, repair data or issue a replay receipt.
After an uncertain write, trusted maintenance must first establish that no operation is still in
flight and reconcile the current source/registry/audit state. A repeated bootstrap never recovers a
lost secret. [C1-K](collaboration-reconciliation.md) now provides `reconcile` for the bounded read-only
source/registry/full audit graph, not secret delivery, command-outcome proof or retry permission.
Enrollment recovery and reviewed migration/restore remain separate gates; do not delete tables,
change IDs or remove evidence to force a retry.

## Verification

The [CLI suite](../../engine/tests/provision_cli_tdd.rs) covers argument/configuration rejection,
private bounded inputs, symlinks/hard links/FIFOs, unavailable storage, secret redaction and absence
of runtime fallback. The [PostgreSQL suite](../../engine/tests/provision_postgres_tdd.rs) covers nine
explicit scenarios: read-only planning/inspection, committed enrollment/login, stale target confirmation,
existing/partial state, unsafe output, competing processes, lock timeout, post-commit output replacement
and short writes, and process cancellation. All 8 default plus 9 real PostgreSQL cases passed locally
against disposable PostgreSQL 16 on 2026-10-02; CI selects every PostgreSQL case by exact name.
The suite also passed through loopback TCP with SCRAM password authentication. A non-superuser role
with database-enforced read-only transactions can plan/inspect, while its attempted apply creates no account.

The schema-filtered C1-I fault hooks remain coordinated through the test-only database advisory guard.
The short-write case uses a child-only file-size limit; no production failure-injection switch exists.
Full development regression totals are recorded in [Project Status](project-status.md). No live database,
frontend production build, migration, deployment, remote CI or privileged kernel acceptance is implied.
