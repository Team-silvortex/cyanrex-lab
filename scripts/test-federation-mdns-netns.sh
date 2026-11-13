#!/usr/bin/env bash
set -euo pipefail

if [[ ${1:-} == --help && $# == 1 ]]; then
  printf '%s\n' 'Usage: scripts/test-federation-mdns-netns.sh [--with-cli] [--with-unicast] [--with-probe] [--with-controller] [--with-retained] [--with-controller-retained] [--with-probe-cli]' 'Build locked/offline; test only inside a fresh anonymous user/network namespace. No host LAN fallback.' '--with-cli requires existing subordinate ID delegation and mapping helpers; never edits it.' '--with-unicast explicitly adds receive-only exact-local socket acceptance.' '--with-probe explicitly adds bounded, one-shot probing; no announcement or ownership permit.' '--with-controller explicitly adds finite opt-in retries with conflict cooldown; no publication authority.' '--with-retained explicitly adds short-lived receive-only review of one quiet round; no announcement or ownership permit.' '--with-controller-retained explicitly adds controller-to-retained quiet handoff; no announcement or ownership permit.' '--with-probe-cli explicitly adds the non-root single-round probe CLI and requires existing CLI mapping prerequisites; no publication authority.'
  exit 0
fi
task_mdns_with_cli=0
task_mdns_with_unicast=0
task_mdns_with_probe=0
task_mdns_with_controller=0
task_mdns_with_retained=0
task_mdns_with_controller_retained=0
task_mdns_with_probe_cli=0
for task_mdns_argument in "$@"; do
  case "$task_mdns_argument" in
    --with-cli) [[ "$task_mdns_with_cli" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_cli=1 ;;
    --with-unicast) [[ "$task_mdns_with_unicast" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_unicast=1 ;;
    --with-probe) [[ "$task_mdns_with_probe" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_probe=1 ;;
    --with-controller) [[ "$task_mdns_with_controller" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_controller=1 ;;
    --with-retained) [[ "$task_mdns_with_retained" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_retained=1 ;;
    --with-controller-retained) [[ "$task_mdns_with_controller_retained" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_controller_retained=1 ;;
    --with-probe-cli) [[ "$task_mdns_with_probe_cli" == 0 ]] || { printf '%s\n' 'Duplicate arguments.' >&2; exit 2; }; task_mdns_with_probe_cli=1 ;;
    *) printf '%s\n' 'Unexpected arguments.' >&2; exit 2 ;;
  esac
done
task_mdns_mapped_cli=0
if [[ "$task_mdns_with_cli" == 1 || "$task_mdns_with_probe_cli" == 1 ]]; then
  task_mdns_mapped_cli=1
fi
task_mdns_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
for task_mdns_tool in cargo node unshare ip nsenter setpriv readlink python3; do
  command -v "$task_mdns_tool" >/dev/null || { printf 'Missing tool: %s\n' "$task_mdns_tool" >&2; exit 1; }
done
task_mdns_namespace=(--user --map-root-user --net --fork)
task_mdns_preexec=()
task_mdns_targets=(--test federation_mdns_network_tdd)
if [[ "$task_mdns_with_unicast" == 1 ]]; then
  task_mdns_targets+=(--test federation_mdns_unicast_network_tdd)
fi
if [[ "$task_mdns_with_probe" == 1 ]]; then
  task_mdns_targets+=(--test federation_mdns_probing_network_tdd)
fi
if [[ "$task_mdns_with_controller" == 1 ]]; then
  task_mdns_targets+=(--test federation_mdns_controller_network_tdd)
fi
if [[ "$task_mdns_with_retained" == 1 ]]; then
  task_mdns_targets+=(--test federation_mdns_retained_network_tdd)
fi
if [[ "$task_mdns_with_controller_retained" == 1 ]]; then
  task_mdns_targets+=(--test federation_mdns_controller_retained_network_tdd)
fi
if [[ "$task_mdns_with_cli" == 1 ]]; then
  task_mdns_targets+=(--test federation_discovery_cli_network_tdd)
fi
if [[ "$task_mdns_with_probe_cli" == 1 ]]; then
  task_mdns_targets+=(--test federation_probe_cli_network_tdd)
fi
if [[ "$task_mdns_mapped_cli" == 1 ]]; then
  for task_mdns_tool in getsubids newuidmap newgidmap; do
    command -v "$task_mdns_tool" >/dev/null || { printf '%s\n' 'CLI namespace mapping prerequisite unavailable.' >&2; exit 1; }
  done
  task_mdns_uid=$(id -u)
  task_mdns_gid=$(id -g)
  [[ "$task_mdns_uid" != 0 && "$task_mdns_gid" != 0 ]] || { printf '%s\n' 'CLI namespace fixture requires a non-root invoking identity.' >&2; exit 1; }
  task_mdns_account=$(id -un)
  task_mdns_delegated_id() {
    node --input-type=module -e '
import {readFileSync} from "node:fs";
const own=Number(process.argv[1]);
const lines=readFileSync(0,"utf8").trim().split("\n");
for(const line of lines){
  const fields=line.trim().split(/\s+/);
  if(fields.length!==4||!/^\d+:$/.test(fields[0])||!/^\d+$/.test(fields[2])||!/^\d+$/.test(fields[3]))process.exit(1);
  const start=Number(fields[2]),count=Number(fields[3]);
  if(!Number.isSafeInteger(start)||!Number.isSafeInteger(count)||start<1||count<1||start+count>4294967295)process.exit(1);
  const selected=start===own?start+1:start;
  if(selected<start+count){process.stdout.write(String(selected));process.exit(0);}
}
process.exit(1);' "$1"
  }
  if ! task_mdns_subuid=$(getsubids "$task_mdns_account" 2>/dev/null | task_mdns_delegated_id "$task_mdns_uid") \
    || ! task_mdns_subgid=$(getsubids -g "$task_mdns_account" 2>/dev/null | task_mdns_delegated_id "$task_mdns_gid"); then
    printf '%s\n' 'CLI namespace fixture requires an existing usable subordinate UID and GID.' >&2; exit 1
  fi
  # A cap-free driver must not map its parent UID 0 into another user namespace.
  task_mdns_namespace=(--user "--map-users=1000:$task_mdns_uid:1" "--map-users=0:$task_mdns_subuid:1"
    "--map-groups=1000:$task_mdns_gid:1" "--map-groups=0:$task_mdns_subgid:1" --setgroups=allow --keep-caps --net --fork)
  task_mdns_preexec=(setpriv --clear-groups)
fi
task_mdns_logs=$(mktemp -d /tmp/cyanrex-mdns-netns-XXXXXX)
chmod 0700 "$task_mdns_logs"
task_mdns_parent=$(readlink /proc/self/ns/net)
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path "$task_mdns_root/engine/Cargo.toml" --locked --offline \
    "${task_mdns_targets[@]}" --no-run --message-format=json > "$task_mdns_logs/build.jsonl"
task_mdns_exact_binary() {
  node --input-type=module -e '
import {readFileSync} from "node:fs";
const values=readFileSync(process.argv[1],"utf8").trim().split("\n").map(line=>JSON.parse(line));
const artifacts=values.filter(item=>item.reason==="compiler-artifact"&&item.target?.name===process.argv[2]&&item.executable);
if(artifacts.length!==1)throw new Error("Expected one exact test executable");
process.stdout.write(artifacts[0].executable);' "$task_mdns_logs/build.jsonl" "$1"
}
task_mdns_binary=$(task_mdns_exact_binary federation_mdns_network_tdd)
[[ "$task_mdns_binary" == "$task_mdns_root/engine/target/"* && -x "$task_mdns_binary" ]] || exit 1
task_mdns_cli_binary=
if [[ "$task_mdns_with_cli" == 1 ]]; then
  task_mdns_cli_binary=$(task_mdns_exact_binary federation_discovery_cli_network_tdd)
  [[ "$(dirname "$task_mdns_cli_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_cli_binary")" =~ ^federation_discovery_cli_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_cli_binary" ]] || exit 1
fi
task_mdns_unicast_binary=
if [[ "$task_mdns_with_unicast" == 1 ]]; then
  task_mdns_unicast_binary=$(task_mdns_exact_binary federation_mdns_unicast_network_tdd)
  [[ "$(dirname "$task_mdns_unicast_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_unicast_binary")" =~ ^federation_mdns_unicast_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_unicast_binary" ]] || exit 1
fi
task_mdns_probe_binary=
if [[ "$task_mdns_with_probe" == 1 ]]; then
  task_mdns_probe_binary=$(task_mdns_exact_binary federation_mdns_probing_network_tdd)
  [[ "$(dirname "$task_mdns_probe_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_probe_binary")" =~ ^federation_mdns_probing_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_probe_binary" ]] || exit 1
fi
task_mdns_controller_binary=
if [[ "$task_mdns_with_controller" == 1 ]]; then
  task_mdns_controller_binary=$(task_mdns_exact_binary federation_mdns_controller_network_tdd)
  [[ "$(dirname "$task_mdns_controller_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_controller_binary")" =~ ^federation_mdns_controller_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_controller_binary" ]] || exit 1
fi
task_mdns_retained_binary=
if [[ "$task_mdns_with_retained" == 1 ]]; then
  task_mdns_retained_binary=$(task_mdns_exact_binary federation_mdns_retained_network_tdd)
  [[ "$(dirname "$task_mdns_retained_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_retained_binary")" =~ ^federation_mdns_retained_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_retained_binary" ]] || exit 1
fi
task_mdns_controller_retained_binary=
if [[ "$task_mdns_with_controller_retained" == 1 ]]; then
  task_mdns_controller_retained_binary=$(task_mdns_exact_binary federation_mdns_controller_retained_network_tdd)
  [[ "$(dirname "$task_mdns_controller_retained_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_controller_retained_binary")" =~ ^federation_mdns_controller_retained_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_controller_retained_binary" ]] || exit 1
fi
task_mdns_probe_cli_binary=
if [[ "$task_mdns_with_probe_cli" == 1 ]]; then
  task_mdns_probe_cli_binary=$(task_mdns_exact_binary federation_probe_cli_network_tdd)
  [[ "$(dirname "$task_mdns_probe_cli_binary")" == "$task_mdns_root/engine/target/debug/deps" \
    && "$(basename "$task_mdns_probe_cli_binary")" =~ ^federation_probe_cli_network_tdd-[a-f0-9]+$ \
    && -x "$task_mdns_probe_cli_binary" ]] || exit 1
fi

unshare "${task_mdns_namespace[@]}" "${task_mdns_preexec[@]}" bash -euo pipefail -c '
  task_mdns_binary=$1
  task_mdns_parent=$2
  task_mdns_helper=$3
  task_mdns_socket=$4
  task_mdns_with_cli=$5
  task_mdns_cli_binary=$6
  task_mdns_with_unicast=$7
  task_mdns_unicast_binary=$8
  task_mdns_with_probe=$9
  task_mdns_probe_binary=${10}
  task_mdns_with_controller=${11}
  task_mdns_controller_binary=${12}
  task_mdns_with_retained=${13}
  task_mdns_retained_binary=${14}
  task_mdns_with_controller_retained=${15}
  task_mdns_controller_retained_binary=${16}
  task_mdns_mapped_cli=${17}
  task_mdns_with_probe_cli=${18}
  task_mdns_probe_cli_binary=${19}
  task_mdns_current=$(readlink /proc/self/ns/net)
  [[ "$task_mdns_current" != "$task_mdns_parent" ]] || exit 1
  export CYANREX_MDNS_TEST_PARENT_NETNS="$task_mdns_parent"
  export CYANREX_MDNS_TEST_PEER_PARENT_NETNS="$task_mdns_current"
  export CYANREX_MDNS_TEST_PEER_SOCKET="$task_mdns_socket"
  task_mdns_peer_pid=
  task_mdns_cleanup() {
    if [[ -n "$task_mdns_peer_pid" ]]; then
      task_mdns_running=$(jobs -pr)
      if [[ $'\''\n'\''$task_mdns_running$'\''\n'\'' == *$'\''\n'\''$task_mdns_peer_pid$'\''\n'\''* ]]; then
        kill "$task_mdns_peer_pid" 2>/dev/null || true
      fi
      wait "$task_mdns_peer_pid" 2>/dev/null || true
    fi
  }
  trap task_mdns_cleanup EXIT
  trap "exit 130" INT
  trap "exit 143" TERM
  ip link set lo up
  ip link add crx0 type veth peer name crx1
  ip link add crx2 type veth peer name crx3
  # The peer opens only its private Unix RPC listener until the parent requests UDP I/O.
  unshare --net setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all \
    --no-new-privs python3 "$task_mdns_helper" "$task_mdns_socket" &
  task_mdns_peer_pid=$!
  task_mdns_peer_ready=0
  for task_mdns_attempt in {1..100}; do
    kill -0 "$task_mdns_peer_pid" 2>/dev/null || break
    task_mdns_peer_namespace=$(readlink "/proc/$task_mdns_peer_pid/ns/net" 2>/dev/null || true)
    if [[ -n "$task_mdns_peer_namespace" \
      && "$task_mdns_peer_namespace" != "$task_mdns_current" \
      && "$task_mdns_peer_namespace" != "$task_mdns_parent" \
      && -S "$task_mdns_socket" ]]; then
      task_mdns_peer_ready=1
      break
    fi
    sleep 0.01
  done
  [[ "$task_mdns_peer_ready" == 1 ]] || { printf "%s\n" "Isolated peer did not become ready." >&2; exit 1; }
  ip link set crx1 netns "$task_mdns_peer_pid"
  ip link set crx3 netns "$task_mdns_peer_pid"
  ip address add 10.23.0.1/24 dev crx0
  ip address add 10.24.0.1/24 dev crx2
  for task_mdns_device in crx0 crx2; do ip link set "$task_mdns_device" up; done
  nsenter -t "$task_mdns_peer_pid" -n ip link set lo up
  nsenter -t "$task_mdns_peer_pid" -n ip address add 10.23.0.2/24 dev crx1
  nsenter -t "$task_mdns_peer_pid" -n ip address add 10.24.0.2/24 dev crx3
  for task_mdns_device in crx1 crx3; do
    nsenter -t "$task_mdns_peer_pid" -n ip link set "$task_mdns_device" up
  done
  setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs \
    "$task_mdns_binary" --ignored --test-threads=1 --skip selected_address_removal_invalidates_the_socket_without_fallback
  if [[ "$task_mdns_with_cli" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_CLI=1 setpriv --bounding-set=-all --inh-caps=-all \
      --ambient-caps=-all --no-new-privs "$task_mdns_cli_binary" --ignored --test-threads=1
  fi
  if [[ "$task_mdns_with_probe_cli" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_PROBE_CLI=1 setpriv --bounding-set=-all --inh-caps=-all \
      --ambient-caps=-all --no-new-privs "$task_mdns_probe_cli_binary" --ignored --test-threads=1
  fi
  if [[ "$task_mdns_with_unicast" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_UNICAST=1 setpriv --bounding-set=-all --inh-caps=-all \
      --ambient-caps=-all --no-new-privs "$task_mdns_unicast_binary" --ignored --test-threads=1 \
      --skip selected_address_removal_invalidates_the_unicast_receiver_without_fallback
  fi
  if [[ "$task_mdns_with_probe" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_PROBE=1 setpriv --bounding-set=-all --inh-caps=-all \
      --ambient-caps=-all --no-new-privs "$task_mdns_probe_binary" --ignored --test-threads=1
  fi
  if [[ "$task_mdns_with_controller" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_CONTROLLER=1 setpriv --bounding-set=-all --inh-caps=-all \
      --ambient-caps=-all --no-new-privs "$task_mdns_controller_binary" --ignored --test-threads=1 \
      --skip a_renamed_pinned_interface_is_not_looked_up_during_cooldown_or_replaced_after_it
  fi
  if [[ "$task_mdns_with_retained" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_RETAINED=1 setpriv --bounding-set=-all --inh-caps=-all \
      --ambient-caps=-all --no-new-privs "$task_mdns_retained_binary" --ignored --test-threads=1 \
      --skip a_renamed_retained_interface_fails_review_and_releases_the_original_channels
  fi
  if [[ "$task_mdns_with_controller_retained" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_CONTROLLER_RETAINED=1 CYANREX_MDNS_TEST_WITH_RETAINED=1 \
      setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs \
      "$task_mdns_controller_retained_binary" --ignored --test-threads=1 \
      --skip a_renamed_controller_retained_interface_closes_and_releases_original_channels
  fi
  task_mdns_mutation_caps=(--inh-caps=-all --ambient-caps=-all)
  if [[ "$task_mdns_mapped_cli" == 1 ]]; then
    task_mdns_mutation_caps=(--inh-caps=-all,+net_admin --ambient-caps=-all,+net_admin)
  fi
  if [[ "$task_mdns_with_controller_retained" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_CONTROLLER_RETAINED=1 CYANREX_MDNS_TEST_WITH_RETAINED=1 \
      CYANREX_MDNS_TEST_MUTATE=1 setpriv --bounding-set=-all,+net_admin \
      "${task_mdns_mutation_caps[@]}" --no-new-privs \
      "$task_mdns_controller_retained_binary" --ignored --test-threads=1 \
      --exact a_renamed_controller_retained_interface_closes_and_releases_original_channels
    # Restore the exact fixture name before the independently selected older mutation cases.
    ip link set dev crx4 name crx0
  fi
  if [[ "$task_mdns_with_retained" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_RETAINED=1 CYANREX_MDNS_TEST_MUTATE=1 \
      setpriv --bounding-set=-all,+net_admin "${task_mdns_mutation_caps[@]}" --no-new-privs \
      "$task_mdns_retained_binary" --ignored --test-threads=1 \
      --exact a_renamed_retained_interface_fails_review_and_releases_the_original_channels
    # Restore only the renamed retained fixture before the unchanged older mutation cases.
    ip link set dev crx4 name crx0
  fi
  if [[ "$task_mdns_with_controller" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_CONTROLLER=1 CYANREX_MDNS_TEST_MUTATE=1 \
      setpriv --bounding-set=-all,+net_admin "${task_mdns_mutation_caps[@]}" --no-new-privs \
      "$task_mdns_controller_binary" --ignored --test-threads=1 \
      --exact a_renamed_pinned_interface_is_not_looked_up_during_cooldown_or_replaced_after_it
    # Restore only the renamed fixture interface inside this owned namespace for old cases.
    ip link set dev crx4 name crx0
  fi
  if [[ "$task_mdns_with_unicast" == 1 ]]; then
    CYANREX_MDNS_TEST_WITH_UNICAST=1 CYANREX_MDNS_TEST_MUTATE=1 \
      setpriv --bounding-set=-all,+net_admin "${task_mdns_mutation_caps[@]}" --no-new-privs \
      "$task_mdns_unicast_binary" --ignored --test-threads=1 \
      --exact selected_address_removal_invalidates_the_unicast_receiver_without_fallback
    # Restore only the fixture address inside this owned namespace for the unchanged final case.
    ip address add 10.23.0.1/24 dev crx0
  fi
  CYANREX_MDNS_TEST_MUTATE=1 setpriv --bounding-set=-all,+net_admin "${task_mdns_mutation_caps[@]}" \
    --no-new-privs "$task_mdns_binary" --ignored \
    --exact selected_address_removal_invalidates_the_socket_without_fallback --test-threads=1
  # All interfaces and routes above belong only to this anonymous namespace and die with it.
' cyanrex-mdns-netns "$task_mdns_binary" "$task_mdns_parent" \
  "$task_mdns_root/scripts/tests/federationMdnsPeer.py" "$task_mdns_logs/peer.sock" \
  "$task_mdns_with_cli" "$task_mdns_cli_binary" "$task_mdns_with_unicast" "$task_mdns_unicast_binary" \
  "$task_mdns_with_probe" "$task_mdns_probe_binary" "$task_mdns_with_controller" "$task_mdns_controller_binary" \
  "$task_mdns_with_retained" "$task_mdns_retained_binary" \
  "$task_mdns_with_controller_retained" "$task_mdns_controller_retained_binary" \
  "$task_mdns_mapped_cli" "$task_mdns_with_probe_cli" "$task_mdns_probe_cli_binary"
printf 'Isolated mDNS acceptance passed; build evidence: %s\n' "$task_mdns_logs"
