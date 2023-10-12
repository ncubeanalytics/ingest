from ncube_ingest_plugin import RequestProcessor, Response


class StaticProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=418,
            headers=[("a", "b"), ("c", "d")],
            body=b"body",
        )


class FailingProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        raise Exception("fail")

class FailingHeadProcessor(RequestProcessor):
    def process_head(self, url, method, headers):
        raise Exception("head fail")
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=418,
            headers=[("a", "b"), ("c", "d")],
            body=b"body",
        )

class HeadOnlyProcessor(RequestProcessor):
    def process_head(self, url, method, headers):
        return Response(
            forward=True,
            status_code=418,
            headers=[("q", "w"), ("e", "r")],
            body=b"head",
        )
    def process(self, url, method, headers, body):
        raise Exception("fail")

class HeadNoopProcessor(RequestProcessor):
    def process_head(self, url, method, headers):
        return
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=418,
            headers=[("a", "s"), ("d", "f")],
            body=b"head empty",
        )

class NoopProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return


class BodyLengthInResponseHeaderProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        return Response(
            forward=True,
            status_code=None,
            headers=[("python-body-length", str(len(body)))],
            body=None,
        )


class MirrorGetProcessor(RequestProcessor):
    def process(self, url, method, headers, body):
        if method == "GET":
            return Response(
                forward=True,
                status_code=418,
                headers=headers,
                body=body,
            )
        return Response(forward=True, status_code=200)
