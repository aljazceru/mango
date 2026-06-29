#!/usr/bin/env python3
import argparse
import json
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


LOCAL_TOOL_NAMES = {
    "search_documents",
    "read_document",
    "finish",
    "web_search",
    "fetch_url",
    "file",
    "calculate",
}


class Handler(BaseHTTPRequestHandler):
    server_version = "MangoSmokeOpenAI/1.0"

    def log_message(self, fmt: str, *args: object) -> None:
        print(f"[mock-openai] {self.address_string()} {fmt % args}", flush=True)

    @property
    def model(self) -> str:
        return self.server.model  # type: ignore[attr-defined]

    @property
    def chunk_delay(self) -> float:
        return self.server.chunk_delay  # type: ignore[attr-defined]

    def write_json(self, payload: dict, status: int = 200) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path.rstrip("/") == "/v1/models":
            self.write_json(
                {
                    "object": "list",
                    "data": [
                        {
                            "id": self.model,
                            "object": "model",
                            "created": 0,
                            "owned_by": "mango-smoke",
                        }
                    ],
                }
            )
            return
        self.write_json({"error": {"message": "not found"}}, status=404)

    def do_POST(self) -> None:
        if self.path.rstrip("/") != "/v1/chat/completions":
            self.write_json({"error": {"message": "not found"}}, status=404)
            return

        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode("utf-8")
        request = json.loads(body or "{}")
        messages = request.get("messages", [])
        tools = request.get("tools", [])
        stream = request.get("stream") is True

        if stream:
            self.write_streaming_response(messages)
        elif tools:
            self.write_tool_call_response(tools)
        else:
            self.write_json(self.completion("ok"))

    def completion(self, content: str) -> dict:
        return {
            "id": "chatcmpl-mango-smoke",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": self.model,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": content},
                    "finish_reason": "stop",
                }
            ],
        }

    def write_tool_call_response(self, tools: list[dict]) -> None:
        tool_names = [
            tool.get("function", {}).get("name")
            for tool in tools
            if tool.get("function", {}).get("name")
        ]
        print(f"[mock-openai] advertised tools: {', '.join(tool_names)}", flush=True)

        tool_name = None
        for tool in tools:
            name = tool.get("function", {}).get("name")
            if name and name not in LOCAL_TOOL_NAMES:
                tool_name = name
                break
        if tool_name is None:
            self.write_json(
                self.completion(
                    "No ContextVM tool was advertised. "
                    f"Advertised tools: {', '.join(tool_names) or '(none)'}"
                )
            )
            return

        self.write_json(
            {
                "id": "chatcmpl-mango-smoke-tool",
                "object": "chat.completion",
                "created": int(time.time()),
                "model": self.model,
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": None,
                            "tool_calls": [
                                {
                                    "id": "call_mango_smoke_echo",
                                    "type": "function",
                                    "function": {
                                        "name": tool_name,
                                        "arguments": json.dumps(
                                            {"message": "hello from smoke test"}
                                        ),
                                    },
                                }
                            ],
                        },
                        "finish_reason": "tool_calls",
                    }
                ],
            }
        )

    def write_streaming_response(self, messages: list[dict]) -> None:
        text = "Echo: hello from smoke test"
        for message in reversed(messages):
            if message.get("role") == "tool" and message.get("content"):
                text = str(message["content"])
                break

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()

        chunks = [
            {"role": "assistant"},
            {"content": text},
            {},
        ]
        for i, delta in enumerate(chunks):
            if self.chunk_delay:
                time.sleep(self.chunk_delay)
            finish_reason = "stop" if i == len(chunks) - 1 else None
            payload = {
                "id": "chatcmpl-mango-smoke-stream",
                "object": "chat.completion.chunk",
                "created": int(time.time()),
                "model": self.model,
                "choices": [
                    {"index": 0, "delta": delta, "finish_reason": finish_reason}
                ],
            }
            self.wfile.write(f"data: {json.dumps(payload)}\n\n".encode("utf-8"))
            self.wfile.flush()
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--model", default="mango-smoke-model")
    parser.add_argument("--chunk-delay", type=float, default=0.0)
    args = parser.parse_args()

    server = ThreadingHTTPServer((args.host, args.port), Handler)
    server.model = args.model  # type: ignore[attr-defined]
    server.chunk_delay = args.chunk_delay  # type: ignore[attr-defined]
    print(f"Mock OpenAI provider: http://{args.host}:{server.server_port}/v1", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
