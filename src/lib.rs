use grep_printer::StandardBuilder;
use grep_regex::RegexMatcher;
use grep_searcher::SearcherBuilder;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use pyo3::prelude::*;
use regex::Regex;

#[pyfunction]
fn search(pattern: &str, path: &str, file_pattern: Option<&str>) -> PyResult<Vec<String>> {
    let matcher = RegexMatcher::new(pattern)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    let mut builder = SearcherBuilder::new();
    let builder_with_context = builder
        .after_context(2)
        .before_context(2);

    let mut searcher = builder_with_context.build();

    let mut matches = Vec::new();

    let mut pbuilder = StandardBuilder::new();
    let printer_tpl = pbuilder
        .heading(true)
        .separator_field_context(b"|".to_vec())
        .separator_field_match(b"|".to_vec())
        .separator_context(Some(b"\n...\n".to_vec()));


    let mut override_builder = OverrideBuilder::new(path);
    if let Some(file_pattern) = file_pattern {
        let _ = override_builder.add(&file_pattern);
    }
    let overrides = override_builder.build().unwrap();

    let mut walker_builder = WalkBuilder::new(path);
    let walker = walker_builder.overrides(overrides).build();

    for result in walker {
        let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("Error: {}", err);
                continue;
            }
        };

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let stripped_path = entry.path().strip_prefix(path).unwrap_or_else(|_| entry.path());
        let mut printer = printer_tpl.clone().build_no_color(vec![]);
        let mut sink = printer.sink_with_path(&matcher, stripped_path);

        if let Err(err) = searcher.search_path(&matcher, entry.path(), &mut sink) {
            eprintln!("Error searching {}: {}", entry.path().display(), err);
        } else {
            let result = printer.into_inner().into_inner();

            if !result.is_empty() {
                matches.push(String::from_utf8(result).unwrap());
            }
        }
    }

    Ok(matches)
}

#[pyfunction]
fn find_files(pattern: &str, path: &str, ignore_extra: Vec<String>) -> PyResult<Vec<String>> {
    let regex = Regex::new(pattern)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    let mut matches = Vec::new();

    let mut override_builder = OverrideBuilder::new(path);
    for glob in ignore_extra {
        let _ = override_builder.add(&format!("!{}", glob));
    }
    let overrides = override_builder.build().unwrap();

    let mut builder = WalkBuilder::new(path);
    let walker = builder.overrides(overrides).build();
    for result in walker {
        let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("Error: {}", err);
                continue;
            }
        };

        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            let file_name = entry.file_name().to_string_lossy();
            if regex.is_match(&file_name) {
                let stripped_path = entry.path().strip_prefix(path).unwrap_or_else(|_| entry.path());
                matches.push(stripped_path.display().to_string());
            }
        }
    }

    Ok(matches)
}

#[pymodule]
fn python_ripgrep(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(search, m)?)?;
    m.add_function(wrap_pyfunction!(find_files, m)?)?;
    Ok(())
}
