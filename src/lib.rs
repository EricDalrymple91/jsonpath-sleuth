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
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "$.".to_string();
    }
    if trimmed.starts_with('$') {
        trimmed.to_string()
    } else if trimmed.starts_with('.') || trimmed.starts_with('[') {
        let mut s = String::from("$");
        s.push_str(trimmed);
        s
    } else {
        let mut s = String::from("$.");
        s.push_str(trimmed);
        s
    }
}

fn visit_find_paths(node: &Value, target: &Value, path: &mut String, out: &mut Vec<String>) {
    if node == target {
        if !path.is_empty() {
            out.push(path.clone());
        }
    }
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
        _ => {}
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
#[pymodule]
fn jsonpath_sleuth(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(resolve_jsonpath, m)?)?;
    m.add_function(wrap_pyfunction!(find_jsonpaths_by_value, m)?)?;
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
        let m2 = jsonpath_lib::select(&obj, &super::normalize_jsonpath("store.book[*].title")).unwrap();
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
        let m = jsonpath_lib::select(&obj, &super::normalize_jsonpath(path_no_root)).unwrap();
        let got: Vec<String> = m.iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(got, vec!["fiction".to_string()]);

        // Explicit root should behave the same
        let m2 = jsonpath_lib::select(&obj, "$.store.book[?(@.title == 'Sword')].category").unwrap();
        let got2: Vec<String> = m2.iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(got2, vec!["fiction".to_string()]);
    }
}
