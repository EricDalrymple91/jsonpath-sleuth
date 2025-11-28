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


## Notes

### JSONPath Support
- Standard JSONPath is powered by `jsonpath-rust` crate
- Nested wildcard filters use custom implementation for enhanced functionality
- JSONPath keys with spaces or special characters must be quoted using bracket notation
  - Example: use `a['some key'].next` instead of `a.some key.next`
  - You may omit the leading `$`; the resolver adds it automatically

### Path Format
- Paths produced by value search use `.` between object keys and `[idx]` for arrays
- If the entire input equals the target, no paths are returned (empty list)

### Supported Patterns
- ✅ Standard wildcards: `store.book[*].title`
- ✅ Filters: `store.book[?(@.price < 10)].title`
- ✅ **Nested wildcard filters**: `parties[?(@.results[*].item=='A')].name`
- ✅ Recursive descent: `$..price`
- ✅ Array slices: `store.book[0:2].title`
