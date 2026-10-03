#!/usr/bin/env python3
"""Smoke-test transport_probe as separate processes on IPv4 loopback only.

Build the example first, then pass its executable with --binary. This checks
local direct transport, authenticated identities, rejection, timeout, and
SIGINT cleanup, plus a blocked-output timeout regression. It does not qualify
relay service or separate networks.
Only the Python standard library is required; child output stays in temporary
files and is never copied into the report because contacts may contain IPs.
The harness allows one additional --timeout-ms interval for a timed-out probe
to close its endpoint and exit; readiness and forced cleanup use that interval
directly. No external relay or address lookup is configured.
The blocked-output fixture uses a 100 ms probe deadline and 100 ms output
grace, followed by the caller-selected harness shutdown allowance.
"""

import argparse
from contextlib import AbstractContextManager, contextmanager
import ipaddress
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


class CheckFailure(Exception):
    """A smoke-test expectation failed, without including raw child output."""


@contextmanager
def prefilled_pipe():
    """Yield a blocking stdout writer whose undrained pipe has no free bytes."""
    read_fd, write_fd = os.pipe()
    try:
        os.set_blocking(write_fd, False)
        payload = b"x" * 65536
        while True:
            try:
                os.write(write_fd, payload)
            except BlockingIOError:
                if len(payload) == 1:
                    break
                # A large atomic write may fail with some capacity remaining.
                # Fill the remainder too so even a short metadata line blocks.
                payload = b"x"
        os.set_blocking(write_fd, True)
        with os.fdopen(write_fd, "wb", buffering=0) as output:
            write_fd = None  # The file object owns this descriptor now.
            yield output
    finally:
        if write_fd is not None:
            os.close(write_fd)
        os.close(read_fd)


def parse_records(output: str) -> list[dict[str, str]]:
    records = []
    for number, line in enumerate(output.splitlines(), 1):
        if not line.strip():
            continue
        record = {}
        for field in line.split():
            key, separator, value = field.partition("=")
            if not separator or not key or not value or key in record:
                raise CheckFailure(f"invalid output record on line {number}")
            record[key] = value
        if "record" not in record:
            raise CheckFailure(f"missing record discriminator on line {number}")
        records.append(record)
    return records


def one_record(records: list[dict[str, str]], kind: str) -> dict[str, str]:
    matches = [record for record in records if record["record"] == kind]
    if len(matches) != 1:
        raise CheckFailure(f"expected exactly one {kind} record")
    return matches[0]


def endpoint_id(record: dict[str, str], key: str = "endpoint_id") -> str:
    value = record.get(key, "")
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise CheckFailure(f"missing or invalid {key}")
    return value


class ProbeProcess:
    def __init__(self, arguments: list[str], cleanup_timeout: float,
                 stdout_fd: int | None = None):
        self.cleanup_timeout = cleanup_timeout
        self.directory = tempfile.TemporaryDirectory(prefix="locust-transport-probe-")
        self.stdout_path = Path(self.directory.name) / "stdout"
        self.stderr_path = Path(self.directory.name) / "stderr"
        try:
            with self.stdout_path.open("wb") as stdout, self.stderr_path.open("wb") as stderr:
                self.process = subprocess.Popen(
                    arguments, stdin=subprocess.DEVNULL,
                    stdout=stdout if stdout_fd is None else stdout_fd, stderr=stderr
                )
        except BaseException:
            self.directory.cleanup()
            raise

    def records(self) -> list[dict[str, str]]:
        # A separate file descriptor keeps reads from moving the child's write
        # position, and files avoid pipe backpressure while peers run together.
        output = self.stdout_path.read_text(encoding="utf-8")
        # Do not parse a record while the child is still writing its line.
        if self.process.poll() is None and not output.endswith("\n"):
            output = output.rpartition("\n")[0]
        return parse_records(output)

    def ready(self, timeout: float) -> tuple[str, str]:
        deadline = time.monotonic() + timeout
        while True:
            records = self.records()
            if any(record["record"] == "ready" for record in records):
                ready = one_record(records, "ready")
                identity = endpoint_id(ready)
                if ready.get("mode") != "direct":
                    raise CheckFailure("listener did not report direct mode")
                addresses = []
                for record in records:
                    if record["record"] != "contact":
                        continue
                    if endpoint_id(record) != identity:
                        raise CheckFailure("contact and ready identities differ")
                    if "relay_url" in record:
                        raise CheckFailure("loopback probe unexpectedly advertises a relay")
                    address = record.get("direct_addr")
                    if address is None:
                        continue
                    host, separator, port = address.rpartition(":")
                    try:
                        loopback = ipaddress.ip_address(host) == ipaddress.ip_address("127.0.0.1")
                        valid_port = 0 < int(port) <= 65535
                    except ValueError:
                        loopback = valid_port = False
                    if separator and loopback and valid_port:
                        addresses.append(address)
                if not addresses:
                    raise CheckFailure("listener did not publish an IPv4 loopback contact")
                return identity, addresses[0]
            if self.process.poll() is not None:
                raise CheckFailure("listener exited before readiness")
            if time.monotonic() >= deadline:
                raise CheckFailure("listener readiness exceeded the harness timeout")
            time.sleep(0.02)

    def finish(self, timeout: float) -> tuple[int, list[dict[str, str]]]:
        try:
            code = self.process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            raise CheckFailure("child exit exceeded the harness timeout") from error
        return code, self.records()

    def close(self):
        try:
            if self.process.poll() is None:
                try:
                    self.process.terminate()
                    self.process.wait(timeout=self.cleanup_timeout)
                except BaseException as error:
                    if self.process.poll() is None:
                        self.process.kill()
                    self.process.wait()
                    if not isinstance(error, subprocess.TimeoutExpired):
                        raise
            else:
                self.process.wait()
        finally:
            self.directory.cleanup()


class Children(AbstractContextManager):
    def __init__(self, timeout: float):
        self.timeout = timeout
        self.children = []

    def launch(self, arguments: list[str], stdout_fd: int | None = None) -> ProbeProcess:
        child = ProbeProcess(arguments, self.timeout, stdout_fd)
        self.children.append(child)
        return child

    def __exit__(self, exception_type, exception, traceback):
        cleanup_error = None
        try:
            for child in self.children:
                try:
                    child.close()
                except BaseException as error:
                    # One failed cleanup must not strand the other peer.
                    if cleanup_error is None:
                        cleanup_error = error
        finally:
            self.children.clear()
        if cleanup_error is not None and exception is None:
            raise cleanup_error
        return False


def expect_result(child: ProbeProcess, timeout: float, code: int, status: str,
                  error: str | None = None) -> list[dict[str, str]]:
    actual_code, records = child.finish(timeout)
    result = one_record(records, "result")
    if actual_code != code or result.get("status") != status:
        raise CheckFailure(f"expected exit {code} and result status {status}")
    if result.get("mode") != "direct":
        raise CheckFailure("result did not report direct mode")
    if error is not None and result.get("error") != error:
        raise CheckFailure(f"expected result error {error}")
    if status == "success" and result.get("phase") != "complete":
        raise CheckFailure("success did not complete the exchange")
    return records


def check_success(records: list[dict[str, str]], peer: str):
    if endpoint_id(one_record(records, "peer"), "peer_id") != peer:
        raise CheckFailure("authenticated remote identity did not match its peer")
    paths = [record for record in records if record["record"] == "path"
             and record.get("phase") == "after" and record.get("selected") == "true"]
    if not paths or any(record.get("kind") != "direct" for record in paths):
        raise CheckFailure("completed exchange did not select a direct route")


def run_checks(binary: Path, timeout_ms: int):
    timeout = timeout_ms / 1000
    base = [str(binary), "--mode", "direct", "--timeout-ms", str(timeout_ms),
            "--bind", "127.0.0.1:0"]

    def arguments(role: str, *extra: str) -> list[str]:
        return [base[0], role, *base[1:], *extra]

    with Children(timeout) as children:
        listener = children.launch(arguments("listen"))
        listener_id, address = listener.ready(timeout)
        connector = children.launch(arguments("connect", "--peer", listener_id,
                                              "--peer-addr", address))
        connector_records = expect_result(connector, timeout, 0, "success")
        listener_records = expect_result(listener, timeout, 0, "success")
        connector_id = endpoint_id(one_record(connector_records, "ready"))
        check_success(connector_records, listener_id)
        check_success(listener_records, connector_id)
    print("PASS authenticated process exchange and direct route (127.0.0.1)", flush=True)

    with Children(timeout) as children:
        listener = children.launch(arguments("listen", "--expect-peer", listener_id))
        rejected_listener_id, address = listener.ready(timeout)
        connector = children.launch(arguments("connect", "--peer", rejected_listener_id,
                                              "--peer-addr", address))
        rejected_records = expect_result(listener, timeout, 1, "failure", "unexpected_peer")
        connector_code, connector_records = connector.finish(timeout)
        if endpoint_id(one_record(connector_records, "ready")) == listener_id:
            raise CheckFailure("rejection fixture accidentally used the expected peer")
        if connector_code == 0 or one_record(connector_records, "result").get("status") != "failure":
            raise CheckFailure("rejected connector unexpectedly completed the exchange")
        if any(record["record"] == "result" and record.get("status") == "success"
               for record in rejected_records):
            raise CheckFailure("unexpected peer was accepted")
    print("PASS unexpected authenticated peer rejection", flush=True)

    with Children(timeout) as children:
        listener = children.launch(arguments("listen"))
        listener.ready(timeout)
        # The probe operation may use the full requested interval before its
        # asynchronous endpoint shutdown begins. Give shutdown a separate,
        # caller-selected interval rather than racing that operation deadline.
        expect_result(listener, timeout * 2, 124, "failure", "timeout")
    print("PASS no-peer timeout and process cleanup", flush=True)

    with Children(timeout) as children:
        listener = children.launch(arguments("listen"))
        listener.ready(timeout)
        listener.process.send_signal(signal.SIGINT)
        expect_result(listener, timeout, 130, "failure", "cancelled")
    print("PASS SIGINT cancellation and process cleanup", flush=True)

    # This is a short deadline regression fixture, independent of the timeout
    # chosen for network exchange. No pipe byte is read before the child exits.
    blocked_timeout_ms = 100
    blocked_arguments = [str(binary), "listen", "--mode", "direct", "--timeout-ms",
                         str(blocked_timeout_ms), "--bind", "127.0.0.1:0"]
    with prefilled_pipe() as output, Children(timeout) as children:
        listener = children.launch(blocked_arguments, stdout_fd=output.fileno())
        output.close()  # Only the child retains the write end after launch.
        code, _ = listener.finish(timeout + 2 * blocked_timeout_ms / 1000)
        # Machine records cannot be observed through an intentionally undrained
        # pipe. The timeout exit must still work despite the blocked writer.
        if code != 124:
            raise CheckFailure("blocked output did not exit with timeout status 124")
    print("PASS blocked-output timeout and process cleanup", flush=True)
    print("Transport probe smoke passed: local loopback only; relay and cross-network checks remain separate.")


def positive_milliseconds(value: str) -> int:
    try:
        milliseconds = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("timeout must be a positive integer") from error
    if milliseconds <= 0:
        raise argparse.ArgumentTypeError("timeout must be a positive integer")
    return milliseconds


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True,
                        help="absolute path to the built transport_probe example")
    parser.add_argument("--timeout-ms", type=positive_milliseconds, default=10000,
                        help="probe and harness wait timeout in milliseconds (default: 10000)")
    arguments = parser.parse_args()
    if not arguments.binary.is_absolute() or not arguments.binary.is_file():
        parser.error("--binary must be an existing absolute executable path")
    try:
        run_checks(arguments.binary, arguments.timeout_ms)
    except KeyboardInterrupt:
        print("Transport probe smoke interrupted; child processes reaped.", file=sys.stderr)
        return 130
    except (CheckFailure, OSError, UnicodeError) as error:
        # Never print OSError filenames or raw child records: they may contain
        # host contacts. CheckFailure messages contain only static labels.
        detail = str(error) if isinstance(error, CheckFailure) else type(error).__name__
        print(f"Transport probe smoke failed: {detail}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
