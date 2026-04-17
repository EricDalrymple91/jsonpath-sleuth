import pytest

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
        Note: Indexed access within filters (e.g., results[0]) is not
        supported; only wildcard patterns (e.g., results[*]) work.
        """
        obj = {
            "parties": [
                {"name": "V1", "results": [{"item": "A"}, {"item": "B"}]},
                {"name": "V2", "results": []},
                {"name": "V3", "results": [{"item": "A"}]},
            ]
        }

        # Nested wildcard in filter - now supported!
        # This checks if ANY item in the results array has item=='A'
        result_wildcard = resolve_jsonpath(
            obj, "parties[?(@.results[*].item=='A')].name"
        )
        assert result_wildcard == ["V1", "V3"]

    @pytest.mark.parametrize(
        "description,data,path,expected",
        [
            # Simple filter with single apostrophe
            (
                "simple filter with single apostrophe",
                [
                    {"name": "item with's", "value": 10},
                    {"name": "plain item", "value": 20},
                    {"name": "item with's", "value": 30},
                ],
                r"[?(@.name == 'item with\'s')].value",
                [10, 30],
            ),
            # Simple filter with nested structure and apostrophe
            (
                "simple filter with nested structure and apostrophe",
                {
                    "records": [
                        {"title": "alpha's beta", "amount": 50},
                        {"title": "other", "amount": 60},
                    ]
                },
                r"records[?(@.title == 'alpha\'s beta')].amount",
                [50],
            ),
            # Nested wildcard with single apostrophe
            (
                "nested wildcard with single apostrophe",
                {
                    "items": [
                        {
                            "name": "item1",
                            "results": [{"field": "value's type"}, {"field": "other"}],
                        },
                        {"name": "item2", "results": [{"field": "value's type"}]},
                        {"name": "item3", "results": [{"field": "different"}]},
                    ]
                },
                r"items[?(@.results[*].field=='value\'s type')].name",
                ["item1", "item2"],
            ),
            # Nested wildcard with different data structure
            (
                "nested wildcard with object type apostrophe",
                {
                    "items": [
                        {"id": "a", "results": [{"type": "object's type"}]},
                        {"id": "b", "results": [{"type": "object's type"}]},
                    ]
                },
                r"items[?(@.results[*].type=='object\'s type')].id",
                ["a", "b"],
            ),
            # Multiple apostrophes in simple filter
            (
                "multiple apostrophes in simple filter",
                [
                    {"name": "it's Bob's item", "value": 1},
                    {"name": "it's not his", "value": 2},
                    {"name": "it's Bob's item", "value": 3},
                ],
                r"[?(@.name == 'it\'s Bob\'s item')].value",
                [1, 3],
            ),
            # Multiple apostrophes in nested structure
            (
                "multiple apostrophes in nested structure",
                {
                    "data": [
                        {"desc": "Mary's and John's", "id": "x"},
                        {"desc": "Mary's only", "id": "y"},
                        {"desc": "Mary's and John's", "id": "z"},
                    ]
                },
                r"data[?(@.desc == 'Mary\'s and John\'s')].id",
                ["x", "z"],
            ),
        ],
    )
    def test_apostrophes_in_filter_expressions(
        self, description, data, path, expected
    ) -> None:
        """Test filter expressions with apostrophes (single and multiple).

        Covers:
        - Simple filters with apostrophes
        - Nested wildcard filters with apostrophes
        - Multiple apostrophes in a single filter value
        """
        result = resolve_jsonpath(data, path)
        assert result == expected, f"Failed for: {description}"


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


class TestRegressionScenarios:
    """
    Test the 4 critical regression scenarios from v0.1.9.
    These cover chained filters, logical operators, and wildcards after filters.
    """

    @pytest.fixture
    def insurance_data(self):
        """Sample insurance data used in regression tests."""
        return {
            "parties": [
                {
                    "first_name": "John",
                    "last_name": "Doe",
                    "insured_type": "primary_named_insured",
                    "credit_score": 750,
                    "length_of_residence": "1.5",
                    "credit_hit_status": "success",
                    "credit_reasons": [
                        {"code": "REASON_001"},
                        {"code": "REASON_002"},
                    ],
                    "communications": [
                        {"type": "EMAIL", "value": "john@example.com"},
                        {"type": "PHONE", "value": "555-0001"},
                    ],
                },
                {
                    "first_name": "Jane",
                    "last_name": "Smith",
                    "insured_type": "spouse",
                    "credit_score": 720,
                    "length_of_residence": "2.0",
                    "credit_hit_status": "success",
                    "credit_reasons": [
                        {"code": "REASON_003"},
                    ],
                    "communications": [
                        {"type": "EMAIL", "value": "jane@example.com"},
                    ],
                },
                {
                    "first_name": "Bob",
                    "last_name": "Johnson",
                    "insured_type": "dependent",
                    "credit_score": 700,
                    "length_of_residence": "1.5",
                    "credit_hit_status": "failed",
                    "credit_reasons": [],
                    "communications": [],
                },
                {
                    "first_name": "Spouse",
                    "last_name": "extra",
                    "insured_type": "extra",
                    "credit_score": 680,
                    "length_of_residence": "0.5",
                    "credit_hit_status": "success",
                    "credit_reasons": [{"code": "REASON_004"}],
                    "communications": [
                        {"type": "EMAIL", "value": "extra@example.com"},
                    ],
                },
            ]
        }

    def test_regression_1_chained_filters(self, insurance_data):
        """
        Regression 1: Chained filters were broken.
        Query: parties[?(@.insured_type == 'primary_named_insured')].communications[?(@.type == 'EMAIL')].value
        Expected: ['john@example.com']
        """
        result = resolve_jsonpath(
            insurance_data,
            r"parties[?(@.insured_type == 'primary_named_insured')].communications[?(@.type == 'EMAIL')].value",
        )
        assert result == ["john@example.com"]

    def test_regression_2_logical_and(self, insurance_data):
        """
        Regression 2: Logical AND in filters was broken.
        Query: parties[?(@.credit_score && @.length_of_residence == '1.5')].insured_type
        Expected: ['primary_named_insured', 'dependent']
        """
        result = resolve_jsonpath(
            insurance_data,
            r"parties[?(@.credit_score && @.length_of_residence == '1.5')].insured_type",
        )
        assert sorted(result) == sorted(["primary_named_insured", "dependent"])

    def test_regression_3_logical_or(self, insurance_data):
        """
        Regression 3: Logical OR in filters was broken.
        Query: parties[?(@.first_name == 'Spouse' || @.last_name == 'extra')].first_name
        Expected: ['Spouse']
        """
        result = resolve_jsonpath(
            insurance_data,
            r"parties[?(@.first_name == 'Spouse' || @.last_name == 'extra')].first_name",
        )
        assert result == ["Spouse"]

    def test_regression_4_wildcard_after_filter(self, insurance_data):
        """
        Regression 4: Wildcards after filters were broken.
        Query: parties[?(@.credit_hit_status == 'success')].credit_reasons[*].code
        Expected: ['REASON_001', 'REASON_002', 'REASON_003', 'REASON_004']
        """
        result = resolve_jsonpath(
            insurance_data,
            r"parties[?(@.credit_hit_status == 'success')].credit_reasons[*].code",
        )
        assert sorted(result) == sorted(
            ["REASON_001", "REASON_002", "REASON_003", "REASON_004"]
        )
