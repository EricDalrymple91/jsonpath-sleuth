#[cfg(feature = "python")]
use pyo3::exceptions::PyValueError;
#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyModule;
#[cfg(feature = "python")]
use pythonize::{depythonize, pythonize};
use serde_json::Value;

fn normalize_jsonpath(path: &str) -> String {
    let p = path.trim();
    match p.chars().next() {
        None => "$.".to_owned(),
        Some('$') => p.to_owned(),
        Some('.') | Some('[') => format!("${}", p),
        _ => format!("$.{}", p),
    }
}

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
fn resolve_jsonpath(py: Python<'_>, data: &Bound<'_, PyAny>, path: &str) -> PyResult<Vec<Py<PyAny>>> {
    let v: Value = depythonize(data)
        .map_err(|e: pythonize::PythonizeError| PyValueError::new_err(format!("Invalid input JSON: {e}")))?;

    let path = normalize_jsonpath(path);
    let matches = jsonpath_lib::select(&v, &path)
        .map_err(|e| PyValueError::new_err(format!("JSONPath error: {e}")))?;

    let mut out: Vec<Py<PyAny>> = Vec::with_capacity(matches.len());
    for m in matches {
        let py_obj = pythonize(py, m)
            .map_err(|e| PyValueError::new_err(format!("Convert error: {e}")))?;
        out.push(py_obj.into());
    }
    Ok(out)
}

#[cfg(feature = "python")]
#[pyfunction]
fn find_jsonpaths_by_value(
    _py: Python<'_>,
    data: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let v: Value = depythonize(data)
        .map_err(|e: pythonize::PythonizeError| PyValueError::new_err(format!("Invalid input JSON: {e}")))?;
    let t: Value = depythonize(target)
        .map_err(|e: pythonize::PythonizeError| PyValueError::new_err(format!("Invalid target JSON: {e}")))?;

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
    let v: Value = depythonize(data)
        .map_err(|e: pythonize::PythonizeError| PyValueError::new_err(format!("Invalid input JSON: {e}")))?;

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
        let obj = json!({
            "store": {
                "book": [
                    {"category": "fiction", "title": "Sword"},
                    {"category": "fiction", "title": "Shield"}
                ]
            }
        });
        // direct jsonpath_lib (with leading $)
        let matches = jsonpath_lib::select(&obj, "$.store.book[*].title").unwrap();
        let got: Vec<String> = matches
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(got, vec!["Sword".to_string(), "Shield".to_string()]);

        // our normalizer should accept paths without the leading $
        let m2 = jsonpath_lib::select(&obj, &normalize_jsonpath("store.book[*].title")).unwrap();
        let got2: Vec<String> = m2.iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(got2, vec!["Sword".to_string(), "Shield".to_string()]);
    }

    #[test]
    fn test_resolve_jsonpath_filter_by_title() {
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
        let m = jsonpath_lib::select(&obj, &normalize_jsonpath(path_no_root)).unwrap();
        let got: Vec<String> = m.iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(got, vec!["fiction".to_string()]);

        // Explicit root should behave the same
        let m2 = jsonpath_lib::select(&obj, "$.store.book[?(@.title == 'Sword')].category").unwrap();
        let got2: Vec<String> = m2.iter().map(|v| v.as_str().unwrap().to_string()).collect();
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
        let obj = json!({"a": {"b": {"c": 1}, "x": 2}});
        let matches = jsonpath_lib::select(&obj, &normalize_jsonpath("a.b.c")).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], &json!(1));

        // missing path should yield no matches
        let missing = jsonpath_lib::select(&obj, &normalize_jsonpath("a.b.missing")).unwrap();
        assert!(missing.is_empty());

        // traversal stops when encountering non-object (a.x is scalar)
        let stop = jsonpath_lib::select(&obj, &normalize_jsonpath("a.x.y")).unwrap();
        assert!(stop.is_empty());
    }

    #[test]
    fn test_jsonpath_quoted_key_segments() {
        // Properly quoted segment with a space: a['some key'].next
        let obj = json!({"a": {"some key": {"next": 42}}});
        let m = jsonpath_lib::select(&obj, &normalize_jsonpath("a['some key'].next")).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0], &json!(42));

        // Mixed characters requiring quotes
        let obj2 = json!({"a b": {"c-d_e": {"k": "v"}}});
        let m2 = jsonpath_lib::select(&obj2, &normalize_jsonpath("['a b']['c-d_e'].k")).unwrap();
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0], &json!("v"));
    }
}
