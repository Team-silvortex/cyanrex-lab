#!/usr/bin/env bash
set -euo pipefail

if [[ ${1:-} == --help && $# == 1 ]]; then
  printf '%s\n' 'Usage: scripts/test-federation-mdns-netns.sh' 'Build locked/offline; test only inside a fresh anonymous user/network namespace. No host LAN fallback.'
  exit 0
fi
[[ $# == 0 ]] || { printf '%s\n' 'Unexpected arguments.' >&2; exit 2; }
task_mdns_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
for task_mdns_tool in cargo node unshare ip nsenter setpriv readlink python3; do
  command -v "$task_mdns_tool" >/dev/null || { printf 'Missing tool: %s\n' "$task_mdns_tool" >&2; exit 1; }
done
task_mdns_logs=$(mktemp -d /tmp/cyanrex-mdns-netns-XXXXXX)
chmod 0700 "$task_mdns_logs"
task_mdns_parent=$(readlink /proc/self/ns/net)
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path "$task_mdns_root/engine/Cargo.toml" --locked --offline \
    --test federation_mdns_network_tdd --no-run --message-format=json > "$task_mdns_logs/build.jsonl"
task_mdns_binary=$(node --input-type=module -e '
import {readFileSync} from "node:fs";
const values=readFileSync(process.argv[1],"utf8").trim().split("\n").map(line=>JSON.parse(line));
const artifacts=values.filter(item=>item.reason==="compiler-artifact"&&item.target?.name==="federation_mdns_network_tdd"&&item.executable);
if(artifacts.length!==1)throw new Error("Expected one exact test executable");
process.stdout.write(artifacts[0].executable);' "$task_mdns_logs/build.jsonl")
[[ "$task_mdns_binary" == "$task_mdns_root/engine/target/"* && -x "$task_mdns_binary" ]] || exit 1

unshare --user --map-root-user --net --fork bash -euo pipefail -c '
  task_mdns_binary=$1
  task_mdns_parent=$2
  task_mdns_helper=$3
  task_mdns_socket=$4
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
  CYANREX_MDNS_TEST_MUTATE=1 setpriv --bounding-set=-all,+net_admin --inh-caps=-all \
    --ambient-caps=-all --no-new-privs "$task_mdns_binary" --ignored \
    --exact selected_address_removal_invalidates_the_socket_without_fallback --test-threads=1
  # All interfaces and routes above belong only to this anonymous namespace and die with it.
' cyanrex-mdns-netns "$task_mdns_binary" "$task_mdns_parent" \
  "$task_mdns_root/scripts/tests/federationMdnsPeer.py" "$task_mdns_logs/peer.sock"
printf 'Isolated mDNS acceptance passed; build evidence: %s\n' "$task_mdns_logs"
