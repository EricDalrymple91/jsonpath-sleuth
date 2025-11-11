# JSONPath Sleuth

Fast Python bindings (via Rust + PyO3) for:

- Resolving JSONPath expressions against Python dict/list JSON
- Finding JSONPath-like paths for all occurrences of a target value

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
  - `cargo test`
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

## Examples

```python
from jsonpath_sleuth import resolve_jsonpath, find_jsonpaths_by_value

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
```

## Notes
- JSONPath is powered by `jsonpath_lib` crate.
- Paths produced by value search:
  - Use `.` between object keys and `[idx]` for arrays.
  - If the entire input equals the target, no paths are returned (empty list).
