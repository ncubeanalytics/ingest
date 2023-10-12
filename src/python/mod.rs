use std::path::Path;

use pyo3::types::PyList;
use pyo3::{intern, Py, PyAny, PyErr, PyResult, Python};
use tracing::info;

pub fn init_python(plugin_src_dir: &str) -> PyResult<()> {
    info!("Initializing python interpreter...");
    pyo3::prepare_freethreaded_python();
    // choosing to import at runtime instead of pulling in python code in the binary
    // https://pyo3.rs/v0.18.3/python_from_rust#include-multiple-python-files
    let pymodule_path = Path::new(plugin_src_dir);
    Python::with_gil(|py| {
        let syspath: &PyList = py.import("sys")?.getattr("path")?.extract()?;
        syspath.insert(0, &pymodule_path)?;
        Ok(())
    })
}

pub fn import_and_call_callable(module: &str, callable: &str) -> PyResult<Py<PyAny>> {
    Python::with_gil(|py| {
        let module = py.import(module)?;
        let processor_instance = module.getattr(callable)?.call0()?;
        Ok(processor_instance.extract()?)
    })
}

#[derive(Debug)]
// XXX: expose this struct directly to python instead of converting
pub struct ProcessorResponse {
    pub forward: bool,
    pub response_status: Option<u16>,
    pub response_headers: Option<Vec<(String, String)>>,
    pub response_body: Option<Vec<u8>>,
}

fn marshal_response(py: Python, response: Py<PyAny>) -> PyResult<ProcessorResponse> {
    let forward: bool = response.getattr(py, intern!(py, "forward"))?.extract(py)?;
    let response_status: Option<u16> = response
        .getattr(py, intern!(py, "status_code"))?
        .extract(py)?;
    let response_headers: Option<Vec<(String, String)>> =
        response.getattr(py, intern!(py, "headers"))?.extract(py)?;
    let response_body: Option<Vec<u8>> = response.getattr(py, intern!(py, "body"))?.extract(py)?;
    Ok(ProcessorResponse {
        forward,
        response_status,
        response_headers,
        response_body,
    })
}

pub fn call_processor_process_head(
    processor: &Py<PyAny>,
    url: &str,
    method: &str,
    headers: &[(&str, &str)],
) -> PyResult<Option<ProcessorResponse>> {
    Python::with_gil(|py| {
        let response_opt: Option<Py<PyAny>> = processor
            .as_ref(py)
            .call_method1(
                intern!(py, "_RequestProcessor__process_head"),
                (url, method, headers.to_vec()),
            )?
            .extract()?;
        if let Some(response) = response_opt {
            Ok(Some(marshal_response(py, response)?))
        } else {
            Ok(None)
        }
    })
}

pub fn call_processor_process(
    processor: &Py<PyAny>,
    url: &str,
    method: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> PyResult<Option<ProcessorResponse>> {
    Python::with_gil(|py| {
        let response_opt: Option<Py<PyAny>> = processor
            .as_ref(py)
            .call_method1(
                intern!(py, "_RequestProcessor__process"),
                (url, method, headers.to_vec(), body),
            )?
            .extract()?;
        if let Some(response) = response_opt {
            Ok(Some(marshal_response(py, response)?))
        } else {
            Ok(None)
        }
    })
}

pub fn pyerror_with_traceback_string(e: &PyErr) -> String {
    Python::with_gil(|py| {
        format!(
            "{}{}",
            e.traceback(py)
                .map(|t| t.format().ok())
                .flatten()
                .unwrap_or("".to_owned()),
            e
        )
    })
}

#[cfg(test)]
mod test;
