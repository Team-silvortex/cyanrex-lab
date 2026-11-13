#!/usr/bin/env python3
"""Owned, capability-free peer RPC fixture; never run against the host network."""

import json
import os
from pathlib import Path
import socket
import stat
import struct
import sys
import threading
import time

MAX_RPC_BYTES = 32768
MAX_DATAGRAM_BYTES = 5000
MAX_WORKERS = 4
GROUP = "224.0.0.251"
PEER_ADDRESSES = {"10.23.0.2": "crx1", "10.24.0.2": "crx3"}


def require(condition):
    if not condition:
        raise ValueError("fixture rejected")


def zero_capabilities():
    values = {}
    for line in Path("/proc/self/status").read_text().splitlines():
        key, _, value = line.partition(":")
        values[key] = value.strip()
    require(all(int(values[key], 16) == 0 for key in (
        "CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"
    )))
    require(values["NoNewPrivs"] == "1")


def isolated_peer():
    zero_capabilities()
    current = os.readlink("/proc/self/ns/net")
    require(current != os.environ["CYANREX_MDNS_TEST_PARENT_NETNS"])
    require(current != os.environ["CYANREX_MDNS_TEST_PEER_PARENT_NETNS"])


def ready_network():
    isolated_peer()
    lines = Path("/proc/self/net/dev").read_text().splitlines()[2:]
    require(sorted(line.split(":", 1)[0].strip() for line in lines)
            == ["crx1", "crx3", "lo"])
    routes = Path("/proc/self/net/route").read_text().splitlines()[1:]
    require(not any(line.split()[1] == "00000000" for line in routes))


def byte_array(value):
    require(type(value) is list and len(value) <= MAX_DATAGRAM_BYTES)
    require(all(type(byte) is int and 0 <= byte <= 255 for byte in value))
    return bytes(value)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def reject_constant(_value):
    raise ValueError("fixture rejected")


def multicast_interface(sock, ip):
    index = socket.if_nametoindex(PEER_ADDRESSES[ip])
    require(index > 0)
    membership = struct.pack("=4s4si", socket.inet_aton(GROUP),
                             socket.inet_aton(ip), index)
    sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_IF, membership)
    return membership


def send_datagram(ip, port, ttl, destination, data):
    ready_network()
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.settimeout(0.2)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        multicast_interface(sock, ip)
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, ttl)
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_TTL, ttl)
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_LOOP, 0)
        sock.bind((ip, port))
        require(sock.sendto(data, (destination, 5353)) == len(data))


class QueryWorker:
    def __init__(self, replies):
        ready_network()
        self.result = None
        self.receiver = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            self.receiver.settimeout(2.0)
            self.receiver.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            self.receiver.setsockopt(socket.IPPROTO_IP,
                                     getattr(socket, "IP_MULTICAST_ALL", 49), 0)
            membership = multicast_interface(self.receiver, "10.23.0.2")
            self.receiver.bind((GROUP, 5353))
            self.receiver.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP,
                                     membership)
            self.thread = threading.Thread(target=self.capture, args=(replies,))
            self.thread.start()
        except Exception:
            self.receiver.close()
            raise

    def capture(self, replies):
        try:
            with self.receiver:
                data, source = self.receiver.recvfrom(MAX_DATAGRAM_BYTES + 1)
                require(len(data) <= MAX_DATAGRAM_BYTES)
            query = {"source": source[0], "data": list(data)}
            for index, reply in enumerate(replies):
                if index:
                    time.sleep(0.03)
                send_datagram("10.23.0.2", 5353, 255, GROUP, reply)
            self.result = query
        except Exception:
            self.result = None


class ProbeWorker:
    """Bounded observation of one round, with at most one fixed-target defense."""
    def __init__(self, reply_after, destination, reply):
        ready_network()
        self.result = None
        self.receiver = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            self.receiver.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            self.receiver.setsockopt(socket.IPPROTO_IP,
                                     getattr(socket, "IP_MULTICAST_ALL", 49), 0)
            self.receiver.setsockopt(socket.IPPROTO_IP,
                                     getattr(socket, "IP_RECVTTL", 12), 1)
            membership = multicast_interface(self.receiver, "10.23.0.2")
            self.receiver.bind((GROUP, 5353))
            self.receiver.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP,
                                     membership)
            self.thread = threading.Thread(target=self.capture,
                                           args=(reply_after, destination, reply))
            self.thread.start()
        except Exception:
            self.receiver.close()
            raise

    def capture(self, reply_after, destination, reply):
        captured = []
        deadline = time.monotonic() + 2.2
        try:
            with self.receiver:
                while len(captured) < 4:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        break
                    self.receiver.settimeout(remaining)
                    try:
                        data, controls, flags, source = self.receiver.recvmsg(
                            MAX_DATAGRAM_BYTES + 1, socket.CMSG_SPACE(4))
                    except socket.timeout:
                        break
                    received_ns = time.monotonic_ns()
                    require(not flags and len(data) <= MAX_DATAGRAM_BYTES)
                    require(source == ("10.23.0.1", 5353))
                    require(len(controls) == 1)
                    level, kind, value = controls[0]
                    require(level == socket.IPPROTO_IP and kind == socket.IP_TTL
                            and len(value) == 4)
                    ttl = struct.unpack("=i", value)[0]
                    captured.append({"source": source[0], "port": source[1],
                                     "ttl": ttl, "ns": received_ns,
                                     "data": list(data)})
                    if len(captured) == reply_after:
                        send_datagram("10.23.0.2", 5353, 255, destination, reply)
            self.result = captured
        except Exception:
            self.result = None


class PeerServer:
    def __init__(self):
        self.workers = []
        self.probe_worker = None

    def dispatch(self, request):
        require(type(request) is dict and type(request.get("op")) is str)
        operation = request["op"]
        if operation == "send":
            require(set(request) == {"op", "ip", "port", "ttl", "destination", "data"})
            require(type(request["ip"]) is str and request["ip"] in PEER_ADDRESSES)
            require(type(request["port"]) is int and request["port"] in (5353, 5354))
            require(type(request["ttl"]) is int and request["ttl"] in (254, 255))
            require(type(request["destination"]) is str
                    and request["destination"] in (GROUP, "10.23.0.1", "10.24.0.1"))
            data = byte_array(request["data"])
            send_datagram(request["ip"], request["port"], request["ttl"],
                          request["destination"], data)
            return {"ok": True}
        if operation == "arm":
            require(set(request) == {"op", "replies"})
            require(type(request["replies"]) is list and len(request["replies"]) <= 2)
            replies = [byte_array(reply) for reply in request["replies"]]
            require(len(self.workers) < MAX_WORKERS)
            self.workers.append(QueryWorker(replies))
            return {"ok": True}
        if operation == "wait":
            require(set(request) == {"op"})
            workers = self.workers
            for worker in workers:
                worker.thread.join()
            self.workers = []
            if any(worker.result is None for worker in workers):
                return {"ok": False}
            return {"ok": True, "queries": [worker.result for worker in workers]}
        if operation == "arm-probes":
            require(set(request) == {"op", "reply_after", "destination", "data"})
            require(self.probe_worker is None and not self.workers)
            after, destination = request["reply_after"], request["destination"]
            require(type(after) is int and type(destination) is str)
            reply = byte_array(request["data"])
            require((after == 0 and destination == "none" and not reply)
                    or (after in (1, 2, 3)
                        and destination in (GROUP, "10.23.0.1")
                        and 1 <= len(reply) <= 4096))
            self.probe_worker = ProbeWorker(after, destination, reply)
            return {"ok": True}
        if operation == "wait-probes":
            require(set(request) == {"op"} and self.probe_worker is not None)
            worker, self.probe_worker = self.probe_worker, None
            worker.thread.join(timeout=2.4)
            require(not worker.thread.is_alive() and worker.result is not None)
            return {"ok": True, "queries": worker.result}
        raise ValueError("fixture rejected")


def read_request(connection):
    request = bytearray()
    while b"\n" not in request:
        block = connection.recv(min(4096, MAX_RPC_BYTES + 1 - len(request)))
        require(bool(block))
        request.extend(block)
        require(len(request) <= MAX_RPC_BYTES)
    require(request.endswith(b"\n") and request.count(b"\n") == 1)
    return json.loads(request.decode("utf-8"), object_pairs_hook=unique_object,
                      parse_constant=reject_constant)


def serve(path):
    isolated_peer()
    require(path.is_absolute() and path.name == "peer.sock")
    parent = path.parent.stat()
    require(stat.S_ISDIR(parent.st_mode) and stat.S_IMODE(parent.st_mode) == 0o700)
    require(parent.st_uid == os.geteuid() and not path.exists() and not path.is_symlink())
    os.umask(0o077)
    server = PeerServer()
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
        listener.bind(str(path))
        try:
            listener.listen(4)
            while True:
                connection, _address = listener.accept()
                with connection:
                    connection.settimeout(2.5)
                    try:
                        _pid, uid, _gid = struct.unpack(
                            "3i", connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
                        )
                        require(uid == os.geteuid())
                        response = server.dispatch(read_request(connection))
                        encoded = json.dumps(response, separators=(",", ":")).encode() + b"\n"
                        require(len(encoded) <= MAX_RPC_BYTES)
                    except Exception:
                        encoded = b'{"ok":false}\n'
                    try:
                        connection.sendall(encoded)
                    except OSError:
                        pass
        finally:
            path.unlink(missing_ok=True)


if __name__ == "__main__":
    try:
        require(len(sys.argv) == 2)
        serve(Path(sys.argv[1]))
    except Exception:
        sys.exit(1)
