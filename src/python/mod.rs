use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::Path;

use pyo3::exceptions::PyKeyError;
use pyo3::types::{PyBytes, PyList, PyString};
use pyo3::{
    intern, pyclass, pymethods, IntoPy, Py, PyAny, PyCell, PyErr, PyObject, PyRef, PyRefMut,
    PyResult, Python,
};
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
    headers: HeaderMap,
) -> PyResult<Option<ProcessorResponse>> {
    Python::with_gil(|py| {
        let response_opt: Option<Py<PyAny>> = processor
            .as_ref(py)
            .call_method1(
                intern!(py, "_RequestProcessor__process_head"),
                (url, method, PyCell::new(py, Headers(headers))?),
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
    headers: HeaderMap,
    body: &[u8],
) -> PyResult<Option<ProcessorResponse>> {
    Python::with_gil(|py| {
        let response_opt: Option<Py<PyAny>> = processor
            .as_ref(py)
            .call_method1(
                intern!(py, "_RequestProcessor__process"),
                (url, method, PyCell::new(py, Headers(headers))?, body),
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

#[pyclass(frozen, mapping)]
struct Headers(HeaderMap);

#[pymethods]
impl Headers {
    fn __len__(&self) -> usize {
        self.0.len()
    }

    fn __contains__(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    fn __getitem__<'a, 'b>(slf: &'a PyCell<Self>, key: &'b str) -> PyResult<&'a PyAny> {
        match slf.get().0.get(key) {
            None => Err(PyKeyError::new_err(key.to_owned())),
            Some(v) => Ok(match v.to_str() {
                Ok(v) => PyString::new(slf.py(), v).into(),
                Err(_) => PyBytes::new(slf.py(), v.as_bytes()).into(),
            }),
        }
    }

    fn get<'a, 'b>(
        slf: &'a PyCell<Self>,
        key: &'b str,
        default: Option<&'a PyAny>,
    ) -> PyResult<Option<&'a PyAny>> {
        Ok(match Self::__getitem__(slf, key) {
            Ok(v) => Some(v),
            Err(_) => default,
        }
        .into())
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<HeaderNamesIter>> {
        let iter = HeaderNamesIter(
            slf.0
                .keys()
                .map(|n| n.clone())
                .collect::<Vec<HeaderName>>()
                .into_iter(),
        );

        Py::new(slf.py(), iter)
    }

    fn items(slf: PyRef<'_, Self>) -> PyResult<Py<HeaderItemsIter>> {
        let iter = HeaderItemsIter(
            slf.0
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<(HeaderName, HeaderValue)>>()
                .into_iter(),
        );

        Py::new(slf.py(), iter)
    }

    fn getlist<'a, 'b>(slf: &'a PyCell<Self>, key: &'b str) -> &'a PyList {
        let l = PyList::empty(slf.py());
        for v in slf.get().0.get_all(key) {
            match v.to_str() {
                Ok(v) => l.append(v).unwrap(),
                Err(_) => l.append(v.as_bytes()).unwrap(),
            };
        }
        l
    }
}

#[pyclass]
struct HeaderNamesIter(std::vec::IntoIter<HeaderName>);

#[pymethods]
impl HeaderNamesIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<String> {
        slf.0.next().map(|v| v.to_string())
    }
}

#[pyclass]
struct HeaderItemsIter(std::vec::IntoIter<(HeaderName, HeaderValue)>);

#[pymethods]
impl HeaderItemsIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyObject> {
        slf.0.next().map(|(k, v)| match v.to_str() {
            Ok(v) => (k.to_string(), v).into_py(slf.py()),
            Err(_) => (k.to_string(), v.as_bytes()).into_py(slf.py()),
        })
    }
}

#[cfg(test)]
mod test;
