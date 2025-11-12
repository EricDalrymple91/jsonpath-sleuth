from typing import Dict, List, Union

# Minimal JSON type used by the bindings
JSONScalar = Union[str, int, float, bool, None]
JSONValue = Union[JSONScalar, Dict[str, "JSONValue"], List["JSONValue"]]

def resolve_jsonpath(data: JSONValue, path: str, /) -> List[JSONValue]: ...
def find_jsonpaths_by_value(data: JSONValue, target: JSONValue, /) -> List[str]: ...
