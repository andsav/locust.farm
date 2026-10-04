"""Original scripted loopback provider; never delegates to a real model.

Only tool names actually declared by a client are selected. The sequence is an
authored test plan, not a general model emulator or an assertion of readiness.
"""

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import time
from urllib.parse import urlsplit


def planned_call(step):
    """Resolve authored arguments at request time, after earlier daemon writes.

    An emitted call is never proof of completion. The qualification driver must
    independently inspect bridge receipts and durable daemon state.
    """
    if isinstance(step, str):
        return step, {}
    if not isinstance(step, dict) or set(step) != {"tool", "arguments"}:
        raise ValueError("A scripted step requires tool and arguments")
    name = step["tool"]
    arguments = step["arguments"]() if callable(step["arguments"]) else step["arguments"]
    if not isinstance(name, str) or not name or not isinstance(arguments, dict):
        raise ValueError("Invalid scripted tool name or arguments")
    # Copy a JSON value so a later fixture mutation cannot change an issued call.
    return name, json.loads(json.dumps(arguments, allow_nan=False))


class Provider:
    def __init__(self, plan):
        self.plan = list(plan)
        self.index = 0
        self.requests = []
        self.backend_requests = []
        self.errors = []
        self.lock = threading.Lock()
        provider = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def backend(self, template, status, purpose):
                with provider.lock:
                    provider.backend_requests.append({"method": self.command, "path_template": template,
                                                      "status": status, "purpose": purpose})

            def do_CONNECT(self):
                self.backend("<authority>", 403, "external network prohibited; no forwarding")
                self.send_error(403, "External network prohibited")

            def unsupported(self):
                self.backend("/<unhandled>", 403, "unsupported endpoint; no forwarding")
                self.send_error(403, "Unsupported endpoint; no forwarding")

            def __getattr__(self, name):
                if name.startswith("do_"):
                    return self.unsupported
                raise AttributeError(name)

            def do_GET(self):
                path = urlsplit(self.path).path
                if path.startswith("/api/sessions/") and len(path.split("/")) == 4 and path.split("/")[-1]:
                    self.backend("/api/sessions/<id>", 404, "scripted local-session fallback")
                    self.send_error(404, "Scripted session lookup: use local session")
                elif path.endswith("/models"):
                    self.backend("/<prefix>/models", 200, "scripted model listing")
                    self.reply({"object": "list", "data": [{"id": "locust-fixture", "object": "model"}]})
                else:
                    self.backend("/<unhandled>", 404, "unsupported endpoint; no forwarding")
                    self.send_error(404)

            def do_POST(self):
                try:
                    self.path = urlsplit(self.path).path
                    if not self.path.endswith(("/responses", "/messages", "/chat/completions", "/count_tokens")):
                        self.unsupported()
                        return
                    endpoint = next(name for name in ("responses", "messages", "chat/completions", "count_tokens")
                                    if self.path.endswith("/" + name))
                    self.path = "/" + endpoint
                    body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
                    if self.path.endswith("/count_tokens"):
                        self.backend("/<prefix>/count_tokens", 200, "scripted token count")
                        self.reply({"input_tokens": 32})
                        return
                    tools = body.get("tools", [])
                    names = [tool.get("name") or tool.get("function", {}).get("name") for tool in tools]
                    with provider.lock:
                        requested, authored_args = planned_call(provider.plan[provider.index]) if provider.index < len(provider.plan) else (None, {})
                        actual = next((name for name in names if name and requested and name.endswith(requested)), None)
                        args = authored_args
                        namespace = None
                        discovery = False
                        if requested and actual is None:
                            for tool in tools:
                                if tool.get("type") == "namespace":
                                    nested = next((entry for entry in tool.get("tools", []) if entry.get("name") == requested), None)
                                    if nested:
                                        actual, namespace = requested, tool["name"]
                                        break
                        if requested and requested.startswith("locust_") and actual is None and "codemode" in names:
                            actual = "codemode"
                            args = {"code": f"text(await tools.mcp__locust__{requested}({json.dumps(authored_args)}));"}
                        if requested and actual is None and "ToolSearch" in names:
                            actual, discovery = "ToolSearch", True
                            args = {"query": "locust " + requested}
                        provider.requests.append({"path": self.path, "declared_tools": names,
                                                  "tool_definitions": tools,
                                                  "requested_tool": requested, "selected_tool": actual,
                                                  "namespace": namespace, "discovery": discovery})
                        if requested and actual is None:
                            provider.errors.append(f"Tool not declared: {requested}")
                        if actual and not discovery:
                            provider.index += 1
                        number = len(provider.requests)
                    call_id = f"call_fixture_{number}"
                    if self.path.endswith("/responses"):
                        self.openai_responses(body, actual, args, call_id, namespace)
                    elif self.path.endswith("/messages"):
                        self.messages(body, actual, args, call_id)
                    else:
                        self.chat(body, actual, args, call_id)
                except (BrokenPipeError, ConnectionResetError):
                    pass
                except Exception as error:
                    with provider.lock:
                        provider.errors.append(type(error).__name__)
                    self.backend("/<malformed>", 400, "invalid fixture request")
                    self.send_error(400)

            def reply(self, body):
                data = json.dumps(body).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def stream(self, events, anthropic=False):
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                for event in events:
                    if anthropic:
                        self.wfile.write(f"event: {event['type']}\n".encode())
                    self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                if not anthropic and self.path.endswith("/chat/completions"):
                    self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()
                self.close_connection = True

            def openai_responses(self, body, name, args, call_id, namespace):
                response_id = f"resp_fixture_{len(provider.requests)}"
                if name:
                    item = {"id": "fc_" + call_id, "type": "function_call", "call_id": call_id,
                            "name": name, "arguments": json.dumps(args), "status": "completed"}
                    if namespace:
                        item["namespace"] = namespace
                else:
                    item = {"id": "msg_" + call_id, "type": "message", "role": "assistant", "status": "completed",
                            "content": [{"type": "output_text", "text": "Fixture turn complete.", "annotations": []}]}
                response = {"id": response_id, "object": "response", "created_at": int(time.time()),
                            "status": "completed", "model": body.get("model"), "output": [item],
                            "usage": {"input_tokens": 32, "output_tokens": 16, "total_tokens": 48}}
                if not body.get("stream"):
                    self.reply(response)
                    return
                started = dict(response, status="in_progress", output=[])
                events = [{"type": "response.created", "response": started},
                          {"type": "response.output_item.added", "output_index": 0,
                           "item": dict(item, arguments="", status="in_progress") if name else item}]
                if name:
                    events.extend([{"type": "response.function_call_arguments.delta", "item_id": item["id"],
                                    "output_index": 0, "delta": json.dumps(args)},
                                   {"type": "response.function_call_arguments.done", "item_id": item["id"],
                                    "output_index": 0, "arguments": json.dumps(args)}])
                else:
                    events.append({"type": "response.output_text.delta", "item_id": item["id"],
                                   "output_index": 0, "content_index": 0, "delta": "Fixture turn complete."})
                events.extend([{"type": "response.output_item.done", "output_index": 0, "item": item},
                               {"type": "response.completed", "response": response}])
                for sequence, event in enumerate(events):
                    event["sequence_number"] = sequence
                self.stream(events)

            def messages(self, body, name, args, call_id):
                content = ({"type": "tool_use", "id": call_id, "name": name, "input": args} if name else
                           {"type": "text", "text": "Fixture turn complete."})
                message = {"id": "msg_" + call_id, "type": "message", "role": "assistant",
                           "model": body.get("model"), "content": [content],
                           "stop_reason": "tool_use" if name else "end_turn", "stop_sequence": None,
                           "usage": {"input_tokens": 32, "output_tokens": 16}}
                if not body.get("stream"):
                    self.reply(message)
                    return
                start = dict(message, content=[], stop_reason=None)
                block = dict(content, input={}) if name else dict(content, text="")
                delta = {"type": "input_json_delta", "partial_json": json.dumps(args)} if name else {
                    "type": "text_delta", "text": "Fixture turn complete."}
                self.stream([{"type": "message_start", "message": start},
                             {"type": "content_block_start", "index": 0, "content_block": block},
                             {"type": "content_block_delta", "index": 0, "delta": delta},
                             {"type": "content_block_stop", "index": 0},
                             {"type": "message_delta", "delta": {"stop_reason": message["stop_reason"],
                                                                    "stop_sequence": None},
                              "usage": {"output_tokens": 16}}, {"type": "message_stop"}], anthropic=True)

            def chat(self, body, name, args, call_id):
                tool = {"id": call_id, "type": "function", "function": {"name": name, "arguments": json.dumps(args)}}
                message = {"role": "assistant", "content": None if name else "Fixture turn complete."}
                if name:
                    message["tool_calls"] = [tool]
                result = {"id": "chat_" + call_id, "object": "chat.completion", "created": int(time.time()),
                          "model": body.get("model"), "choices": [{"index": 0, "message": message,
                                                                    "finish_reason": "tool_calls" if name else "stop"}],
                          "usage": {"prompt_tokens": 32, "completion_tokens": 16, "total_tokens": 48}}
                if not body.get("stream"):
                    self.reply(result)
                    return
                delta = dict(message)
                if name:
                    delta["tool_calls"] = [dict(tool, index=0)]
                base = {key: value for key, value in result.items() if key not in ("choices", "usage")}
                base["object"] = "chat.completion.chunk"
                self.stream([dict(base, choices=[{"index": 0, "delta": delta, "finish_reason": None}]),
                             dict(base, choices=[{"index": 0, "delta": {},
                                                  "finish_reason": "tool_calls" if name else "stop"}],
                                  usage=result["usage"])])

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def url(self):
        return f"http://127.0.0.1:{self.server.server_port}"

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
