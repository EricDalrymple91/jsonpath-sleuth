"""Release checks run without importing the Rust extension or downloading packages."""

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def release():
    spec = importlib.util.spec_from_file_location(
        "wheel_release", ROOT / "scripts/wheel_release.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def wheel(directory, architecture="x86_64", version="0.1.11", tag=None):
    tag = tag or f"cp38-abi3-manylinux_2_39_{architecture}"
    path = directory / f"jsonpath_sleuth-{version}-{tag}.whl"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(
            f"jsonpath_sleuth-{version}.dist-info/METADATA",
            f"Name: jsonpath-sleuth\nVersion: {version}\n",
        )
        archive.writestr(
            f"jsonpath_sleuth-{version}.dist-info/WHEEL",
            f"Wheel-Version: 1.0\nTag: {tag}\n",
        )
        archive.writestr(
            "jsonpath_sleuth.abi3.so", b"fixture, not an executable extension"
        )
    return path


def test_manifest_records_exact_wheel_hash_and_source(release, tmp_path):
    artifact = wheel(tmp_path)
    data = release.manifest(tmp_path, "x86_64", "a" * 40, "0.1.11")
    assert data["source_commit"] == "a" * 40
    assert data["files"] == {
        artifact.name: hashlib.sha256(artifact.read_bytes()).hexdigest()
    }


@pytest.mark.parametrize(
    "tag", ["cp314-cp314-linux_x86_64", "cp38-abi3-manylinux_2_39_aarch64"]
)
def test_rejects_wrong_abi_or_platform(release, tmp_path, tag):
    wheel(tmp_path, tag=tag)
    with pytest.raises(ValueError):
        release.manifest(tmp_path, "x86_64", "a" * 40, "0.1.11")


def test_rejects_changed_version(release, tmp_path):
    wheel(tmp_path, version="0.1.12")
    with pytest.raises(ValueError):
        release.manifest(tmp_path, "x86_64", "a" * 40, "0.1.11")


def test_rejects_missing_extension(release, tmp_path):
    artifact = wheel(tmp_path)
    with zipfile.ZipFile(artifact, "w") as archive:
        archive.writestr("not-a-native-wheel.txt", "fixture")
    with pytest.raises(ValueError):
        release.manifest(tmp_path, "x86_64", "a" * 40, "0.1.11")


def test_publish_requires_both_tested_architectures_same_commit_and_version(
    release, tmp_path
):
    for architecture in ["x86_64", "aarch64"]:
        wheel(tmp_path, architecture)
        data = release.manifest(tmp_path, architecture, "a" * 40, "0.1.11")
        (tmp_path / f"manifest-{architecture}.json").write_text(json.dumps(data))
    release.verify(tmp_path, "a" * 40)
    artifact = next(tmp_path.glob("*x86_64.whl"))
    artifact.write_bytes(b"tampered")
    with pytest.raises(ValueError):
        release.verify(tmp_path, "a" * 40)


def test_publish_rejects_missing_architecture(release, tmp_path):
    wheel(tmp_path)
    data = release.manifest(tmp_path, "x86_64", "a" * 40, "0.1.11")
    (tmp_path / "manifest-x86_64.json").write_text(json.dumps(data))
    with pytest.raises(ValueError):
        release.verify(tmp_path, "a" * 40)


def test_workflow_does_not_publish_from_pr_or_fork():
    workflow = (ROOT / ".github/workflows/linux-wheels.yml").read_text()
    assert "workflow_dispatch:" in workflow
    assert "pull_request_target" not in workflow
    assert "github.repository == 'EricDalrymple91/jsonpath-sleuth'" in workflow
    assert "github.ref == 'refs/heads/main'" in workflow
    assert "environment: pypi" in workflow
    assert "needs: [build, test]" in workflow
    assert "--trusted-publishing always" in workflow
    assert "id-token: write" in workflow
    assert "pip install -U" not in workflow


def test_build_cannot_bootstrap_rust_or_use_public_python_registry():
    script = (ROOT / "scripts/build_linux_wheel.sh").read_text()
    setup = (ROOT / "scripts/setup_wheel_tools.sh").read_text()
    assert "--locked" in script and "--offline" in script
    assert "registry.packages.sureapp.com" in script
    assert 'replace-with = "socket"' in script
    assert "--exclude-newer '7 days'" in setup
    assert "--require-hashes" in setup
    assert "UV_PYTHON_DOWNLOADS=never" in setup
    assert "rustup install" not in script
    assert "sh.rustup.rs" not in script
    assert "pypi.org/simple" not in script


def test_publishing_is_opt_in_and_uses_no_long_lived_pypi_token():
    workflow = (ROOT / ".github/workflows/linux-wheels.yml").read_text()
    assert "default: false" in workflow
    assert "vars.WHEEL_PUBLISH_ENABLED == 'true'" in workflow
    assert "vars.WHEEL_BUILDS_ENABLED == 'true'" in workflow
    assert "PYPI_API_TOKEN" not in workflow
    assert "pull_request:" not in workflow


def test_rejects_version_mismatch_between_manifests(release, tmp_path):
    for architecture, version in [("x86_64", "0.1.11"), ("aarch64", "0.1.12")]:
        wheel(tmp_path, architecture, version)
        data = release.manifest(tmp_path, architecture, "a" * 40, version)
        (tmp_path / f"manifest-{architecture}.json").write_text(json.dumps(data))
    with pytest.raises(ValueError):
        release.verify(tmp_path, "a" * 40)


def test_rejects_artifacts_from_another_commit(release, tmp_path):
    for architecture in ["x86_64", "aarch64"]:
        wheel(tmp_path, architecture)
        data = release.manifest(tmp_path, architecture, "a" * 40, "0.1.11")
        (tmp_path / f"manifest-{architecture}.json").write_text(json.dumps(data))
    with pytest.raises(ValueError):
        release.verify(tmp_path, "b" * 40)


def test_rejects_unexpected_source_archive(release, tmp_path):
    for architecture in ["x86_64", "aarch64"]:
        wheel(tmp_path, architecture)
        data = release.manifest(tmp_path, architecture, "a" * 40, "0.1.11")
        (tmp_path / f"manifest-{architecture}.json").write_text(json.dumps(data))
    (tmp_path / "jsonpath_sleuth-0.1.11.tar.gz").write_bytes(
        b"unreviewed source distribution"
    )
    with pytest.raises(ValueError):
        release.verify(tmp_path, "a" * 40)


@pytest.mark.parametrize("script", ["setup_wheel_tools.sh", "build_linux_wheel.sh"])
@pytest.mark.parametrize(
    "missing", ["UV_INDEX_SOCKET_USERNAME", "UV_INDEX_SOCKET_PASSWORD"]
)
def test_scripts_fail_before_external_tools_without_credentials(
    tmp_path, script, missing
):
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(tmp_path),
        "RUNNER_TEMP": str(tmp_path),
        "GITHUB_SHA": "a" * 40,
        "UV_INDEX_SOCKET_USERNAME": "fixture-user",
        "UV_INDEX_SOCKET_PASSWORD": "fixture-password",
    }
    env.pop(missing)
    result = subprocess.run(
        ["bash", str(ROOT / "scripts" / script)],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "Socket" in result.stderr
    assert "fixture-password" not in result.stderr


def executable(path, source):
    path.write_text(f"#!{sys.executable}\n" + source)
    path.chmod(0o755)


@pytest.fixture
def simulated_runner(tmp_path):
    """Fake tool binaries test shell orchestration, never compile or fetch a crate."""
    (tmp_path / "scripts").mkdir()
    for name in ["Cargo.toml", "pyproject.toml", "scripts/wheel_release.py"]:
        shutil.copy2(ROOT / name, tmp_path / name)
    home = tmp_path / "home"
    tools = home / ".rustup/toolchains/stable-fixture/bin"
    tools.mkdir(parents=True)
    runner = tmp_path / "runner"
    (runner / "wheel-tools").mkdir(parents=True)
    (runner / "wheel-tools/requirements.txt").write_text("fixture tools")
    binaries = tmp_path / "bin"
    binaries.mkdir()
    (binaries / "python").symlink_to(sys.executable)
    executable(binaries / "uname", "print('x86_64')\n")
    executable(
        binaries / "git",
        "import sys; assert sys.argv[1:] == ['diff', '--exit-code', '--', 'Cargo.lock', 'Cargo.toml', 'pyproject.toml', 'src']\n",
    )
    executable(tools / "rustc", "print('rustc fixture: no toolchain was downloaded')\n")
    executable(
        tools / "cargo",
        """
import base64, json, os, pathlib, sys
args = sys.argv[1:]
if args == ['--version']:
    print('cargo fixture')
    raise SystemExit(0)
home = pathlib.Path(os.environ['CARGO_HOME'])
assert 'replace-with = "socket"' in (home / 'config.toml').read_text()
assert 'registry.packages.sureapp.com/cargo/' in (home / 'config.toml').read_text()
assert os.environ['CARGO_REGISTRIES_SOCKET_TOKEN'] == 'Basic ' + base64.b64encode(b'fixture-user:fixture-password').decode()
assert args in [['fetch', '--locked'], ['test', '--locked', '--offline', '--no-default-features']]
if args[0] == 'test':
    assert os.environ['CARGO_NET_OFFLINE'] == 'true'
with open('cargo-calls.jsonl', 'a') as output:
    output.write(json.dumps({'args': args, 'home': str(home)}) + '\\n')
""",
    )
    executable(
        binaries / "maturin",
        """
import os, pathlib, sys, zipfile
assert os.environ['CARGO_NET_OFFLINE'] == 'true'
assert sys.argv[1:] == ['build', '--release', '--locked', '--offline', '--compatibility', 'manylinux_2_39', '--out', 'dist']
directory = pathlib.Path('dist'); directory.mkdir()
with zipfile.ZipFile(directory / 'jsonpath_sleuth-0.1.11-cp38-abi3-manylinux_2_39_x86_64.whl', 'w') as archive:
    archive.writestr('jsonpath_sleuth-0.1.11.dist-info/METADATA', 'Name: jsonpath-sleuth\\nVersion: 0.1.11\\n')
    archive.writestr('jsonpath_sleuth-0.1.11.dist-info/WHEEL', 'Tag: cp38-abi3-manylinux_2_39_x86_64\\n')
    archive.writestr('jsonpath_sleuth.abi3.so', b'non-executable fixture')
""",
    )
    env = {
        "PATH": f"{binaries}:/usr/bin:/bin",
        "HOME": str(home),
        "RUNNER_TEMP": str(runner),
        "GITHUB_SHA": "a" * 40,
        "UV_INDEX_SOCKET_USERNAME": "fixture-user",
        "UV_INDEX_SOCKET_PASSWORD": "fixture-password",
    }
    return tmp_path, env


def test_build_orchestration_fetches_locked_then_builds_offline(simulated_runner):
    directory, env = simulated_runner
    result = subprocess.run(
        ["bash", str(ROOT / "scripts/build_linux_wheel.sh")],
        cwd=directory,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    calls = [
        json.loads(line)
        for line in (directory / "cargo-calls.jsonl").read_text().splitlines()
    ]
    assert [call["args"][0] for call in calls] == ["fetch", "test"]
    assert all(not (Path(call["home"]) / "config.toml").exists() for call in calls)
    assert (directory / "dist/manifest-x86_64.json").is_file()
    assert all(
        b"fixture-password" not in path.read_bytes()
        for path in directory.rglob("*")
        if path.is_file() and path.suffix not in [".py", ""]
    )


def test_no_installed_compiler_fails_without_bootstrap(simulated_runner):
    directory, env = simulated_runner
    env["HOME"] = str(directory / "empty-home")
    result = subprocess.run(
        ["bash", str(ROOT / "scripts/build_linux_wheel.sh")],
        cwd=directory,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "refusing to download" in result.stderr
    assert not (directory / "cargo-calls.jsonl").exists()
