#[cfg(feature = "python")]
use pyo3::exceptions::PyValueError;
#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyModule;
#[cfg(feature = "python")]
use pythonize::{depythonize, pythonize};
use serde_json::Value;

#[allow(dead_code)]
fn normalize_jsonpath(path: &str) -> String {
    let p = path.trim();
    match p.chars().next() {
        None => "$.".to_owned(),
        Some('$') => p.to_owned(),
        Some('.') | Some('[') => format!("${}", p),
        _ => format!("$.{}", p),
    }
}

/// Custom evaluation of filters with nested wildcards like: parties[?(@.results[*].item=='A')].name
/// Also supports escaped apostrophes: parties[?(@.results[*].item=='some guy\'s item')].name
#[allow(dead_code)]
fn evaluate_nested_wildcard_filter(data: &Value, path: &str) -> Result<Vec<Value>, String> {
    use jsonpath_rust::JsonPath;

    // Parse: <base>[?(@.<nested_array>[*].<field>=='<value>')].<result>
    // Updated regex to handle escaped quotes: 'value with \'escaped\''
    let re = regex::Regex::new(
        r"^(.*?)\[\?\(@\.(.*?)\[\*\]\.(.*?)\s*==\s*'((?:\\.|[^'])*?)'\)\]\.?(.*)$",
    )
    .map_err(|e| format!("Regex error: {e}"))?;

    let caps = re.captures(path).ok_or("Not a nested wildcard filter")?;
    let (base, nested_arr, nested_field, expected_raw, result_field) = (
        caps.get(1).map_or("$", |m| m.as_str()),
        caps.get(2).map_or("", |m| m.as_str()),
        caps.get(3).map_or("", |m| m.as_str()),
        caps.get(4).map_or("", |m| m.as_str()),
        caps.get(5).map_or("", |m| m.as_str()),
    );

    // Unescape the expected value: \' -> '
    let expected = expected_raw.replace("\\'", "'");

    // Get all items at base path
    let base_norm = if base.is_empty() || base == "$" {
        "$"
    } else if base.starts_with('$') {
        base
    } else {
        &format!("$.{}", base)
    };
    let jp =
        JsonPath::try_from(format!("{}[*]", base_norm).as_str()).map_err(|e| format!("{e}"))?;
    let Value::Array(items) = jp.find(data) else {
        return Ok(vec![]);
    };

    // Filter items where nested array contains matching value
    let expected_val = Value::String(expected.to_string());
    Ok(items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let arr = obj.get(nested_arr)?.as_array()?;

            arr.iter()
                .any(|nested| nested.get(nested_field) == Some(&expected_val))
                .then(|| {
                    if result_field.is_empty() {
                        Some(item.clone())
                    } else {
                        obj.get(result_field).cloned()
                    }
                })?
        })
        .collect())
}

/// Custom evaluation of standard filters with escaped apostrophes like: items[?(@.name == 'some guy\'s item')].amount
#[allow(dead_code)]
fn evaluate_simple_filter_with_escaped_quotes(
    data: &Value,
    path: &str,
) -> Result<Vec<Value>, String> {
    use jsonpath_rust::JsonPath;

    // Parse: <base>[?(@.<field> == '<value>')][.<result>]
    // Regex handles escaped quotes: 'value with \'escaped\''
    let re = regex::Regex::new(r"^(.*?)\[\?\(@\.(.*?)\s*==\s*'((?:\\.|[^'])*?)'\)\](.*)$")
        .map_err(|e| format!("Regex error: {e}"))?;

    let caps = re.captures(path).ok_or("Not a simple filter")?;
    let (base, field, expected_raw, result_suffix) = (
        caps.get(1).map_or("$", |m| m.as_str()),
        caps.get(2).map_or("", |m| m.as_str()),
        caps.get(3).map_or("", |m| m.as_str()),
        caps.get(4).map_or("", |m| m.as_str()),
    );

    // Unescape the expected value: \' -> '
    let expected = expected_raw.replace("\\'", "'");

    // Get all items at base path
    let base_norm = if base.is_empty() || base == "$" {
        "$"
    } else if base.starts_with('$') {
        base
    } else {
        &format!("$.{}", base)
    };
    let jp =
        JsonPath::try_from(format!("{}[*]", base_norm).as_str()).map_err(|e| format!("{e}"))?;
    let Value::Array(items) = jp.find(data) else {
        return Ok(vec![]);
    };

    // Filter items where the field matches the expected value
    let expected_val = Value::String(expected);
    let results: Vec<Value> = items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            (obj.get(field) == Some(&expected_val)).then(|| item.clone())
        })
        .collect();

    // If result_suffix is provided (e.g., ".field"), extract that field from each result
    if !result_suffix.is_empty() {
        let mut final_results = Vec::new();
        for item in results {
            if let Some(obj) = item.as_object() {
                // result_suffix could be ".field" or ".[field]" etc.
                let suffix_norm = if result_suffix.starts_with('.') {
                    &result_suffix[1..]
                } else if result_suffix.starts_with('[') && result_suffix.ends_with(']') {
                    &result_suffix[1..result_suffix.len() - 1]
                } else {
                    result_suffix
                };
                if let Some(val) = obj.get(suffix_norm) {
                    final_results.push(val.clone());
                }
            }
        }
        Ok(final_results)
    } else {
        Ok(results)
    }
}

#[allow(dead_code)]
fn visit_find_paths(node: &Value, target: &Value, path: &mut String, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (k, v) in map {
                let orig_len = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(k);
                visit_find_paths(v, target, path, out);
                path.truncate(orig_len);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let orig_len = path.len();
                use std::fmt::Write as _;
                let _ = write!(path, "[{}]", i);
                visit_find_paths(v, target, path, out);
                path.truncate(orig_len);
            }
        }
        // Only compare equality on non-container (leaf) values to avoid
        // expensive deep comparisons for every object/array node.
        _ => {
            if node == target {
                if !path.is_empty() {
                    out.push(path.clone());
                }
            }
        }
    }
}

#[allow(dead_code)]
fn visit_extract_pairs(node: &Value, path: &mut String, out: &mut Vec<(String, Value)>) {
    match node {
        Value::Object(map) => {
            for (k, v) in map {
                let orig_len = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(k);
                visit_extract_pairs(v, path, out);
                path.truncate(orig_len);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let orig_len = path.len();
                use std::fmt::Write as _;
                let _ = write!(path, "[{}]", i);
                visit_extract_pairs(v, path, out);
                path.truncate(orig_len);
            }
        }
        _ => {
            out.push((path.clone(), node.clone()));
        }
    }
}

#[cfg(feature = "python")]
#[pyfunction]
fn resolve_jsonpath(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    path: &str,
) -> PyResult<Vec<Py<PyAny>>> {
    use jsonpath_rust::JsonPath;

    let v: Value =
        depythonize(data).map_err(|e| PyValueError::new_err(format!("Invalid input JSON: {e}")))?;

    let norm_path = normalize_jsonpath(path);

    // Try custom nested wildcard filter first, then simple filter, then fallback to standard JSONPath
    let matches = evaluate_nested_wildcard_filter(&v, &norm_path)
        .or_else(|_| evaluate_simple_filter_with_escaped_quotes(&v, &norm_path))
        .or_else(|_| {
            let jp = JsonPath::try_from(norm_path.as_str())
                .map_err(|e| format!("JSONPath parse error: {e}"))?;
            Ok(match jp.find(&v) {
                Value::Array(arr) => arr,
                Value::Null => vec![],
                other => vec![other],
            })
        })
        .map_err(|e: String| PyValueError::new_err(e))?;

    matches
        .iter()
        .map(|m| {
            pythonize(py, m)
                .map(|obj| obj.into())
                .map_err(|e| PyValueError::new_err(format!("Convert error: {e}")))
        })
        .collect()
}

#[cfg(feature = "python")]
#[pyfunction]
fn find_jsonpaths_by_value(
    _py: Python<'_>,
    data: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let v: Value = depythonize(data).map_err(|e: pythonize::PythonizeError| {
        PyValueError::new_err(format!("Invalid input JSON: {e}"))
    })?;
    let t: Value = depythonize(target).map_err(|e: pythonize::PythonizeError| {
        PyValueError::new_err(format!("Invalid target JSON: {e}"))
    })?;

    let mut out = Vec::new();
    let mut buf = String::new();
    visit_find_paths(&v, &t, &mut buf, &mut out);
    Ok(out)
}

#[cfg(feature = "python")]
#[pyfunction(signature = (data, path=""))]
fn extract_jsonpaths_and_values(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    path: &str,
) -> PyResult<Vec<(String, Py<PyAny>)>> {
    let v: Value = depythonize(data).map_err(|e: pythonize::PythonizeError| {
        PyValueError::new_err(format!("Invalid input JSON: {e}"))
    })?;

    let mut pairs: Vec<(String, Value)> = Vec::new();
    let mut buf = String::from(path);
    visit_extract_pairs(&v, &mut buf, &mut pairs);

    let mut out: Vec<(String, Py<PyAny>)> = Vec::with_capacity(pairs.len());
    for (p, val) in pairs {
        let py_obj = pythonize(py, &val)
            .map_err(|e| PyValueError::new_err(format!("Convert error: {e}")))?;
        out.push((p, py_obj.into()));
    }
    Ok(out)
}

#[cfg(feature = "python")]
#[pymodule]
fn jsonpath_sleuth(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(resolve_jsonpath, m)?)?;
    m.add_function(wrap_pyfunction!(find_jsonpaths_by_value, m)?)?;
    m.add_function(wrap_pyfunction!(extract_jsonpaths_and_values, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_find_paths_by_value_basic() {
        let obj = json!({
            "a": {"b": 1, "c": [1, 2]},
            "d": [ {"e": 1}, 2, 1 ]
        });
        let target = json!(1);
        let mut out = Vec::new();
        let mut buf = String::new();
        visit_find_paths(&obj, &target, &mut buf, &mut out);
        out.sort();
        let mut expected = vec![
            "a.b".to_string(),
            "a.c[0]".to_string(),
            "d[0].e".to_string(),
            "d[2]".to_string(),
        ];
        expected.sort();
        assert_eq!(out, expected);
    }

    #[test]
    fn test_find_paths_root_no_match() {
        let obj = json!({"x": 1});
        let target = obj.clone();
        let mut out = Vec::new();
        let mut buf = String::new();
        visit_find_paths(&obj, &target, &mut buf, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn test_resolve_jsonpath_titles() {
        use jsonpath_rust::JsonPath;

        let obj = json!({
            "store": {
                "book": [
                    {"category": "fiction", "title": "Sword"},
                    {"category": "fiction", "title": "Shield"}
                ]
            }
        });
        // direct jsonpath-rust (with leading $)
        let path = JsonPath::try_from("$.store.book[*].title").unwrap();
        let result = path.find(&obj);
        let got: Vec<String> = match result {
            Value::Array(arr) => arr
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),
            _ => vec![],
        };
        assert_eq!(got, vec!["Sword".to_string(), "Shield".to_string()]);

        // our normalizer should accept paths without the leading $
        let path2 = JsonPath::try_from(normalize_jsonpath("store.book[*].title").as_str()).unwrap();
        let result2 = path2.find(&obj);
        let got2: Vec<String> = match result2 {
            Value::Array(arr) => arr
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),
            _ => vec![],
        };
        assert_eq!(got2, vec!["Sword".to_string(), "Shield".to_string()]);
    }

    #[test]
    fn test_resolve_jsonpath_filter_by_title() {
        use jsonpath_rust::JsonPath;

        let obj = json!({
            "store": {
                "book": [
                    {"category": "fiction", "title": "Sword"},
                    {"category": "fiction", "title": "Shield"}
                ]
            }
        });
        // Filter selecting the book with title == 'Sword' and returning its category
        let path_no_root = "store.book[?(@.title == 'Sword')].category";
        let path = JsonPath::try_from(normalize_jsonpath(path_no_root).as_str()).unwrap();
        let result = path.find(&obj);
        let got: Vec<String> = match result {
            Value::Array(arr) => arr
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),
            _ => vec![],
        };
        assert_eq!(got, vec!["fiction".to_string()]);

        // Explicit root should behave the same
        let path2 = JsonPath::try_from("$.store.book[?(@.title == 'Sword')].category").unwrap();
        let result2 = path2.find(&obj);
        let got2: Vec<String> = match result2 {
            Value::Array(arr) => arr
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),
            _ => vec![],
        };
        assert_eq!(got2, vec!["fiction".to_string()]);
    }

    #[test]
    fn test_extract_jsonpaths_and_values_basic() {
        let obj = json!({
            "a": {"b": 1, "c": [1, 2]},
            "d": [{"e": 1}, 2, 1]
        });
        let mut out: Vec<(String, Value)> = Vec::new();
        let mut buf = String::new();
        visit_extract_pairs(&obj, &mut buf, &mut out);

        let mut out = out;
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.to_string().cmp(&b.1.to_string())));

        let mut expected: Vec<(String, Value)> = vec![
            ("a.b".into(), json!(1)),
            ("a.c[0]".into(), json!(1)),
            ("a.c[1]".into(), json!(2)),
            ("d[0].e".into(), json!(1)),
            ("d[1]".into(), json!(2)),
            ("d[2]".into(), json!(1)),
        ];
        expected.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.to_string().cmp(&b.1.to_string())));

        assert_eq!(out, expected);
    }

    #[test]
    fn test_extract_jsonpaths_and_values_scalars() {
        let obj = json!(["x", 10, true, null, 1.5]);
        let mut out: Vec<(String, Value)> = Vec::new();
        let mut buf = String::new();
        visit_extract_pairs(&obj, &mut buf, &mut out);

        let mut out = out;
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.to_string().cmp(&b.1.to_string())));

        let mut expected: Vec<(String, Value)> = vec![
            ("[0]".into(), json!("x")),
            ("[1]".into(), json!(10)),
            ("[2]".into(), json!(true)),
            ("[3]".into(), json!(null)),
            ("[4]".into(), json!(1.5)),
        ];
        expected.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.to_string().cmp(&b.1.to_string())));

        assert_eq!(out, expected);
    }

    // Restored (adapted) tests replacing the removed fast-path tests
    #[test]
    fn test_jsonpath_simple_dot_traversal() {
        use jsonpath_rust::JsonPath;

        let obj = json!({"a": {"b": {"c": 1}, "x": 2}});
        let path = JsonPath::try_from(normalize_jsonpath("a.b.c").as_str()).unwrap();
        let result = path.find(&obj);
        let matches = match &result {
            Value::Array(arr) => arr,
            _ => panic!("Expected array result"),
        };
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], json!(1));

        // missing path should yield no matches
        let path_missing = JsonPath::try_from(normalize_jsonpath("a.b.missing").as_str()).unwrap();
        let result_missing = path_missing.find(&obj);
        assert!(
            matches!(result_missing, Value::Array(ref arr) if arr.is_empty())
                || result_missing == Value::Null
        );

        // traversal stops when encountering non-object (a.x is scalar)
        let path_stop = JsonPath::try_from(normalize_jsonpath("a.x.y").as_str()).unwrap();
        let result_stop = path_stop.find(&obj);
        assert!(
            matches!(result_stop, Value::Array(ref arr) if arr.is_empty())
                || result_stop == Value::Null
        );
    }

    #[test]
    fn test_jsonpath_quoted_key_segments() {
        use jsonpath_rust::JsonPath;

        // Properly quoted segment with a space: a['some key'].next
        let obj = json!({"a": {"some key": {"next": 42}}});
        let path = JsonPath::try_from(normalize_jsonpath("a['some key'].next").as_str()).unwrap();
        let result = path.find(&obj);
        let m = match &result {
            Value::Array(arr) => arr,
            _ => panic!("Expected array result"),
        };
        assert_eq!(m.len(), 1);
        assert_eq!(m[0], json!(42));

        // Mixed characters requiring quotes
        let obj2 = json!({"a b": {"c-d_e": {"k": "v"}}});
        let path2 = JsonPath::try_from(normalize_jsonpath("['a b']['c-d_e'].k").as_str()).unwrap();
        let result2 = path2.find(&obj2);
        let m2 = match &result2 {
            Value::Array(arr) => arr,
            _ => panic!("Expected array result"),
        };
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0], json!("v"));
    }

    #[test]
    fn test_simple_filter_with_single_apostrophe() {
        // Parametrized: Test simple filter with single apostrophe
        struct Case {
            obj: Value,
            path: &'static str,
            expected_len: usize,
            expected: Vec<Value>,
        }

        let cases = vec![
            Case {
                obj: json!({
                    "items": [
                        {"name": "item with's", "value": 10},
                        {"name": "plain item", "value": 20},
                        {"name": "item with's", "value": 30},
                    ]
                }),
                path: r"$.items[?(@.name == 'item with\'s')].value",
                expected_len: 2,
                expected: vec![json!(10), json!(30)],
            },
            Case {
                obj: json!({
                    "items": [
                        {"name": "alpha's beta", "value": 50},
                        {"name": "other", "value": 60},
                    ]
                }),
                path: r"$.items[?(@.name == 'alpha\'s beta')].value",
                expected_len: 1,
                expected: vec![json!(50)],
            },
        ];

        for case in cases {
            let result = evaluate_simple_filter_with_escaped_quotes(&case.obj, case.path);
            assert!(result.is_ok(), "Failed for path: {}", case.path);
            let values = result.unwrap();
            assert_eq!(
                values.len(),
                case.expected_len,
                "Length mismatch for path: {}",
                case.path
            );
            for (i, expected_val) in case.expected.iter().enumerate() {
                assert_eq!(
                    &values[i], expected_val,
                    "Value mismatch at index {} for path: {}",
                    i, case.path
                );
            }
        }
    }

    #[test]
    fn test_nested_wildcard_with_single_apostrophe() {
        // Parametrized: Test nested wildcard filter with single apostrophe
        struct Case {
            obj: Value,
            path: &'static str,
            expected_len: usize,
            expected: Vec<Value>,
        }

        let cases = vec![
            Case {
                obj: json!({
                    "items": [
                        {"name": "item1", "results": [{"field": "value's type"}, {"field": "other"}]},
                        {"name": "item2", "results": [{"field": "value's type"}]},
                        {"name": "item3", "results": [{"field": "different"}]},
                    ]
                }),
                path: r"$.items[?(@.results[*].field=='value\'s type')].name",
                expected_len: 2,
                expected: vec![json!("item1"), json!("item2")],
            },
            Case {
                obj: json!({
                    "items": [
                        {"id": "a", "results": [{"type": "object's type"}]},
                        {"id": "b", "results": [{"type": "object's type"}]},
                    ]
                }),
                path: r"$.items[?(@.results[*].type=='object\'s type')].id",
                expected_len: 2,
                expected: vec![json!("a"), json!("b")],
            },
        ];

        for case in cases {
            let result = evaluate_nested_wildcard_filter(&case.obj, case.path);
            assert!(result.is_ok(), "Failed for path: {}", case.path);
            let values = result.unwrap();
            assert_eq!(
                values.len(),
                case.expected_len,
                "Length mismatch for path: {}",
                case.path
            );
            for (i, expected_val) in case.expected.iter().enumerate() {
                assert_eq!(
                    &values[i], expected_val,
                    "Value mismatch at index {} for path: {}",
                    i, case.path
                );
            }
        }
    }

    #[test]
    fn test_multiple_apostrophes_in_filter() {
        // Test filter with multiple apostrophes
        let obj = json!({
            "items": [
                {"name": "it's Bob's item", "value": 1},
                {"name": "it's not his", "value": 2},
                {"name": "it's Bob's item", "value": 3},
            ]
        });

        let result = evaluate_simple_filter_with_escaped_quotes(
            &obj,
            r"$.items[?(@.name == 'it\'s Bob\'s item')].value",
        );
        assert!(result.is_ok());
        let values = result.unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], json!(1));
        assert_eq!(values[1], json!(3));
    }
}
