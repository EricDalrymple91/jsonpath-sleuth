from jsonpath_sleuth import resolve_jsonpath, find_jsonpaths_by_value


class TestResolveJSONPath:
    def test_titles(self):
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

    def test_filter_by_title(self):
        obj = {
            "store": {
                "book": [
                    {"category": "fiction", "title": "Sword"},
                    {"category": "fiction", "title": "Shield"},
                ]
            }
        }
        # short form (no '$') with filter
        assert resolve_jsonpath(obj, "store.book[?(@.title == 'Sword')].category") == ["fiction"]
        # explicit root
        assert resolve_jsonpath(obj, "$.store.book[?(@.title == 'Sword')].category") == ["fiction"]


class TestFindJSONPathsByValue:
    def test_multiple_hits(self):
        obj = {
            "a": {"b": 1, "c": [1, 2]},
            "d": [{"e": 1}, 2, 1],
        }
        paths = sorted(find_jsonpaths_by_value(obj, 1))
        assert paths == sorted(["a.b", "a.c[0]", "d[0].e", "d[2]"])

    def test_no_match(self):
        obj = {"a": 1, "b": [2, 3]}
        paths = find_jsonpaths_by_value(obj, 999)
        assert paths == []

