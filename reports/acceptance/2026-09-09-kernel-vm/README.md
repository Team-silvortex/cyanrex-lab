# Disposable VM kernel checkpoint — 2026-09-09

Result: **passed**, on a separately created and explicitly approved QEMU/KVM guest. Existing host
deployments and the unrelated existing VM were not modified. This is a dirty-working-tree **native dev
build**, not an optimized release, offline package or Tag candidate. The native report has `candidate: null`.

## Evidence

- `live-kernel.json`: native v2 evidence from `cyanrex-release smoke live-kernel`; verified in the guest
  and again on the host. Aya attached the built-in `ringbuf-hi-freq-sampler` to `sched/sched_switch`, a
  uniquely matched 24-byte event reached authenticated history, and exact detach left zero attachments.
- `kernel-programs-{before,after}.json` and `kernel-links-{before,after}.json`: independent `bpftool -j`
  inventories taken before loading and after the native smoke returned, while Engine was still running.
  The same 12 program IDs and one link ID remained. Separate filesystem checks found no pinned bpffs files.
- `metadata.json`: VM configuration, observed network control, image provenance, binary hashes and limits.
- `source-initial.sha256`: all 145 files in `engine/src`, `engine/openapi`, `engine/migrations`, plus
  `engine/Cargo.toml` and `engine/Cargo.lock`, frozen before the test. This is an Engine input manifest,
  not a full repository, environment or reproducible-build attestation. No Engine source was changed
  during this checkpoint.
- `kernel-smoke.txt`: output of the successful native smoke, evidence verification and bpffs check.

## Method and boundaries

The native binaries were built using `cargo build --manifest-path engine/Cargo.toml --locked
--bin cyanrex-engine --bin cyanrex-release --jobs 2`. Only copied binaries had their debug information
stripped; originals were preserved. Transferred binary hashes matched on both sides. Guest packages
were installed/upgraded from the image's Ubuntu repositories before a clean shutdown/restart into
QEMU restricted user networking, with only a loopback SSH forward and pinned SSH host key.

An owned host-loopback HTTP canary was reachable from the guest during setup and failed with curl
exit 7 after the isolated restart. The host could still reach the canary immediately before and after
the rejection. Its server was then stopped. This verifies that specific guest-to-host path, not every
network path or resistance to a hypervisor escape.

Guest root mounted bpffs/tracefs and ran Engine on guest `127.0.0.1:8080`, using newly generated admin
and TOTP secrets passed only through process environment. No real deployment secrets, database,
frontend, external Agent or student data were copied. Inside that guest, the existing Runner still
uses the shared **guest** kernel; this setup does not implement the proposed per-student VM driver.

The native command was:

```sh
timeout 120 ./cyanrex-release smoke live-kernel \
  --engine-url http://127.0.0.1:8080 --origin http://localhost:3000 \
  --report /opt/cyanrex-acceptance/live-kernel.json
./cyanrex-release evidence verify /opt/cyanrex-acceptance/live-kernel.json
```

These commands require a prepared disposable guest, a running privileged Engine and its synthetic
admin/TOTP environment. Do not run them against an existing classroom or host deployment.

Engine stopped after recording the inventories; a final check found no Engine listener or bpffs pins.
The guest was then powered off, its SSH forward disappeared and `qemu-img check` passed. The private
disk, setup files and SSH key remain outside Git in a temporary directory; they are not a backup or
published artifact. Fresh guest state is required for a later package installation test.

Still pending: clean-candidate/offline installation acceptance, bpftool/XDP and additional kernel
coverage, real LAN student/teacher browser flows, restart persistence and product-managed VM ownership.

## References

Image provenance follows Ubuntu's [checksum and signature verification](https://ubuntu.com/docs/public-images/public-images-how-to/verify-image-checksum/)
using the [dated minimal image release](https://cloud-images.ubuntu.com/minimal/releases/noble/release-20260905/).
The network option is described in [QEMU invocation documentation](https://www.qemu.org/docs/master/system/invocation.html).
Operator summaries are in `docs/en/acceptance.md` and `docs/zh-CN/acceptance.md`.
