#!/usr/bin/env python3
"""Strictly verify a downloaded Cyanrex release-candidate artifact bundle."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import stat
import sys
import tarfile
from types import ModuleType
from typing import Any, BinaryIO


ARCHIVE_NAME = re.compile(
    r"^cyanrex-lab-(?P<version>[0-9]+\.[0-9]+\.[0-9]+)-"
    r"(?P<timestamp>\d{8}-\d{6})\.tar\.gz$"
)
PACKAGE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
REVISION = re.compile(r"^(?:[a-f0-9]{40}|[a-f0-9]{64})$")
SHA256 = re.compile(r"^[a-f0-9]{64}$")
IMAGE_ID = re.compile(r"^sha256:[a-f0-9]{64}$")
TAG = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+$")
UTC_TIMESTAMP = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
PORTABLE_COMPONENT = re.compile(r"^[A-Za-z0-9._-]+$")
IMAGE_NAMES = {"engine", "frontend", "postgres"}
EVIDENCE_NAME = "cyanrex-live-kernel-acceptance.json"
MAX_ARCHIVE_MEMBERS = 512
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024 * 1024
MAX_CONTROL_BYTES = 4 * 1024 * 1024


class CandidateError(ValueError):
    """Raised when a candidate bundle is malformed or internally inconsistent."""


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise CandidateError(f"JSON contains duplicate key {key!r}")
        value[key] = item
    return value


def reject_json_constant(value: str) -> None:
    raise CandidateError(f"JSON contains unsupported constant {value}")


def parse_json(source: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            source.decode("utf-8"),
            object_pairs_hook=unique_object,
            parse_constant=reject_json_constant,
        )
    except CandidateError:
        raise
    except (UnicodeError, json.JSONDecodeError) as error:
        raise CandidateError(f"cannot parse {label}: {error}") from error
    return record(value, label)


def record(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CandidateError(f"{label} must be a JSON object")
    return value


def exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual == expected:
        return
    details = []
    if missing := sorted(expected - actual):
        details.append(f"missing {', '.join(missing)}")
    if unexpected := sorted(actual - expected):
        details.append(f"unexpected {', '.join(unexpected)}")
    raise CandidateError(f"{label} fields are invalid: {'; '.join(details)}")


def portable_path(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or value.startswith("/"):
        raise CandidateError(f"{label} must be a portable relative path")
    parts = value.split("/")
    if (
        any(
            not part
            or part in {".", ".."}
            or PORTABLE_COMPONENT.fullmatch(part) is None
            for part in parts
        )
        or ":" in parts[0]
    ):
        raise CandidateError(f"{label} must stay within the package")
    return value


def nonempty_line(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or "\n" in value or "\r" in value:
        raise CandidateError(f"{label} must be a non-empty single-line string")
    return value


def parse_timestamp(value: Any, label: str) -> dt.datetime:
    if not isinstance(value, str) or UTC_TIMESTAMP.fullmatch(value) is None:
        raise CandidateError(f"{label} must be an RFC3339 UTC timestamp")
    try:
        parsed = dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except ValueError as error:
        raise CandidateError(f"{label} must be an RFC3339 UTC timestamp") from error
    return parsed


def regular_file(path: Path, label: str, maximum: int | None = None) -> None:
    try:
        info = path.lstat()
    except OSError as error:
        raise CandidateError(f"cannot inspect {label} {path}: {error}") from error
    if not stat.S_ISREG(info.st_mode):
        raise CandidateError(f"{label} must be a regular file: {path.name}")
    if maximum is not None and info.st_size > maximum:
        raise CandidateError(f"{label} exceeds the {maximum}-byte limit")


def sha256_stream(stream: BinaryIO, capture: bool = False) -> tuple[str, bytes | None, int]:
    digest = hashlib.sha256()
    chunks: list[bytes] | None = [] if capture else None
    size = 0
    while chunk := stream.read(1024 * 1024):
        digest.update(chunk)
        size += len(chunk)
        if chunks is not None:
            chunks.append(chunk)
    return digest.hexdigest(), b"".join(chunks) if chunks is not None else None, size


def sha256_file(path: Path) -> str:
    try:
        with path.open("rb") as handle:
            digest, _, _ = sha256_stream(handle)
    except OSError as error:
        raise CandidateError(f"cannot hash {path}: {error}") from error
    return digest


def verify_checksum_file(checksum: Path, target: Path) -> None:
    regular_file(checksum, "checksum file", 4096)
    try:
        source = checksum.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise CandidateError(f"cannot read checksum file {checksum}: {error}") from error
    match = re.fullmatch(r"([a-f0-9]{64}) [ *]([^\r\n]+)\n?", source)
    if match is None or match.group(2) != target.name:
        raise CandidateError(f"checksum file must contain exactly one entry for {target.name}")
    if sha256_file(target) != match.group(1):
        raise CandidateError(f"SHA-256 mismatch for {target.name}")


def discover_bundle(directory: Path) -> tuple[Path, Path]:
    try:
        root = directory.resolve(strict=True)
        entries = list(root.iterdir())
    except OSError as error:
        raise CandidateError(f"cannot inspect candidate bundle {directory}: {error}") from error
    if not root.is_dir():
        raise CandidateError(f"candidate bundle is not a directory: {directory}")
    archives = [entry for entry in entries if ARCHIVE_NAME.fullmatch(entry.name)]
    if len(archives) != 1:
        raise CandidateError("candidate bundle must contain exactly one Cyanrex .tar.gz archive")
    archive = archives[0]
    report = root / EVIDENCE_NAME
    required = {
        archive.name,
        f"{archive.name}.sha256",
        report.name,
        f"{report.name}.sha256",
    }
    actual = {entry.name for entry in entries}
    if actual != required:
        missing = sorted(required - actual)
        unexpected = sorted(actual - required)
        details = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if unexpected:
            details.append(f"unexpected {', '.join(unexpected)}")
        raise CandidateError(f"candidate bundle file set is invalid: {'; '.join(details)}")
    regular_file(archive, "candidate archive")
    regular_file(report, "live kernel evidence", MAX_CONTROL_BYTES)
    verify_checksum_file(root / f"{archive.name}.sha256", archive)
    verify_checksum_file(root / f"{report.name}.sha256", report)
    return archive, report


def archive_path(member: tarfile.TarInfo) -> tuple[str, list[str]]:
    name = member.name[:-1] if member.isdir() and member.name.endswith("/") else member.name
    if not name or "\\" in name or name.startswith("/"):
        raise CandidateError(f"archive member has an unsafe path: {member.name!r}")
    parts = name.split("/")
    if (
        any(
            not part
            or part in {".", ".."}
            or PORTABLE_COMPONENT.fullmatch(part) is None
            for part in parts
        )
        or ":" in parts[0]
    ):
        raise CandidateError(f"archive member escapes its package root: {member.name!r}")
    return name, parts


def inspect_archive(archive: Path) -> tuple[str, dict[str, str], dict[str, bytes]]:
    match = ARCHIVE_NAME.fullmatch(archive.name)
    if match is None:
        raise CandidateError(f"invalid candidate archive name: {archive.name}")
    package_root = archive.name.removesuffix(".tar.gz")
    seen: set[str] = set()
    hashes: dict[str, str] = {}
    controls: dict[str, bytes] = {}
    total_size = 0
    root_directory_seen = False
    try:
        with tarfile.open(archive, mode="r|gz") as package:
            for count, member in enumerate(package, start=1):
                if count > MAX_ARCHIVE_MEMBERS:
                    raise CandidateError(f"archive exceeds the {MAX_ARCHIVE_MEMBERS}-member limit")
                name, parts = archive_path(member)
                if parts[0] != package_root:
                    raise CandidateError(f"archive member is outside {package_root}: {member.name!r}")
                if name in seen:
                    raise CandidateError(f"archive contains duplicate member {name!r}")
                seen.add(name)
                if member.type == tarfile.DIRTYPE:
                    if len(parts) == 1:
                        root_directory_seen = True
                    continue
                if member.type not in {tarfile.REGTYPE, tarfile.AREGTYPE}:
                    raise CandidateError(f"archive contains unsupported member type: {member.name!r}")
                if len(parts) == 1:
                    raise CandidateError("archive package root must be a directory")
                total_size += member.size
                if member.size < 0 or total_size > MAX_ARCHIVE_BYTES:
                    raise CandidateError(f"archive exceeds the {MAX_ARCHIVE_BYTES}-byte content limit")
                relative = "/".join(parts[1:])
                if relative in hashes:
                    raise CandidateError(f"archive contains duplicate package path {relative!r}")
                capture = relative in {"checksums.sha256", "release-metadata.json"}
                if capture and member.size > MAX_CONTROL_BYTES:
                    raise CandidateError(f"archive control file is too large: {relative}")
                stream = package.extractfile(member)
                if stream is None:
                    raise CandidateError(f"cannot read archive member {member.name!r}")
                with stream:
                    digest, source, actual_size = sha256_stream(stream, capture)
                if actual_size != member.size:
                    raise CandidateError(f"archive member size is inconsistent: {member.name!r}")
                hashes[relative] = digest
                if source is not None:
                    controls[relative] = source
    except CandidateError:
        raise
    except (OSError, EOFError, tarfile.TarError) as error:
        raise CandidateError(f"cannot inspect candidate archive {archive.name}: {error}") from error
    if not root_directory_seen:
        raise CandidateError("archive does not contain its declared package root directory")
    for required in ("checksums.sha256", "release-metadata.json"):
        if required not in controls:
            raise CandidateError(f"archive is missing {required}")
    return package_root, hashes, controls


def parse_manifest(source: bytes) -> dict[str, str]:
    try:
        text = source.decode("utf-8")
    except UnicodeError as error:
        raise CandidateError(f"checksum manifest is not UTF-8: {error}") from error
    entries: dict[str, str] = {}
    for line in text.splitlines():
        if not line:
            continue
        match = re.fullmatch(r"([a-f0-9]{64}) [ *]([^\r\n]+)", line)
        if match is None:
            raise CandidateError(f"invalid checksum manifest line: {line!r}")
        target = portable_path(match.group(2), "checksum target")
        if target in entries:
            raise CandidateError(f"duplicate checksum entry for {target}")
        entries[target] = match.group(1)
    if not entries:
        raise CandidateError("checksum manifest is empty")
    return entries


def verify_manifest(hashes: dict[str, str], source: bytes) -> None:
    manifest = parse_manifest(source)
    expected = set(hashes) - {"checksums.sha256"}
    if set(manifest) != expected:
        missing = sorted(expected - set(manifest))
        unexpected = sorted(set(manifest) - expected)
        details = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if unexpected:
            details.append(f"unexpected {', '.join(unexpected)}")
        raise CandidateError(f"checksum manifest file set is invalid: {'; '.join(details)}")
    for name, expected_hash in manifest.items():
        if hashes[name] != expected_hash:
            raise CandidateError(f"checksum manifest does not match {name}")


def validate_metadata(
    source: bytes,
    package_root: str,
    archive_name: str,
    hashes: dict[str, str],
) -> dict[str, Any]:
    metadata = parse_json(source, "release metadata")
    exact_keys(metadata, {"schemaVersion", "package", "source", "compose", "images", "integrity"}, "release metadata")
    if metadata.get("schemaVersion") != 1:
        raise CandidateError("release metadata schemaVersion must be 1")

    package = record(metadata.get("package"), "release metadata package")
    exact_keys(package, {"name", "version", "createdAt"}, "release metadata package")
    archive_match = ARCHIVE_NAME.fullmatch(archive_name)
    if archive_match is None or package.get("name") != package_root:
        raise CandidateError("release metadata package name does not match the archive")
    if not isinstance(package.get("name"), str) or PACKAGE_NAME.fullmatch(package["name"]) is None:
        raise CandidateError("release metadata package name is invalid")
    version = package.get("version")
    if not isinstance(version, str) or SEMVER.fullmatch(version) is None:
        raise CandidateError("release metadata package version is invalid")
    if version != archive_match.group("version"):
        raise CandidateError("release metadata package version does not match the archive")
    created_at = parse_timestamp(package.get("createdAt"), "release metadata package createdAt")
    try:
        archive_time = dt.datetime.strptime(
            archive_match.group("timestamp"), "%Y%m%d-%H%M%S"
        ).replace(tzinfo=dt.timezone.utc)
    except ValueError as error:
        raise CandidateError("candidate archive timestamp is invalid") from error
    if created_at != archive_time:
        raise CandidateError("release metadata creation time does not match the archive name")

    source_info = record(metadata.get("source"), "release metadata source")
    exact_keys(source_info, {"revision", "state", "tag"}, "release metadata source")
    state = source_info.get("state")
    if state not in {"clean", "dirty", "unavailable"}:
        raise CandidateError("release metadata source state is invalid")
    if state == "unavailable":
        if source_info.get("revision") is not None or source_info.get("tag") is not None:
            raise CandidateError("unavailable release source must not claim a revision or Tag")
    else:
        revision = source_info.get("revision")
        if not isinstance(revision, str) or REVISION.fullmatch(revision) is None:
            raise CandidateError("release metadata source revision is invalid")
        tag = source_info.get("tag")
        if tag is not None and tag != f"v{version}":
            raise CandidateError("release metadata source Tag does not match its version")

    compose = record(metadata.get("compose"), "release metadata compose")
    exact_keys(compose, {"file", "source"}, "release metadata compose")
    if compose.get("file") != "docker-compose.yml":
        raise CandidateError("release metadata compose file is invalid")
    portable_path(compose.get("source"), "release metadata compose source")

    images = record(metadata.get("images"), "release metadata images")
    exact_keys(images, {"mode", "references", "contentIds", "archive"}, "release metadata images")
    if images.get("mode") not in {"built", "prebuilt"}:
        raise CandidateError("release metadata image mode is invalid")
    references = record(images.get("references"), "release metadata image references")
    identities = record(images.get("contentIds"), "release metadata image content IDs")
    exact_keys(references, IMAGE_NAMES, "release metadata image references")
    exact_keys(identities, IMAGE_NAMES, "release metadata image content IDs")
    for name in sorted(IMAGE_NAMES):
        nonempty_line(references.get(name), f"release metadata {name} image reference")
        identity = identities.get(name)
        if not isinstance(identity, str) or IMAGE_ID.fullmatch(identity) is None:
            raise CandidateError(f"release metadata {name} image content ID is invalid")
    image_archive = record(images.get("archive"), "release metadata image archive")
    exact_keys(image_archive, {"file", "sha256"}, "release metadata image archive")
    if image_archive.get("file") != "cyanrex-images.tar":
        raise CandidateError("release metadata image archive file is invalid")
    archive_hash = image_archive.get("sha256")
    if not isinstance(archive_hash, str) or SHA256.fullmatch(archive_hash) is None:
        raise CandidateError("release metadata image archive SHA-256 is invalid")

    integrity = record(metadata.get("integrity"), "release metadata integrity")
    exact_keys(integrity, {"algorithm", "manifest"}, "release metadata integrity")
    if integrity != {"algorithm": "sha256", "manifest": "checksums.sha256"}:
        raise CandidateError("release metadata integrity declaration is invalid")
    for required in (compose["file"], image_archive["file"]):
        if required not in hashes:
            raise CandidateError(f"release metadata references missing package file {required}")
    if hashes[image_archive["file"]] != archive_hash:
        raise CandidateError("image archive SHA-256 does not match release metadata")
    return metadata


def enforce_expectations(metadata: dict[str, Any], options: argparse.Namespace) -> None:
    package = metadata["package"]
    source = metadata["source"]
    images = metadata["images"]
    if options.expect_version:
        if SEMVER.fullmatch(options.expect_version) is None:
            raise CandidateError("expected package version must use x.y.z")
        if package["version"] != options.expect_version:
            raise CandidateError("candidate package version does not match expectation")
    if options.expect_revision:
        expected_revision = options.expect_revision.lower()
        if REVISION.fullmatch(expected_revision) is None:
            raise CandidateError("expected source revision is invalid")
        if source["revision"] != expected_revision:
            raise CandidateError("candidate source revision does not match expectation")
    if options.expect_tag:
        if TAG.fullmatch(options.expect_tag) is None:
            raise CandidateError("expected source Tag must use vx.y.z")
        if source["tag"] != options.expect_tag:
            raise CandidateError("candidate source Tag does not match expectation")
    if options.expect_source_state and source["state"] != options.expect_source_state:
        raise CandidateError("candidate source state does not match expectation")
    if options.expect_image_mode and images["mode"] != options.expect_image_mode:
        raise CandidateError("candidate image mode does not match expectation")


def load_evidence_tool() -> ModuleType:
    path = Path(__file__).resolve().with_name("live-kernel-evidence.py")
    spec = importlib.util.spec_from_file_location("cyanrex_live_kernel_evidence", path)
    if spec is None or spec.loader is None:
        raise CandidateError(f"cannot load live-kernel evidence verifier from {path}")
    module = importlib.util.module_from_spec(spec)
    previous_bytecode_setting = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        spec.loader.exec_module(module)
    except (ImportError, OSError) as error:
        raise CandidateError(f"cannot load live-kernel evidence verifier: {error}") from error
    finally:
        sys.dont_write_bytecode = previous_bytecode_setting
    return module


def verify_evidence(report_path: Path, metadata: dict[str, Any], metadata_source: bytes) -> dict[str, Any]:
    try:
        report_source = report_path.read_bytes()
    except OSError as error:
        raise CandidateError(f"cannot read live kernel evidence: {error}") from error
    report = parse_json(report_source, "live kernel evidence")
    evidence = load_evidence_tool()
    try:
        evidence.validate_report(report)
    except (ValueError, TypeError) as error:
        raise CandidateError(f"live kernel evidence is invalid: {error}") from error
    images = metadata["images"]
    expected_candidate = {
        "releaseMetadataSha256": hashlib.sha256(metadata_source).hexdigest(),
        "package": metadata["package"],
        "source": metadata["source"],
        "images": {
            "mode": images["mode"],
            "references": images["references"],
            "contentIds": images["contentIds"],
            "archiveSha256": images["archive"]["sha256"],
        },
    }
    if report.get("candidate") != expected_candidate:
        raise CandidateError("live kernel evidence does not match the archived release metadata")
    return report


def verify_candidate(options: argparse.Namespace) -> tuple[dict[str, Any], dict[str, Any], Path]:
    archive, report_path = discover_bundle(Path(options.bundle))
    package_root, hashes, controls = inspect_archive(archive)
    verify_manifest(hashes, controls["checksums.sha256"])
    metadata_source = controls["release-metadata.json"]
    metadata = validate_metadata(metadata_source, package_root, archive.name, hashes)
    enforce_expectations(metadata, options)
    report = verify_evidence(report_path, metadata, metadata_source)
    return metadata, report, archive


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("verify", help="verify one complete candidate bundle")
    verify.add_argument("bundle", help="directory containing archive, checksums, and kernel evidence")
    verify.add_argument("--expect-version")
    verify.add_argument("--expect-revision")
    verify.add_argument("--expect-tag")
    verify.add_argument("--expect-source-state", choices=("clean", "dirty", "unavailable"))
    verify.add_argument("--expect-image-mode", choices=("built", "prebuilt"))
    return parser


def main(argv: list[str] | None = None) -> int:
    options = build_parser().parse_args(argv)
    try:
        metadata, report, archive = verify_candidate(options)
    except (CandidateError, OSError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    revision = metadata["source"]["revision"] or "unavailable"
    print(
        "[cyanrex] Release candidate verified: "
        f"{archive.name} version={metadata['package']['version']} "
        f"revision={revision} evidence={report['generatedAt']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
