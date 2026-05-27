"""
_subprocess.py — AmoreSubprocess: wraps amore-mcp binary via stdio MCP JSON-RPC.

Prior-art: Adapt from npm/bin/amore-mcp.js exec shim (repo-local) and
crates/amore-integration-tests/tests/mcp_handshake.rs initialize handshake.
stdlib subprocess + json only — no third-party deps.

Protocol: MCP stdio transport (newline-delimited JSON-RPC 2.0).
  - Client sends: {"jsonrpc":"2.0","id":<int>,"method":"tools/call",
                   "params":{"name":<tool>,"arguments":<args>}}
  - Server replies: {"jsonrpc":"2.0","id":<int>,"result":{...}}

Failure mode is LOUD: any protocol/process error raises immediately.
No silent fail-open.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import threading
from typing import Any


class AmoreSubprocess:
    """
    Long-lived amore-mcp subprocess with MCP initialize handshake.

    The process is started on construction and terminated on ``close()``.
    Calls are synchronous (lock-protected) — not safe for concurrent use
    from multiple threads without external locking.
    """

    _id: int
    _lock: threading.Lock

    def __init__(self, binary: str | None = None) -> None:
        exe = binary or shutil.which("amore-mcp")
        if exe is None:
            raise FileNotFoundError(
                "amore-mcp not found on PATH. Install via:\n"
                "  npm install -g amore-mcp\n"
                "  # or: pip install amore and then: amore-mcp (if bundled)\n"
                "  # or: download from https://github.com/antonio-amore-akiki/amore/releases"
            )
        self._proc = subprocess.Popen(
            [exe],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=sys.stderr,
            bufsize=0,
        )
        self._id = 0
        self._lock = threading.Lock()
        self._handshake()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _next_id(self) -> int:
        self._id += 1
        return self._id

    def _send(self, obj: dict[str, Any]) -> None:
        assert self._proc.stdin is not None
        line = json.dumps(obj, separators=(",", ":")) + "\n"
        self._proc.stdin.write(line.encode())
        self._proc.stdin.flush()

    def _recv(self) -> dict[str, Any]:
        assert self._proc.stdout is not None
        while True:
            raw = self._proc.stdout.readline()
            if not raw:
                raise RuntimeError(
                    "amore-mcp subprocess closed stdout unexpectedly. "
                    "Check that amore-mcp is installed and working."
                )
            raw = raw.strip()
            if not raw:
                continue
            try:
                return json.loads(raw)
            except json.JSONDecodeError as exc:
                raise RuntimeError(
                    f"amore-mcp sent non-JSON line: {raw[:200]!r}"
                ) from exc

    def _handshake(self) -> None:
        """Send MCP initialize request and consume the response."""
        req_id = self._next_id()
        self._send({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "clientInfo": {"name": "amore-python", "version": "1.0.2"},
                "capabilities": {},
            },
        })
        resp = self._recv()
        if resp.get("id") != req_id:
            raise RuntimeError(
                f"MCP initialize response id mismatch: expected {req_id}, got {resp.get('id')}"
            )
        if "error" in resp:
            raise RuntimeError(f"MCP initialize error: {resp['error']}")
        # Send initialized notification (no response expected).
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

    # ------------------------------------------------------------------
    # Public interface
    # ------------------------------------------------------------------

    def call(self, tool: str, arguments: dict[str, Any]) -> dict[str, Any]:
        """
        Invoke an amore-mcp tool via ``tools/call`` and return the result dict.

        Raises
        ------
        RuntimeError
            On JSON-RPC error response or subprocess failure.
        """
        with self._lock:
            req_id = self._next_id()
            self._send({
                "jsonrpc": "2.0",
                "id": req_id,
                "method": "tools/call",
                "params": {"name": tool, "arguments": arguments},
            })
            resp = self._recv()

        if resp.get("id") != req_id:
            raise RuntimeError(
                f"Response id mismatch for tool {tool!r}: "
                f"expected {req_id}, got {resp.get('id')}"
            )
        if "error" in resp:
            err = resp["error"]
            raise RuntimeError(
                f"amore-mcp tool {tool!r} error {err.get('code')}: {err.get('message')}"
            )
        # MCP tools/call result shape: {"content": [{"type":"text","text":"<json>"}]}
        result = resp.get("result", {})
        content = result.get("content", [])
        if content and isinstance(content, list):
            text = content[0].get("text", "")
            try:
                return json.loads(text)
            except (json.JSONDecodeError, TypeError):
                return {"raw": text}
        return result

    def close(self) -> None:
        """Terminate the amore-mcp subprocess."""
        try:
            if self._proc.stdin:
                self._proc.stdin.close()
            self._proc.terminate()
            self._proc.wait(timeout=5)
        except Exception:
            self._proc.kill()
