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
