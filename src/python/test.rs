use actix_web::http::header::HeaderMap;
use pyo3::types::PyModule;
use pyo3::{Py, PyAny, PyResult, Python};

use crate::python::{call_processor_process, pyerror_with_traceback_string, ProcessorResponse};

use super::init_python;

// language=python
const NOOP_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class NoopProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=False,
            status_code=None,
            headers=None,
            body=None
        )

"#;

// language=python
const NONE_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class NoneProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return

"#;

// language=python
const STATIC_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class StaticProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=False,
            status_code=201,
            headers=[('a', 'b'), ('c', 'd')],
            body=b'body'
        )

"#;

// language=python
const ARG_USING_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class ArgUsingProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=200,
            headers=[('url', url), ('method', method)] + list(headers.items()),
            body=body
        )

"#;

// language=python
const HEADERS_GETLIST_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeaderGetListProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        vals = headers.getlist("x-tEsT")
        return Response(
            forward=True,
            status_code=200,
            headers=[(f"x-test-{i}", v) for i, v in enumerate(vals)],
            body=b""
        )

"#;

// language=python
const HEADERS_GETITEM_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeaderGetitemProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        val = headers["x-tEsT"]
        return Response(
            forward=True,
            status_code=200,
            headers=[(f"x-test", val)],
            body=b""
        )

"#;

// language=python
const HEADERS_GET_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeaderGetProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        val = headers.get("x-tEsT", "notfound")
        return Response(
            forward=True,
            status_code=200,
            headers=[(f"x-test", val)],
            body=b""
        )

"#;

// language=python
const HEADERS_CONTAIN_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeadersContainProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        val = "x-tEsT" in headers
        return Response(
            forward=True,
            status_code=200,
            headers=[(f"x-test", str(val))],
            body=b""
        )

"#;

// language=python
const HEADERS_LENGTH_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeadersLengthProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        val = len(headers)
        return Response(
            forward=True,
            status_code=200,
            headers=[(f"x-test", str(val))],
            body=b""
        )

"#;

// language=python
const HEADERS_ITER_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeadersIterProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=200,
            headers=[(k, headers[k]) for k in headers],
            body=b""
        )

"#;

// language=python
const HEADERS_ITEMS_PROCESSOR: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class HeadersItemsProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=200,
            headers=list(headers.items()),
            body=b""
        )

"#;

// language=python
const FAILING_PROCESSOR_ABC: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor

class FailingAbcProcessor(RequestProcessor):
    pass

"#;

// language=python
const FAILING_PROCESSOR_INIT: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class FailingInitProcessor(RequestProcessor):
    def __init__(self):
        self.init_fails()
    def init_fails(self):
        raise Exception("init fail")
    def process(self, url, method, headers, body):
        return

"#;

// language=python
const FAILING_PROCESSOR_PROCESS: &'static str = r#"
from ncube_ingest_plugin import RequestProcessor, Response

class FailingProcessProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return self.fails()
    def fails(self):
        raise Exception("fail")

"#;

fn instantiate_processor(code: &str, name: &str) -> PyResult<Py<PyAny>> {
    init_python(&(env!("CARGO_MANIFEST_DIR").to_owned() + "/src/python"))?;
    Python::with_gil(|py| {
        let module = PyModule::from_code(py, code, "", "")?;
        let cls = module.getattr(name)?;
        let obj = cls.call0()?;
        Ok(obj.extract()?)
    })
}

fn process(
    code: &str,
    name: &str,
    url: &str,
    method: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> PyResult<Option<ProcessorResponse>> {
    let processor = instantiate_processor(code, name)?;
    let mut h = HeaderMap::with_capacity(headers.len());
    for (k, v) in headers {
        h.append(k.parse().unwrap(), v.parse().unwrap());
    }
    call_processor_process(&processor, url, method, h, body)
}

fn _sort<T>(v: Option<Vec<T>>) -> Option<Vec<T>>
where
    T: Ord,
{
    v.map(|mut v| {
        v.sort();
        v
    })
}

fn assert_default_response(forward: bool, status_code_opt: Option<u16>, body_opt: Option<Vec<u8>>) {
    assert_eq!(forward, true);
    assert_eq!(status_code_opt, Some(200));
    assert_eq!(body_opt, Some("".to_owned().into_bytes()));
}

#[test]
fn test_processor_noop() {
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        NOOP_PROCESSOR,
        "NoopProcessor",
        "",
        "",
        vec![].as_slice(),
        vec![].as_slice(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(forward, false);
    assert_eq!(status_code_opt, None);
    assert_eq!(headers_opt, None);
    assert_eq!(body_opt, None);
}

#[test]
fn test_processor_none() {
    let result = process(
        NONE_PROCESSOR,
        "NoneProcessor",
        "",
        "",
        vec![].as_slice(),
        vec![].as_slice(),
    )
    .unwrap();

    assert_eq!(result.is_none(), true);
}

#[test]
fn test_processor_static() {
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        STATIC_PROCESSOR,
        "StaticProcessor",
        "",
        "",
        vec![].as_slice(),
        vec![].as_slice(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(forward, false);
    assert_eq!(status_code_opt, Some(201));
    assert_eq!(
        headers_opt,
        Some(vec![
            ("a".to_owned(), "b".to_owned()),
            ("c".to_owned(), "d".to_owned())
        ])
    );
    assert_eq!(body_opt, Some("body".to_owned().into_bytes()));
}

#[test]
fn test_processor_arg_using() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        ARG_USING_PROCESSOR,
        "ArgUsingProcessor",
        url,
        method,
        vec![("A", "B"), ("C", "D")].as_slice(),
        "body_arg_using".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(forward, true);
    assert_eq!(status_code_opt, Some(200));
    assert_eq!(
        _sort(headers_opt),
        _sort(Some(vec![
            ("url".to_owned(), url.to_owned()),
            ("method".to_owned(), method.to_owned()),
            ("a".to_owned(), "B".to_owned()),
            ("c".to_owned(), "D".to_owned()),
        ]))
    );
    assert_eq!(body_opt, Some("body_arg_using".to_owned().into_bytes()));
}

#[test]
fn test_processor_headers_getlist() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_GETLIST_PROCESSOR,
        "HeaderGetListProcessor",
        url,
        method,
        vec![("X-Test", "A"), ("X-Test", "B"), ("X-Test", "C")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        headers_opt,
        Some(vec![
            ("x-test-0".to_owned(), "A".to_owned()),
            ("x-test-1".to_owned(), "B".to_owned()),
            ("x-test-2".to_owned(), "C".to_owned()),
        ])
    );
}

#[test]
fn test_processor_headers_getitem() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_GETITEM_PROCESSOR,
        "HeaderGetitemProcessor",
        url,
        method,
        vec![("X-Test", "Q"), ("X-Test", "W")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        headers_opt,
        Some(vec![("x-test".to_owned(), "Q".to_owned())])
    );
}

#[test]
fn test_processor_headers_get() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_GET_PROCESSOR,
        "HeaderGetProcessor",
        url,
        method,
        vec![("X-Test", "Q"), ("X-Test", "W")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        headers_opt,
        Some(vec![("x-test".to_owned(), "Q".to_owned())])
    );
}

#[test]
fn test_processor_headers_get_default() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_GET_PROCESSOR,
        "HeaderGetProcessor",
        url,
        method,
        vec![].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        headers_opt,
        Some(vec![("x-test".to_owned(), "notfound".to_owned())])
    );
}

#[test]
fn test_processor_headers_contain() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_CONTAIN_PROCESSOR,
        "HeadersContainProcessor",
        url,
        method,
        vec![("X-Test", "Q")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        headers_opt,
        Some(vec![("x-test".to_owned(), "True".to_owned())])
    );
}

#[test]
fn test_processor_headers_length() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_LENGTH_PROCESSOR,
        "HeadersLengthProcessor",
        url,
        method,
        vec![("X-Test", "Q"), ("X-TeSt", "W"), ("X-TeSt-2", "E")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        headers_opt,
        Some(vec![("x-test".to_owned(), "3".to_owned())])
    );
}

#[test]
fn test_processor_headers_iter() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_ITER_PROCESSOR,
        "HeadersIterProcessor",
        url,
        method,
        vec![("X-Test", "Q"), ("X-TeSt", "W"), ("X-TeSt-2", "E")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        _sort(headers_opt),
        _sort(Some(vec![
            ("x-test".to_owned(), "Q".to_owned()),
            ("x-test-2".to_owned(), "E".to_owned()),
        ]))
    );
}

#[test]
fn test_processor_headers_items() {
    let url = "https://example.com";
    let method = "POST";
    let ProcessorResponse {
        forward,
        response_status: status_code_opt,
        response_headers: headers_opt,
        response_body: body_opt,
    } = process(
        HEADERS_ITEMS_PROCESSOR,
        "HeadersItemsProcessor",
        url,
        method,
        vec![("X-Test", "Q"), ("X-TeSt", "W"), ("X-TeSt-2", "E")].as_slice(),
        "".as_bytes(),
    )
    .unwrap()
    .unwrap();

    assert_default_response(forward, status_code_opt, body_opt);
    assert_eq!(
        _sort(headers_opt),
        _sort(Some(vec![
            ("x-test".to_owned(), "Q".to_owned()),
            ("x-test".to_owned(), "W".to_owned()),
            ("x-test-2".to_owned(), "E".to_owned()),
        ]))
    );
}

#[test]
fn test_processor_failing_init() {
    let err = instantiate_processor(FAILING_PROCESSOR_INIT, "FailingInitProcessor").unwrap_err();

    let expected_err_str = r#"
Traceback (most recent call last):
  File "", line 6, in __init__
  File "", line 8, in init_fails
Exception: init fail
"#
    .trim()
    .to_owned();

    assert_eq!(pyerror_with_traceback_string(&err), expected_err_str);
}

#[test]
fn test_processor_failing_abc() {
    let err = instantiate_processor(FAILING_PROCESSOR_ABC, "FailingAbcProcessor").unwrap_err();

    let expected_err_str = r#"
TypeError: Can't instantiate abstract class FailingAbcProcessor with abstract method process
"#
    .trim()
    .to_owned();

    assert_eq!(pyerror_with_traceback_string(&err), expected_err_str);
}

#[test]
fn test_processor_failing_process() {
    let err = process(
        FAILING_PROCESSOR_PROCESS,
        "FailingProcessProcessor",
        "",
        "",
        vec![].as_slice(),
        vec![].as_slice(),
    )
    .unwrap_err();

    let expected_err_str = format!(
        r#"
Traceback (most recent call last):
  File "{}/src/python/ncube_ingest_plugin/request_processor.py", line 31, in __process
    return self.process(url, method, headers, body)
  File "", line 6, in process
  File "", line 8, in fails
Exception: fail
"#,
        env!("CARGO_MANIFEST_DIR")
    )
    .trim()
    .to_owned();

    assert_eq!(pyerror_with_traceback_string(&err), expected_err_str);
}
