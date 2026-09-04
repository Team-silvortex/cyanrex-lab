#!/usr/bin/env python3
"""Create and verify Cyanrex live-kernel acceptance evidence."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import uuid
from typing import Any


SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
REVISION = re.compile(r"^(?:[a-f0-9]{40}|[a-f0-9]{64})$")
SHA256 = re.compile(r"^[a-f0-9]{64}$")
IMAGE_ID = re.compile(r"^sha256:[a-f0-9]{64}$")
PROGRAM_NAME = re.compile(r"^release-kernel-smoke-[a-f0-9]{16}$")
UTC_TIMESTAMP = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")
PACKAGE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
IMAGE_NAMES = ("engine", "frontend", "postgres")


class EvidenceError(ValueError):
    """Raised when evidence is malformed or does not match its candidate."""


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError(f"JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _load_json(path: str | Path, label: str) -> tuple[dict[str, Any], bytes]:
    target = Path(path)
    try:
        source = target.read_bytes()
        value = json.loads(source.decode("utf-8"), object_pairs_hook=_unique_object)
    except EvidenceError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read {label} {target}: {error}") from error
    return _record(value, label), source


def _record(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be a JSON object")
    return value


def _exact_keys(value: dict[str, Any], keys: set[str], label: str) -> None:
    actual = set(value)
    if actual != keys:
        missing = sorted(keys - actual)
        unexpected = sorted(actual - keys)
        details = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if unexpected:
            details.append(f"unexpected {', '.join(unexpected)}")
        raise EvidenceError(f"{label} fields are invalid: {'; '.join(details)}")


def _fullmatch(pattern: re.Pattern[str], value: Any) -> bool:
    return isinstance(value, str) and pattern.fullmatch(value) is not None


def _timestamp(value: Any, label: str) -> None:
    if not _fullmatch(UTC_TIMESTAMP, value):
        raise EvidenceError(f"{label} must be an RFC3339 UTC timestamp")
    try:
        parsed = dt.datetime.fromisoformat(f"{value[:-1]}+00:00")
    except ValueError as error:
        raise EvidenceError(f"{label} must be an RFC3339 UTC timestamp") from error
    if parsed.tzinfo != dt.timezone.utc:
        raise EvidenceError(f"{label} must use UTC")


def _nonempty(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or "\n" in value or "\r" in value:
        raise EvidenceError(f"{label} must be a non-empty single-line string")
    return value


def _positive_int(value: Any, label: str) -> None:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise EvidenceError(f"{label} must be a positive integer")


def _validate_environment(environment: Any) -> dict[str, Any]:
    report = _record(environment, "environment")
    if not isinstance(report.get("overall_ok"), bool):
        raise EvidenceError("environment overall_ok must be boolean")
    _timestamp(report.get("generated_at"), "environment generated_at")
    runtime_mode = report.get("runtime_mode")
    if not isinstance(runtime_mode, str) or runtime_mode not in {"native-linux", "wsl2", "docker"}:
        raise EvidenceError("environment runtime_mode is invalid")
    checks = report.get("checks")
    if not isinstance(checks, list):
        raise EvidenceError("environment checks must be an array")
    kernel = next(
        (
            item
            for item in checks
            if isinstance(item, dict) and item.get("name") == "kernel"
        ),
        None,
    )
    if kernel is None or not isinstance(kernel.get("ok"), bool):
        raise EvidenceError("environment must include a boolean kernel check")
    _nonempty(kernel.get("detail"), "environment kernel detail")
    return report


def _validate_candidate(candidate: Any) -> dict[str, Any] | None:
    if candidate is None:
        return None
    value = _record(candidate, "candidate")
    _exact_keys(
        value,
        {"releaseMetadataSha256", "package", "source", "images"},
        "candidate",
    )
    if not _fullmatch(SHA256, value.get("releaseMetadataSha256")):
        raise EvidenceError("candidate release metadata SHA-256 is invalid")

    package = _record(value.get("package"), "candidate package")
    _exact_keys(package, {"name", "version", "createdAt"}, "candidate package")
    if not _fullmatch(PACKAGE_NAME, package.get("name")):
        raise EvidenceError("candidate package name is invalid")
    version = package.get("version")
    if not _fullmatch(SEMVER, version):
        raise EvidenceError("candidate package version is invalid")
    _timestamp(package.get("createdAt"), "candidate package createdAt")

    source = _record(value.get("source"), "candidate source")
    _exact_keys(source, {"revision", "state", "tag"}, "candidate source")
    state = source.get("state")
    if not isinstance(state, str) or state not in {"clean", "dirty", "unavailable"}:
        raise EvidenceError("candidate source state is invalid")
    if state == "unavailable":
        if source.get("revision") is not None or source.get("tag") is not None:
            raise EvidenceError("unavailable candidate source must not claim a revision or Tag")
    else:
        if not _fullmatch(REVISION, source.get("revision")):
            raise EvidenceError("candidate source revision is invalid")
        tag = source.get("tag")
        if tag is not None and tag != f"v{version}":
            raise EvidenceError("candidate source Tag does not match its package version")

    images = _record(value.get("images"), "candidate images")
    _exact_keys(
        images,
        {"mode", "references", "contentIds", "archiveSha256"},
        "candidate images",
    )
    image_mode = images.get("mode")
    if not isinstance(image_mode, str) or image_mode not in {"built", "prebuilt"}:
        raise EvidenceError("candidate image mode is invalid")
    references = _record(images.get("references"), "candidate image references")
    content_ids = _record(images.get("contentIds"), "candidate image content IDs")
    _exact_keys(references, set(IMAGE_NAMES), "candidate image references")
    _exact_keys(content_ids, set(IMAGE_NAMES), "candidate image content IDs")
    for name in IMAGE_NAMES:
        _nonempty(references.get(name), f"candidate {name} image reference")
        if not _fullmatch(IMAGE_ID, content_ids.get(name)):
            raise EvidenceError(f"candidate {name} image content ID is invalid")
    if not _fullmatch(SHA256, images.get("archiveSha256")):
        raise EvidenceError("candidate image archive SHA-256 is invalid")
    return value


def candidate_from_metadata(path: str | Path) -> dict[str, Any]:
    metadata, source = _load_json(path, "release metadata")
    if metadata.get("schemaVersion") != 1:
        raise EvidenceError("release metadata schemaVersion must be 1")
    images = _record(metadata.get("images"), "release metadata images")
    archive = _record(images.get("archive"), "release metadata image archive")
    candidate = {
        "releaseMetadataSha256": hashlib.sha256(source).hexdigest(),
        "package": metadata.get("package"),
        "source": metadata.get("source"),
        "images": {
            "mode": images.get("mode"),
            "references": images.get("references"),
            "contentIds": images.get("contentIds"),
            "archiveSha256": archive.get("sha256"),
        },
    }
    return _validate_candidate(candidate) or {}


def validate_report(report: Any) -> dict[str, Any]:
    value = _record(report, "live kernel evidence")
    _exact_keys(
        value,
        {"schemaVersion", "result", "generatedAt", "candidate", "environment", "exercise", "cleanup"},
        "live kernel evidence",
    )
    schema_version = value.get("schemaVersion")
    if schema_version not in (1, 2) or value.get("result") != "passed":
        raise EvidenceError("live kernel evidence must report a passed supported schema result")
    _timestamp(value.get("generatedAt"), "evidence generatedAt")
    _validate_candidate(value.get("candidate"))
    _validate_environment(value.get("environment"))

    exercise = _record(value.get("exercise"), "exercise")
    _exact_keys(
        exercise,
        {"templateId", "programName", "runtimeBackend", "hook", "pinPath", "event"},
        "exercise",
    )
    if exercise.get("templateId") != "ringbuf-hi-freq-sampler":
        raise EvidenceError("exercise template is not the release ring-buffer sampler")
    if not _fullmatch(PROGRAM_NAME, exercise.get("programName")):
        raise EvidenceError("exercise program name is not uniquely bound")
    if exercise.get("runtimeBackend") != "aya":
        raise EvidenceError("exercise runtime backend must be aya")
    if exercise.get("hook") != "tracepoint/sched/sched_switch":
        raise EvidenceError("exercise hook must be sched_switch")
    pin_path = _nonempty(exercise.get("pinPath"), "exercise pin path")
    if not pin_path.startswith("/sys/fs/bpf/"):
        raise EvidenceError("exercise pin path must be inside bpffs")

    event = _record(exercise.get("event"), "exercise event")
    event_fields = {"timestamp", "type", "bytes"}
    if schema_version == 2:
        event_fields.add("programName")
    _exact_keys(event, event_fields, "exercise event")
    _timestamp(event.get("timestamp"), "exercise event timestamp")
    if event.get("type") != "ebpf.kernel_ringbuf":
        raise EvidenceError("exercise event type is invalid")
    _positive_int(event.get("bytes"), "exercise event bytes")
    if schema_version == 2 and event.get("programName") != exercise.get("programName"):
        raise EvidenceError("exercise event is not bound to its unique program name")

    cleanup = _record(value.get("cleanup"), "cleanup")
    _exact_keys(cleanup, {"exactPinDetached", "remainingAttachments"}, "cleanup")
    if cleanup.get("exactPinDetached") is not True or cleanup.get("remainingAttachments") != 0:
        raise EvidenceError("live kernel evidence does not prove exact cleanup")
    return value


def create_report(options: argparse.Namespace) -> dict[str, Any]:
    environment, _ = _load_json(options.environment, "environment report")
    event, _ = _load_json(options.event, "matched kernel event")
    _validate_environment(environment)
    payload = _record(event.get("payload"), "matched kernel event payload")
    if event.get("event_type") != "ebpf.kernel_ringbuf":
        raise EvidenceError("matched kernel event type is invalid")
    if payload.get("program_name") != options.program_name:
        raise EvidenceError("matched kernel event belongs to another program")
    _positive_int(payload.get("bytes"), "matched kernel event bytes")
    _timestamp(event.get("timestamp"), "matched kernel event timestamp")

    candidate = candidate_from_metadata(options.release_metadata) if options.release_metadata else None
    report = {
        "schemaVersion": 2,
        "result": "passed",
        "generatedAt": dt.datetime.now(dt.timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        "candidate": candidate,
        "environment": environment,
        "exercise": {
            "templateId": "ringbuf-hi-freq-sampler",
            "programName": options.program_name,
            "runtimeBackend": "aya",
            "hook": "tracepoint/sched/sched_switch",
            "pinPath": options.pin_path,
            "event": {
                "timestamp": event.get("timestamp"),
                "type": event.get("event_type"),
                "bytes": payload.get("bytes"),
                "programName": payload.get("program_name"),
            },
        },
        "cleanup": {"exactPinDetached": True, "remainingAttachments": 0},
    }
    return validate_report(report)


def write_report(report: dict[str, Any], output: str | Path) -> Path:
    destination = Path(output).resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.parent / f".{destination.name}.tmp-{os.getpid()}-{uuid.uuid4().hex}"
    try:
        with temporary.open("x", encoding="utf-8") as handle:
            json.dump(report, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        temporary.chmod(0o644)
        os.link(temporary, destination)
    except FileExistsError as error:
        raise EvidenceError(f"live kernel evidence output already exists: {destination}") from error
    finally:
        temporary.unlink(missing_ok=True)
    return destination


def verify_report(options: argparse.Namespace) -> dict[str, Any]:
    report, _ = _load_json(options.report, "live kernel evidence")
    validate_report(report)
    candidate = report.get("candidate")
    if options.expect_version and not _fullmatch(SEMVER, options.expect_version):
        raise EvidenceError("expected package version must use x.y.z")
    if options.expect_revision and not _fullmatch(REVISION, options.expect_revision):
        raise EvidenceError("expected source revision is invalid")
    if options.expect_tag and not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", options.expect_tag):
        raise EvidenceError("expected source Tag must use vx.y.z")
    if options.release_metadata:
        expected_candidate = candidate_from_metadata(options.release_metadata)
        if candidate != expected_candidate:
            raise EvidenceError("live kernel evidence does not match the supplied release metadata")
    if any(
        (
            options.expect_version,
            options.expect_revision,
            options.expect_tag,
            options.expect_source_state,
            options.expect_image_mode,
        )
    ) and candidate is None:
        raise EvidenceError("candidate expectations require release-bound evidence")
    if candidate is not None:
        package = candidate["package"]
        source = candidate["source"]
        if options.expect_version and package["version"] != options.expect_version:
            raise EvidenceError("live kernel evidence package version does not match expectation")
        if options.expect_revision and source["revision"] != options.expect_revision:
            raise EvidenceError("live kernel evidence source revision does not match expectation")
        if options.expect_tag and source["tag"] != options.expect_tag:
            raise EvidenceError("live kernel evidence source Tag does not match expectation")
        if options.expect_source_state and source["state"] != options.expect_source_state:
            raise EvidenceError("live kernel evidence source state does not match expectation")
        if options.expect_image_mode and candidate["images"]["mode"] != options.expect_image_mode:
            raise EvidenceError("live kernel evidence image mode does not match expectation")
    return report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    create = commands.add_parser("create", help="create evidence after exact cleanup")
    create.add_argument("--output", required=True)
    create.add_argument("--environment", required=True)
    create.add_argument("--event", required=True)
    create.add_argument("--program-name", required=True)
    create.add_argument("--pin-path", required=True)
    create.add_argument("--release-metadata")

    verify = commands.add_parser("verify", help="strictly verify existing evidence")
    verify.add_argument("report")
    verify.add_argument("--release-metadata")
    verify.add_argument("--expect-version")
    verify.add_argument("--expect-revision")
    verify.add_argument("--expect-tag")
    verify.add_argument("--expect-source-state", choices=("clean", "dirty", "unavailable"))
    verify.add_argument("--expect-image-mode", choices=("built", "prebuilt"))
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    options = parser.parse_args(argv)
    try:
        if options.command == "create":
            report = create_report(options)
            output = write_report(report, options.output)
            print(f"[cyanrex] Live kernel acceptance evidence: {output}")
        else:
            verify_report(options)
            print(f"[cyanrex] Live kernel acceptance evidence verified: {Path(options.report)}")
    except (EvidenceError, OSError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
