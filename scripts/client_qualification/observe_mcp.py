#!/usr/bin/env python3
"""Transparent stdio observer; production MCP remains the operation authority.

Receipts describe bytes observed, not client delivery or durable execution.
EOF/reader closure closes pipes and waits for natural bridge exit. No signals or
forced cleanup are issued; the qualification harness owns timeout recovery.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import stat
import subprocess
import sys
import threading

REDACT = {"ticket", "credential", "secret", "session_secret", "authorization", "api_key", "apikey"}


def redacted(value):
    if isinstance(value, dict):
        return {key: "<redacted>" if key.lower() in REDACT else redacted(item) for key, item in value.items()}
    if isinstance(value, list):
        return [redacted(item) for item in value]
    return value


def metadata(message, direction, calls):
    if not isinstance(message, dict):
        return {"event": "non_object_json", "direction": direction}
    record = {"event": "jsonrpc", "direction": direction, "jsonrpc": message.get("jsonrpc")}
    for key in ("id", "method"):
        if key in message:
            record[key] = message[key]
    if direction == "client_request":
        params = message.get("params", {})
        if message.get("method") == "tools/call" and isinstance(params, dict):
            record["tool"] = params.get("name")
            record["arguments"] = redacted(params.get("arguments", {}))
        elif message.get("method") == "initialize" and isinstance(params, dict):
            record["protocolVersion"] = params.get("protocolVersion")
        elif message.get("method") == "notifications/cancelled" and isinstance(params, dict):
            record["requestId"] = params.get("requestId")
        if "id" in message:
            calls[json.dumps(message["id"], sort_keys=True)] = record.copy()
    else:
        record["has_result"] = "result" in message
        request = calls.pop(json.dumps(message.get("id"), sort_keys=True), {})
        record["request_method"] = request.get("method")
        if "tool" in request:
            record["tool"] = request["tool"]
        if "error" in message:
            record["error"] = redacted(message["error"])
        result = message.get("result")
        if request.get("method") == "tools/list":
            canonical = json.dumps(result, sort_keys=True, separators=(",", ":")).encode()
            record["schema_sha256"] = hashlib.sha256(canonical).hexdigest()
            # These are public schema definitions, not operation arguments or
            # results. Redacting a property named `ticket` would corrupt the
            # schema and make its retained digest unverifiable.
            record["tool_definitions"] = result.get("tools", []) if isinstance(result, dict) else []
            record["tools"] = [tool.get("name") for tool in result.get("tools", [])] if isinstance(result, dict) else []
        elif request.get("method") == "initialize":
            record["result"] = redacted(result)
        elif request.get("method") == "tools/call" and isinstance(result, dict):
            record["isError"] = result.get("isError")
            if "structuredContent" in result:
                record["result"] = redacted(result["structuredContent"])
            else:
                # Older MCP versions carry JSON in text blocks. Never retain
                # opaque text: it can contain an invitation secret in prose.
                content = []
                for block in result.get("content", []):
                    if isinstance(block, dict) and block.get("type") == "text":
                        try:
                            content.append(redacted(json.loads(block.get("text", ""))))
                        except (ValueError, TypeError):
                            content.append({"opaque_text_sha256": hashlib.sha256(str(block.get("text", "")).encode()).hexdigest()})
                record["result"] = content
    return record


class Receipts:
    def __init__(self, file):
        self.file = file
        self.calls = {}
        self.buffers = {"client_request": bytearray(), "bridge_response": bytearray()}

    def write(self, value):
        self.file.write(json.dumps(value, sort_keys=True) + "\n")
        self.file.flush()

    def observe(self, data, direction):
        buffer = self.buffers[direction]
        buffer.extend(data)
        while b"\n" in buffer:
            line, _, rest = buffer.partition(b"\n")
            buffer[:] = rest
            try:
                value = metadata(json.loads(line), direction, self.calls)
            except (ValueError, UnicodeDecodeError, TypeError, AttributeError):
                value = {"event": "invalid_json", "direction": direction, "bytes": len(line), "sha256": hashlib.sha256(line).hexdigest()}
            self.write(value)


class OutputClosure:
    """Darwin poll does not watch an idle pipe writer; kqueue reports EV_EOF."""

    def __init__(self, output_fd):
        self.notification = None
        self.thread = None
        if not hasattr(select, "kqueue"):
            return
        self.notify_read, self.notify_write = os.pipe()
        self.stop_read, self.stop_write = os.pipe()
        self.notification = self.notify_read
        self.queue = select.kqueue()
        self.queue.control([
            select.kevent(output_fd, filter=select.KQ_FILTER_WRITE,
                          flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR),
            select.kevent(self.stop_read, filter=select.KQ_FILTER_READ,
                          flags=select.KQ_EV_ADD),
        ], 0)

        def watch():
            while True:
                for event in self.queue.control([], 2):
                    if event.ident == self.stop_read:
                        return
                    if event.flags & select.KQ_EV_EOF:
                        os.write(self.notify_write, b"x")
                        return

        self.thread = threading.Thread(target=watch, daemon=True)
        self.thread.start()

    def close(self):
        if self.thread is not None:
            os.write(self.stop_write, b"x")
            self.thread.join()
            self.queue.close()
            for fd in (self.notify_read, self.notify_write, self.stop_read, self.stop_write):
                os.close(fd)


def observe(executable, arguments, receipt, events_file, input_fd=0, output_fd=1):
    digest = hashlib.sha256()
    with open(executable, "rb") as binary:
        for chunk in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(chunk)
    child = subprocess.Popen([executable, *arguments], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    receipts = Receipts(receipt)
    receipts.write({"event": "bridge_started", "pid": child.pid, "observer_pid": os.getpid(), "executable": executable, "executable_sha256": digest.hexdigest(), "events_file": str(events_file)})
    child_in, child_out = child.stdin.fileno(), child.stdout.fileno()
    for fd in (input_fd, output_fd, child_in, child_out):
        os.set_blocking(fd, False)
    closure = OutputClosure(output_fd)
    incoming, outgoing = bytearray(), bytearray()
    input_open = output_open = bridge_open = True
    stdin_open = True
    while bridge_open or outgoing:
        poll = select.poll()
        if closure.notification is not None:
            poll.register(closure.notification, select.POLLIN)
        if input_open and stdin_open and not incoming:
            poll.register(input_fd, select.POLLIN)
        if bridge_open and not outgoing:
            poll.register(child_out, select.POLLIN)
        if output_open:
            poll.register(output_fd, select.POLLOUT if outgoing else 0)
        if stdin_open and incoming:
            poll.register(child_in, select.POLLOUT)
        for fd, event in poll.poll():
            if fd == output_fd or fd == closure.notification:
                if fd == closure.notification or event & (select.POLLERR | select.POLLHUP | select.POLLNVAL):
                    output_open = input_open = bridge_open = False
                    incoming.clear(); outgoing.clear()
                    child.stdout.close()
                    receipts.write({"event": "client_output_closed"})
                elif outgoing:
                    try:
                        count = os.write(output_fd, outgoing)
                        del outgoing[:count]
                    except BlockingIOError:
                        pass
                    except BrokenPipeError:
                        output_open = input_open = bridge_open = False
                        incoming.clear(); outgoing.clear(); child.stdout.close()
                        receipts.write({"event": "client_output_closed"})
            elif fd == input_fd and input_open:
                try:
                    data = os.read(input_fd, 65536)
                except BlockingIOError:
                    continue
                if data:
                    receipts.observe(data, "client_request")
                    incoming.extend(data)
                else:
                    input_open = False
                    receipts.write({"event": "client_input_eof"})
            elif fd == child_out and bridge_open:
                try:
                    data = os.read(child_out, 65536)
                except BlockingIOError:
                    continue
                if data:
                    receipts.observe(data, "bridge_response")
                    outgoing.extend(data)
                else:
                    bridge_open = input_open = False
                    incoming.clear()
            elif fd == child_in:
                try:
                    count = os.write(child_in, incoming)
                    del incoming[:count]
                except BlockingIOError:
                    pass
                except BrokenPipeError:
                    incoming.clear(); input_open = False
        if stdin_open and not input_open and not incoming:
            child.stdin.close()
            stdin_open = False
    if stdin_open:
        child.stdin.close()
    if not child.stdout.closed:
        child.stdout.close()
    closure.close()
    code = child.wait()
    receipts.write({"event": "bridge_exited", "pid": child.pid, "exit_code": code, "cleanup": "natural_wait_no_signals"})
    return code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--events-file", required=True)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--argument", action="append", default=[])
    args = parser.parse_args()
    path = Path(args.events_file)
    executable = Path(args.executable)
    if not path.is_absolute() or not executable.is_absolute():
        parser.error("receipt and executable paths must be absolute")
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW)
    mode = os.fstat(fd)
    if not stat.S_ISREG(mode.st_mode) or stat.S_IMODE(mode.st_mode) != 0o600 or mode.st_uid != os.getuid() or mode.st_nlink != 1:
        os.close(fd)
        parser.error("receipt must be an existing owned regular file with mode 0600 and no hardlinks")
    with os.fdopen(fd, "w") as receipt:
        return observe(str(executable), args.argument, receipt, path)


if __name__ == "__main__":
    sys.exit(main())
