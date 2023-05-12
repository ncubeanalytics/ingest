from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Optional


@dataclass
class Response:
    forward: bool = True
    status_code: int = None
    headers: list[tuple[str, str]] = None
    body: bytes = None


class RequestProcessor(ABC):
    @abstractmethod
    def process(
        self, url: str, method: str, headers: list[tuple[str, str]], body: bytes
    ) -> Optional[Response]:
        raise NotImplementedError

    def __process(
        self, url: str, method: str, headers: list[tuple[str, str]], body: bytes
    ) -> Optional[Response]:
        # TODO: replace with multiprocessing
        return self.process(url, method, headers, body)
