use pyo3::prelude::*;

mod ripgrep_core;
use ripgrep_core::{py_search, py_files, PyArgs, PySortMode, PySortModeKind};

#[pymodule]
fn python_ripgrep(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyArgs>()?;
    m.add_class::<PySortMode>()?;
    m.add_class::<PySortModeKind>()?;
    m.add_function(wrap_pyfunction!(py_search, m)?)?;
    m.add_function(wrap_pyfunction!(py_files, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        Python::with_gil(|py| {
            let result = hello_world().unwrap();
            assert_eq!(result, "Hello, world!");
        });
    }
}