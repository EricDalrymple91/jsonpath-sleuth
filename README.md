# JSONPath Sleuth

Fast Python bindings (via Rust + PyO3) for:

- Resolving JSONPath expressions against Python dict/list JSON
- Finding JSONPath-like paths for all occurrences of a target value
- Extracting all JSONPath-like paths paired with their leaf values

## Install / Build

- Option A: editable dev install
  - pip install maturin
  - maturin develop -m pyproject.toml
- Option B: build wheel
  - maturin build -m pyproject.toml
  - pip install dist/*.whl

Requires Python 3.8+.

## Run Tests

- Rust unit tests (no Python needed)
  - `cargo test --no-default-features`
  - Also compile PyO3 bindings: `PYO3_PYTHON=$(which python3) cargo test --features python`

- Python tests (pytest)
  - Option A (quick):
    - `python3 -m venv .venv && source .venv/bin/activate`
    - `pip install -U pip pytest maturin`
    - `maturin develop -m Cargo.toml --features python`
    - `pytest -q`
  - Option B (dev extra):
    - `python3 -m venv .venv && source .venv/bin/activate`
    - `pip install -U pip`
    - `pip install -e .[dev]`
    - `maturin develop -m Cargo.toml --features python`
    - `pytest -q`

Notes
- Re-run `maturin develop` after Rust changes to refresh the extension in your venv.
- If pytest cannot import `jsonpath_sleuth`, ensure you activated the same venv used for `maturin develop`.

## Publish

### Linux wheel automation (maintainer setup required)

`.github/workflows/linux-wheels.yml` adds **manual, opt-in** builds on native
Ubuntu 24.04 x86_64 and aarch64 runners. It uses their already-installed Rust
compiler directly, not rustup or a Python build backend that bootstraps Rust.
Cargo downloads the committed lock through Socket, then compiles/tests offline.
Python build/test tools resolve through Socket with a seven-day release floor,
native generated hashes and binary-only installation. No package downloads use
public PyPI/crates.io, and no repository credentials are exposed to PRs or forks.

This first target is **CPython 3.8+ stable ABI, glibc 2.39+**
(`cp38-abi3-manylinux_2_39_x86_64` and `..._aarch64`). It supports Debian 13 and
Ubuntu 24.04, **not** older glibc, musl/Alpine, PyPy or free-threaded CPython.
Maturin audits the explicit manylinux compatibility target; it must fail rather
than mislabel an incompatible wheel. Older Linux support would require a
separately approved/provisioned build image, not an automatic Rust download.

The workflow installs each wheel offline into Python 3.8 through 3.14 on both
architectures and runs the existing pytest suite plus release-guard tests.
Only artifacts from that same workflow commit, with matching versions and
hashes, can be published. No source archive is published by this workflow.
Compiler details and the hash-pinned Python tool resolution accompany artifacts.

Before enabling it, the repository owner must:

1. Review the workflow and the native runner/toolchain and glibc baseline.
2. Create a protected `wheel-build` environment with approved, narrowly scoped
   `SOCKET_FIREWALL_USERNAME` and `SOCKET_FIREWALL_PASSWORD` secrets. Do not reuse
   Sure AWS deployment credentials or grant this public repo AWS access.
3. Set repository variable `WHEEL_BUILDS_ENABLED=true`. Missing Socket credentials
   fail closed; there is no fallback to public package registries.
4. Configure a protected `pypi` environment with required human approval and a
   PyPI Trusted Publisher for owner `EricDalrymple91`, repository
   `jsonpath-sleuth`, workflow `linux-wheels.yml`, environment `pypi`.
   No long-lived PyPI API token is used.
5. Only after a successful build/test-only run, set `WHEEL_PUBLISH_ENABLED=true`.
   Manually dispatch on `main` with `publish=true` and approve the publishing
   environment. The default is build/test only; publishing must be explicit.

This PR does not configure secrets/environments, create a publisher, bump a
version, run a release or change Rust/Python runtime code. Existing PyPI artifacts
cannot be overwritten. Choose a new package version if the intended filenames
already exist, keeping Cargo and Python versions aligned.

For Document Center adoption, compare upstream against its slimmed vendored
implementation and validate the stored JSONPath corpus before changing the app.
Installing a wheel must not silently fall back to source: consumers should use
their approved Socket/private index and require a binary for `jsonpath-sleuth`.

Run release-guard tests with the existing pytest tooling:

```bash
python -m pytest -q tests/test_wheel_release.py
```

The manual commands below are the original upstream publishing instructions,
not the Socket-routed automation described above.

- Build wheels + sdist
  - `maturin build -m Cargo.toml --features python --release --sdist`

- TestPyPI (requires separate TestPyPI account and token)
  - Publish: `maturin publish -m Cargo.toml --features python --repository-url https://test.pypi.org/legacy/ -u __token__ -p <pypi-TEST_TOKEN>`
  - Install to verify: `pip install -i https://test.pypi.org/simple jsonpath-sleuth`

- PyPI
  - Publish: `maturin publish -m Cargo.toml --features python -u __token__ -p <pypi-PROD_TOKEN>`
  - Install to verify: `pip install jsonpath-sleuth`

Tips
- Bump version in both `Cargo.toml` and `pyproject.toml` before publishing a new release.
- Tokens begin with `pypi-`. Avoid committing tokens; pass on the command line or configure `~/.pypirc`.

## Python API

Module: `jsonpath_sleuth`

- `resolve_jsonpath(data: dict | list, path: str) -> list[Any]`
  - Returns a list of matched values for the given JSONPath. The path may omit the leading `$` (it is added automatically).
- `find_jsonpaths_by_value(data: dict | list, target: Any) -> list[str]`
  - Returns string paths like `foo.bar[0].baz` where value equals `target`.
- `extract_jsonpaths_and_values(data: dict | list) -> list[tuple[str, Any]]`
  - Returns all JSONPath-like paths paired with their leaf values. Paths use `.` for object keys and `[idx]` for arrays.

## Examples

```python
from jsonpath_sleuth import resolve_jsonpath, find_jsonpaths_by_value, extract_jsonpaths_and_values

obj = {
    "store": {
        "book": [
            {"category": "fiction", "title": "Sword"},
            {"category": "fiction", "title": "Shield"},
        ],
        "bicycle": {"color": "red", "price": 19.95},
    }
}

# 1) Resolve JSONPath (prefix not required)
print(resolve_jsonpath(obj, "store.book[*].title"))
# -> ["Sword", "Shield"]
# Also works with explicit JSONPath:
print(resolve_jsonpath(obj, "$.store.book[*].title"))
# -> ["Sword", "Shield"]

# 2) Find paths by target value
print(find_jsonpaths_by_value(obj, "fiction"))
# -> ["store.book[0].category", "store.book[1].category"]

# 3) Extract all leaf paths and values
print(extract_jsonpaths_and_values(obj))
# -> [
#     ("store.book[0].category", "fiction"),
#     ("store.book[0].title", "Sword"),
#     ("store.book[1].category", "fiction"),
#     ("store.book[1].title", "Shield"),
#     ("store.bicycle.color", "red"),
#     ("store.bicycle.price", 19.95),
# ]
```

### Advanced: Nested Wildcard Filters

JSONPath Sleuth supports **nested wildcard filters** - a powerful feature that most JSONPath libraries don't handle well:

```python
from jsonpath_sleuth import resolve_jsonpath

data = {
    "parties": [
        {
            "name": "Alice",
            "results": [
                {"item": "A"},
                {"item": "B"}
            ]
        },
        {
            "name": "Bob",
            "results": []
        },
        {
            "name": "Charlie",
            "results": [
                {"item": "A"},
                {"item": "C"}
            ]
        }
    ]
}

# Find all parties that have ANY result with item='A'
print(resolve_jsonpath(data, "parties[?(@.results[*].item=='A')].name"))
# -> ["Alice", "Charlie"]

# This pattern works: <base>[?(@.<nested_array>[*].<field>=='<value>')].<result_field>
```

**How it works:**
- Checks if ANY item in the nested array matches the condition
- Returns the specified field from matching parent objects
- Custom implementation handles what standard JSONPath libraries can't

### Apostrophes in Filter Values

Filter expressions support apostrophes in comparison strings by escaping them with backslashes:

```python
from jsonpath_sleuth import resolve_jsonpath

# Simple filter with apostrophe
data = [
    {"name": "item with's", "amount": 100},
    {"name": "plain item", "amount": 200},
    {"name": "item with's", "amount": 300},
]

result = resolve_jsonpath(data, r"[?(@.name == 'item with\'s')].amount")
# -> [100, 300]

# Nested wildcard with apostrophe
nested_data = {
    "items": [
        {"name": "item1", "results": [{"type": "value's type"}, {"type": "other"}]},
        {"name": "item2", "results": [{"type": "value's type"}]},
    ]
}

result = resolve_jsonpath(
    nested_data, 
    r"items[?(@.results[*].type=='value\'s type')].name"
)
# -> ["item1", "item2"]

# Multiple apostrophes in a single filter
multi_data = [
    {"desc": "Mary's and John's", "id": "a"},
    {"desc": "other", "id": "b"},
]

result = resolve_jsonpath(multi_data, r"[?(@.desc == 'Mary\'s and John\'s')].id")
# -> ["a"]
```

When writing JSONPath queries in Python:
- Use raw strings (`r"..."`) to avoid double-escaping: `r"[?(@.name == 'user\'s name')]"`
- Escape apostrophes with backslash: `\'`
- Works with both simple and nested wildcard filters

## Notes

### JSONPath Support
- Standard JSONPath is powered by `jsonpath-rust` crate
- Nested wildcard filters use custom implementation for enhanced functionality
- JSONPath keys with spaces or special characters must be quoted using bracket notation
  - Example: use `a['some key'].next` instead of `a.some key.next`
  - You may omit the leading `$`; the resolver adds it automatically
- Filter values with apostrophes must escape them: `'user\'s name'`

### Path Format
- Paths produced by value search use `.` between object keys and `[idx]` for arrays
- If the entire input equals the target, no paths are returned (empty list)

### Supported Patterns
- ✅ Standard wildcards: `store.book[*].title`
- ✅ Filters: `store.book[?(@.price < 10)].title`
- ✅ **Nested wildcard filters**: `parties[?(@.results[*].item=='A')].name`
- ✅ Recursive descent: `$..price`
- ✅ Array slices: `store.book[0:2].title`
