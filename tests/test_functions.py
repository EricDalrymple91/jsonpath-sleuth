from jsonpath_sleuth import (
    resolve_jsonpath,
    find_jsonpaths_by_value,
    extract_jsonpaths_and_values,
)


class TestResolveJSONPath:
    def test_titles(self) -> None:
        obj = {
            "store": {
                "book": [
                    {"category": "fiction", "title": "Sword"},
                    {"category": "fiction", "title": "Shield"},
                ],
                "bicycle": {"color": "red", "price": 19.95},
            }
        }
        # short form without '$' is accepted
        assert resolve_jsonpath(obj, "store.book[*].title") == ["Sword", "Shield"]
        # explicit root also works
        assert resolve_jsonpath(obj, "$.store.book[*].title") == ["Sword", "Shield"]

    def test_filter_by_title(self) -> None:
        obj = {
            "store": {
                "book": [
                    {"category": "fiction", "title": "Sword"},
                    {"category": "fiction", "title": "Shield"},
                ]
            }
        }
        # short form (no '$') with filter
        assert resolve_jsonpath(obj, "store.book[?(@.title == 'Sword')].category") == [
            "fiction"
        ]
        # explicit root
        assert resolve_jsonpath(
            obj, "$.store.book[?(@.title == 'Sword')].category"
        ) == ["fiction"]

    def test_simple_dot_only(self) -> None:
        obj = {"a": {"b": {"c": 1}}}
        assert resolve_jsonpath(obj, "a.b.c") == [1]
        # also supports optional '$.' prefix
        assert resolve_jsonpath(obj, "$.a.b.c") == [1]

    def test_simple_keys_with_spaces_and_dashes(self) -> None:
        obj = {"a b": {"c-d_e": {"k": "v"}}}
        # JSONPath requires quoting keys with spaces/special chars
        assert resolve_jsonpath(obj, "['a b']['c-d_e'].k") == ["v"]

    def test_simple_missing_key_returns_empty(self) -> None:
        obj = {"a": {"b": {"c": 1}}}
        assert resolve_jsonpath(obj, "a.b.x") == []

    def test_nested_wildcard_in_filter(self) -> None:
        """
        Test nested wildcards in filter expressions.

        This package now supports nested wildcards in filter predicates
        like [?(@.results[*].item=='A')] through custom implementation.
        """
        obj = {
            "parties": [
                {"name": "V1", "results": [{"item": "A"}, {"item": "B"}]},
                {"name": "V2", "results": []},
                {"name": "V3", "results": [{"item": "A"}]},
            ]
        }

        # Nested wildcard in filter - now supported!
        result_wildcard = resolve_jsonpath(
            obj, "parties[?(@.results[*].item=='A')].name"
        )
        assert result_wildcard == ["V1", "V3"]

        # Specific index also works
        result_indexed = resolve_jsonpath(
            obj, "parties[?(@.results[0].item=='A')].name"
        )
        assert result_indexed == ["V1", "V3"]


class TestFindJSONPathsByValue:
    def test_multiple_hits(self) -> None:
        obj = {
            "a": {"b": 1, "c": [1, 2]},
            "d": [{"e": 1}, 2, 1],
        }
        paths = sorted(find_jsonpaths_by_value(obj, 1))
        assert paths == sorted(["a.b", "a.c[0]", "d[0].e", "d[2]"])

    def test_no_match(self) -> None:
        obj = {"a": 1, "b": [2, 3]}
        paths = find_jsonpaths_by_value(obj, 999)
        assert paths == []


class TestExtractJSONPathsAndValues:
    def test_extract_basic(self) -> None:
        obj = {
            "a": {"b": 1, "c": [1, 2]},
            "d": [{"e": 1}, 2, 1],
        }
        pairs = sorted(extract_jsonpaths_and_values(obj))
        assert pairs == sorted(
            [
                ("a.b", 1),
                ("a.c[0]", 1),
                ("a.c[1]", 2),
                ("d[0].e", 1),
                ("d[1]", 2),
                ("d[2]", 1),
            ]
        )

    def test_extract_scalars(self) -> None:
        obj = ["x", 10, True, None, 1.5]
        pairs = sorted(extract_jsonpaths_and_values(obj))
        assert pairs == sorted(
            [
                ("[0]", "x"),
                ("[1]", 10),
                ("[2]", True),
                ("[3]", None),
                ("[4]", 1.5),
            ]
        )
