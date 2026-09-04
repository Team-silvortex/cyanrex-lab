#!/usr/bin/env python3
"""Verify and safely extract a Cyanrex offline release package."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import stat
import sys
import tarfile
import tempfile
from types import ModuleType
from typing import Any, Callable


class PackageError(ValueError):
    """Raised when a release package cannot be verified or safely extracted."""


def load_candidate_tool() -> ModuleType:
    path = Path(__file__).resolve().with_name("release-candidate.py")
    spec = importlib.util.spec_from_file_location("cyanrex_release_candidate", path)
    if spec is None or spec.loader is None:
        raise PackageError(f"cannot load release candidate verifier from {path}")
    module = importlib.util.module_from_spec(spec)
    previous_bytecode_setting = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        spec.loader.exec_module(module)
    except (ImportError, OSError) as error:
        raise PackageError(f"cannot load release candidate verifier: {error}") from error
    finally:
        sys.dont_write_bytecode = previous_bytecode_setting
    return module


def discover_package_bundle(directory: Path, candidate: ModuleType) -> Path:
    try:
        root = directory.resolve(strict=True)
        entries = list(root.iterdir())
    except OSError as error:
        raise PackageError(f"cannot inspect release package bundle {directory}: {error}") from error
    if not root.is_dir():
        raise PackageError(f"release package bundle is not a directory: {directory}")
    archives = [entry for entry in entries if candidate.ARCHIVE_NAME.fullmatch(entry.name)]
    if len(archives) != 1:
        raise PackageError("release package bundle must contain exactly one Cyanrex .tar.gz archive")
    archive = archives[0]
    required = {archive.name, f"{archive.name}.sha256"}
    actual = {entry.name for entry in entries}
    if actual != required:
        missing = sorted(required - actual)
        unexpected = sorted(actual - required)
        details = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if unexpected:
            details.append(f"unexpected {', '.join(unexpected)}")
        raise PackageError(f"release package bundle file set is invalid: {'; '.join(details)}")
    candidate.regular_file(archive, "release package archive")
    candidate.verify_checksum_file(root / f"{archive.name}.sha256", archive)
    return archive


def verify_package(
    bundle: Path,
    options: argparse.Namespace,
    candidate: ModuleType,
) -> tuple[dict[str, Any], Path, str, dict[str, str]]:
    archive = discover_package_bundle(bundle, candidate)
    package_root, hashes, controls = candidate.inspect_archive(archive)
    candidate.verify_manifest(hashes, controls["checksums.sha256"])
    metadata = candidate.validate_metadata(
        controls["release-metadata.json"], package_root, archive.name, hashes
    )
    candidate.enforce_expectations(metadata, options)
    return metadata, archive, package_root, hashes


def output_path(value: str) -> Path:
    requested = Path(os.path.abspath(value))
    if not requested.name:
        raise PackageError("extraction output must name a new directory")
    try:
        parent = requested.parent.resolve(strict=True)
    except OSError as error:
        raise PackageError(f"extraction output parent does not exist: {requested.parent}") from error
    destination = parent / requested.name
    if os.path.lexists(destination):
        raise PackageError(f"refusing to overwrite extraction output: {destination}")
    return destination


def allowed_directories(package_root: str, hashes: dict[str, str]) -> set[str]:
    directories = {package_root}
    for relative in hashes:
        parts = relative.split("/")[:-1]
        for length in range(1, len(parts) + 1):
            directories.add("/".join((package_root, *parts[:length])))
    return directories


def open_output_file(path: Path):
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    return os.fdopen(descriptor, "wb")


def write_member(
    package: tarfile.TarFile,
    member: tarfile.TarInfo,
    destination: Path,
    expected_hash: str,
) -> None:
    stream = package.extractfile(member)
    if stream is None:
        raise PackageError(f"cannot read archive member {member.name!r}")
    digest = hashlib.sha256()
    size = 0
    try:
        with stream, open_output_file(destination) as output:
            while chunk := stream.read(1024 * 1024):
                output.write(chunk)
                digest.update(chunk)
                size += len(chunk)
    except OSError as error:
        raise PackageError(f"cannot write archive member {member.name!r}: {error}") from error
    if size != member.size:
        raise PackageError(f"archive member size changed during extraction: {member.name!r}")
    if digest.hexdigest() != expected_hash:
        raise PackageError(f"archive member changed after verification: {member.name!r}")
    destination.chmod(0o755 if member.mode & 0o111 else 0o644)


def extract_to_temporary(
    archive: Path,
    temporary: Path,
    package_root: str,
    hashes: dict[str, str],
    validate_path: Callable[[tarfile.TarInfo], tuple[str, list[str]]],
    max_members: int,
    max_bytes: int,
) -> Path:
    seen: set[str] = set()
    extracted: set[str] = set()
    allowed = allowed_directories(package_root, hashes)
    total_size = 0
    root_seen = False
    try:
        with tarfile.open(archive, mode="r|gz") as package:
            for count, member in enumerate(package, start=1):
                if count > max_members:
                    raise PackageError(f"archive exceeds the {max_members}-member limit")
                name, parts = validate_path(member)
                if parts[0] != package_root:
                    raise PackageError(f"archive member is outside {package_root}: {member.name!r}")
                if name in seen:
                    raise PackageError(f"archive contains duplicate member {name!r}")
                seen.add(name)
                destination = temporary.joinpath(*parts)
                if member.type == tarfile.DIRTYPE:
                    if name not in allowed:
                        raise PackageError(f"archive contains unexpected directory {name!r}")
                    destination.mkdir(parents=True, exist_ok=True, mode=0o755)
                    if not stat.S_ISDIR(destination.lstat().st_mode):
                        raise PackageError(f"archive directory conflicts with a file: {name!r}")
                    destination.chmod(0o755)
                    root_seen = root_seen or len(parts) == 1
                    continue
                if member.type not in {tarfile.REGTYPE, tarfile.AREGTYPE}:
                    raise PackageError(f"archive contains unsupported member type: {member.name!r}")
                if len(parts) == 1:
                    raise PackageError("archive package root must be a directory")
                relative = "/".join(parts[1:])
                expected_hash = hashes.get(relative)
                if expected_hash is None:
                    raise PackageError(f"archive contains unverified member {member.name!r}")
                total_size += member.size
                if member.size < 0 or total_size > max_bytes:
                    raise PackageError(f"archive exceeds the {max_bytes}-byte content limit")
                destination.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
                write_member(package, member, destination, expected_hash)
                extracted.add(relative)
    except PackageError:
        raise
    except (OSError, EOFError, tarfile.TarError, ValueError) as error:
        raise PackageError(f"cannot safely extract candidate archive: {error}") from error
    if not root_seen:
        raise PackageError("archive does not contain its declared package root directory")
    if extracted != set(hashes):
        missing = ", ".join(sorted(set(hashes) - extracted))
        raise PackageError(f"verified package members disappeared before extraction: {missing}")
    return temporary / package_root


def extract_verified_archive(
    archive: Path,
    package_root: str,
    hashes: dict[str, str],
    output: str,
    validate_path: Callable[[tarfile.TarInfo], tuple[str, list[str]]],
    max_members: int,
    max_bytes: int,
) -> Path:
    destination = output_path(output)
    temporary: Path | None = None
    destination_created = False
    try:
        destination.mkdir(mode=0o700)
        destination_created = True
        temporary = Path(tempfile.mkdtemp(prefix=f".{destination.name}.extract-", dir=destination.parent))
        package_directory = extract_to_temporary(
            archive,
            temporary,
            package_root,
            hashes,
            validate_path,
            max_members,
            max_bytes,
        )
        destination.chmod(0o755)
        final_package = destination / package_root
        if os.path.lexists(final_package):
            raise PackageError(f"refusing to overwrite extracted package: {final_package}")
        package_directory.rename(final_package)
        temporary.rmdir()
        temporary = None
        return final_package
    except BaseException:
        if temporary is not None:
            shutil.rmtree(temporary, ignore_errors=True)
        if destination_created:
            try:
                destination.rmdir()
            except OSError:
                pass
        raise


def add_expectations(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--expect-version")
    parser.add_argument("--expect-revision")
    parser.add_argument("--expect-tag")
    parser.add_argument("--expect-source-state", choices=("clean", "dirty", "unavailable"))
    parser.add_argument("--expect-image-mode", choices=("built", "prebuilt"))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    extract = commands.add_parser("extract", help="verify and safely extract one package bundle")
    extract.add_argument("bundle", help="directory containing one archive and its checksum")
    extract.add_argument("--output", required=True, help="new directory that will contain the package")
    add_expectations(extract)
    return parser


def main(argv: list[str] | None = None) -> int:
    options = build_parser().parse_args(argv)
    try:
        candidate = load_candidate_tool()
        metadata, archive, package_root, hashes = verify_package(
            Path(options.bundle), options, candidate
        )
        extracted = extract_verified_archive(
            archive,
            package_root,
            hashes,
            options.output,
            candidate.archive_path,
            candidate.MAX_ARCHIVE_MEMBERS,
            candidate.MAX_ARCHIVE_BYTES,
        )
    except (ValueError, OSError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    print(
        "[cyanrex] Release package verified and safely extracted: "
        f"{metadata['package']['version']} -> {extracted}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
