#!/usr/bin/env python3
"""Small standard-library client, not a generated SDK. See docs/API.md."""
import argparse
import hashlib
import json
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4


class ProblemError(Exception):
    def __init__(self, status, problem):
        self.status = status
        self.code = problem.get("code", "http_error")
        super().__init__(f"HTTP {status} {self.code}: {problem.get('message', 'Request failed')}")


class Webtool:
    def __init__(self, server="http://127.0.0.1:8420"):
        self.server = server.rstrip("/")

    def request(self, method, path, body=None, content_type=None, *, raw=False):
        headers = {"Content-Type": content_type} if content_type else {}
        request = Request(self.server + path, data=body, headers=headers, method=method)
        try:
            with urlopen(request, timeout=150) as response:
                data = response.read()
        except HTTPError as error:
            with error:
                # The server's Problem messages are bounded. Do not print a proxy HTML body.
                try:
                    problem = json.loads(error.read(8192))
                    if not isinstance(problem, dict):
                        raise ValueError("not an object")
                except (ValueError, UnicodeError):
                    problem = {"code": "http_error", "message": "Non-Problem error response"}
            raise ProblemError(error.code, problem) from None
        return data if raw else json.loads(data)

    def read(self, url):
        return self.request("POST", "/v1/read", json.dumps({"url": url}).encode(), "application/json")

    def ingest(self, file):
        file = Path(file)
        # Supply the filename in the text part, not an interpolated MIME header.
        boundary = "webtool-" + uuid4().hex
        body = (
            f'--{boundary}\r\nContent-Disposition: form-data; name="name"\r\n\r\n'.encode()
            + file.name.encode()
            + f'\r\n--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="upload.bin"\r\n'
              'Content-Type: application/octet-stream\r\n\r\n'.encode()
            + file.read_bytes()
            + f'\r\n--{boundary}--\r\n'.encode()
        )
        return self.request("POST", "/v1/ingest", body, f"multipart/form-data; boundary={boundary}")

    def document(self, document_id):
        return self.request("GET", "/v1/documents/" + quote(document_id, safe=""))

    def original(self, document_id):
        return self.request("GET", "/v1/documents/" + quote(document_id, safe="") + "/original", raw=True)


def main():
    parser = argparse.ArgumentParser(description="Read or upload, then verify the saved document and original.")
    parser.add_argument("--server", default="http://127.0.0.1:8420")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--url")
    source.add_argument("--file", type=Path)
    args = parser.parse_args()
    client = Webtool(args.server)
    try:
        document = client.read(args.url)["document"] if args.url else client.ingest(args.file)
        saved = client.document(document["id"])
        original = client.original(saved["id"])
        artifact = saved["source"]["original"]
        if saved != document or len(original) != artifact["size"] or hashlib.sha256(original).hexdigest() != artifact["sha256"]:
            raise ValueError("Saved document or original verification failed")
        print(json.dumps({"id": saved["id"], "title": saved["title"], "blocks": len(saved["blocks"]),
                          "warnings": saved["warnings"], "original_verified": True}, ensure_ascii=True))
    except (ProblemError, OSError, ValueError, KeyError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
