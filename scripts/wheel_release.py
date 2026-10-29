"""Validate wheel identity before uploading artifacts or publishing to PyPI."""

import argparse
from email.parser import BytesParser
import hashlib
import json
from pathlib import Path
import re
import zipfile

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.8-3.10 wheel smoke tests
    import tomli as tomllib


ARCHITECTURES = ("x86_64", "aarch64")


def project_version(root):
    python = tomllib.loads((root / "pyproject.toml").read_text())["project"]["version"]
    rust = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if python != rust:
        raise ValueError("Cargo and Python package versions disagree")
    return python


def manifest(directory, architecture, commit, version):
    if architecture not in ARCHITECTURES or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Invalid architecture or source commit")
    suffix = f"cp38-abi3-manylinux_2_39_{architecture}"
    expected = f"jsonpath_sleuth-{version}-{suffix}.whl"
    wheels = list(directory.glob(f"*{architecture}.whl"))
    if [wheel.name for wheel in wheels] != [expected]:
        raise ValueError(f"Expected exactly {expected}")
    wheel = wheels[0]
    prefix = f"jsonpath_sleuth-{version}.dist-info/"
    with zipfile.ZipFile(wheel) as archive:
        if not {prefix + "METADATA", prefix + "WHEEL"}.issubset(archive.namelist()):
            raise ValueError("Wheel metadata is missing")
        metadata = BytesParser().parsebytes(archive.read(prefix + "METADATA"))
        tags = (
            BytesParser().parsebytes(archive.read(prefix + "WHEEL")).get_all("Tag", [])
        )
        if metadata["Name"] != "jsonpath-sleuth" or metadata["Version"] != version:
            raise ValueError("Wheel metadata disagrees with source")
        if tags != [suffix] or not any(
            name.endswith(".abi3.so") for name in archive.namelist()
        ):
            raise ValueError("Expected the native CPython 3.8+ stable ABI wheel")
    return {
        "source_commit": commit,
        "version": version,
        "architecture": architecture,
        "files": {wheel.name: hashlib.sha256(wheel.read_bytes()).hexdigest()},
    }


def verify(directory, commit):
    manifests = sorted(directory.glob("manifest-*.json"))
    if len(manifests) != len(ARCHITECTURES):
        raise ValueError("Both architecture manifests are required")
    seen = set()
    versions = set()
    expected_files = set()
    for path in manifests:
        data = json.loads(path.read_text())
        architecture = data["architecture"]
        if architecture in seen or architecture not in ARCHITECTURES:
            raise ValueError("Duplicate or unexpected architecture")
        if data["source_commit"] != commit:
            raise ValueError("Artifact came from another source commit")
        expected = f"jsonpath_sleuth-{data['version']}-cp38-abi3-manylinux_2_39_{architecture}.whl"
        if set(data["files"]) != {expected}:
            raise ValueError("Unexpected file in artifact manifest")
        if (
            hashlib.sha256((directory / expected).read_bytes()).hexdigest()
            != data["files"][expected]
        ):
            raise ValueError("Wheel bytes changed after build")
        if data != manifest(directory, architecture, commit, data["version"]):
            raise ValueError("Artifact metadata or hash changed")
        seen.add(architecture)
        versions.add(data["version"])
        expected_files.update(data["files"])
    if seen != set(ARCHITECTURES) or len(versions) != 1:
        raise ValueError("Both architectures must have the same version")
    if {path.name for path in directory.glob("*.whl")} != expected_files:
        raise ValueError("Unexpected wheels in publication directory")
    if list(directory.glob("*.tar.gz")) or list(directory.glob("*.zip")):
        raise ValueError(
            "This workflow publishes tested wheels only, not source archives"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["manifest", "verify"])
    parser.add_argument("--directory", type=Path, default=Path("dist"))
    parser.add_argument("--commit", required=True)
    parser.add_argument("--architecture", choices=ARCHITECTURES)
    args = parser.parse_args()
    if args.mode == "verify":
        verify(args.directory, args.commit)
    else:
        data = manifest(
            args.directory, args.architecture, args.commit, project_version(Path.cwd())
        )
        (args.directory / f"manifest-{args.architecture}.json").write_text(
            json.dumps(data, indent=2) + "\n"
        )


if __name__ == "__main__":
    main()
