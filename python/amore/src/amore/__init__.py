"""
amore — mem0-compatible Python client backed by the Amore MCP server.

Prior-art: Adapt from npm/postinstall.js + npm/bin/amore-mcp.js subprocess
pattern (repo-local), mem0ai API shape (pypi.org/project/mem0ai), and
openai-python hatchling src-layout convention.  No upstream source vendored.

Drop-in replacement for mem0ai.Memory:

    from amore import Memory
    m = Memory()
    m.add("Alice prefers dark mode", user_id="alice")
    results = m.search("UI preferences", user_id="alice")

The client spawns amore-mcp as a subprocess and communicates over stdio
MCP JSON-RPC (see _subprocess.py).  No network call leaves the machine
unless the underlying amore-mcp is configured with a remote Qdrant.
"""

from __future__ import annotations

from typing import Any

from ._subprocess import AmoreSubprocess

__all__ = ["Memory"]
__version__ = "1.0.2"


class Memory:
    """
    mem0-compatible agent-memory client backed by Amore.

    Parameters
    ----------
    host:
        Ignored — preserved for mem0 API compat (amore-mcp runs in-process
        via stdio, not over TCP).
    port:
        Ignored — same reason as ``host``.
    binary:
        Override the ``amore-mcp`` binary path.  Defaults to the ``amore-mcp``
        found on ``PATH``.
    """

    def __init__(
        self,
        host: str = "localhost",
        port: int = 8765,
        *,
        binary: str | None = None,
    ) -> None:
        self._proc = AmoreSubprocess(binary=binary)

    def add(
        self,
        text: str,
        user_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Store a memory fragment. Returns ``{"id": "<uuid>", "status": "added"}``."""
        params: dict[str, Any] = {"content": text}
        if user_id is not None:
            params["user_id"] = user_id
        if metadata:
            params["metadata"] = metadata
        return self._proc.call("observe", params)

    def search(
        self,
        query: str,
        user_id: str | None = None,
        limit: int = 5,
    ) -> list[dict[str, Any]]:
        """Semantic + keyword search. Returns list of ``{"id", "memory", "score", ...}``."""
        params: dict[str, Any] = {"query": query, "limit": limit}
        if user_id is not None:
            params["user_id"] = user_id
        result = self._proc.call("recall", params)
        return result.get("results", [])

    def get_all(
        self,
        user_id: str | None = None,
    ) -> list[dict[str, Any]]:
        """Return every stored memory (optionally filtered by user_id)."""
        params: dict[str, Any] = {}
        if user_id is not None:
            params["user_id"] = user_id
        result = self._proc.call("list", params)
        return result.get("results", [])

    def delete(self, memory_id: str) -> dict[str, Any]:
        """Delete a memory by UUID. Returns ``{"id": "<uuid>", "status": "deleted"}``."""
        return self._proc.call("forget", {"id": memory_id})

    def close(self) -> None:
        """Terminate the background amore-mcp process."""
        self._proc.close()

    def __enter__(self) -> "Memory":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()
