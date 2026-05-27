"""
test_memory.py -- Smoke tests for amore.Memory (mem0-compatible API).

Tests use unittest.mock to avoid requiring a live amore-mcp process in CI.
Run:  python -m pytest python/amore/tests/  (or python -m unittest discover)

Patch target: "amore.AmoreSubprocess" -- the name as imported into amore/__init__.py.
"""

import sys
import unittest
from unittest.mock import patch

# Insert src/ so tests can run without installing the package.
import pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent.parent / "src"))

from amore import Memory  # noqa: E402

# Patch target: amore/__init__.py does "from ._subprocess import AmoreSubprocess"
# so the live binding is at "amore.AmoreSubprocess".
PATCH = "amore.AmoreSubprocess"


def _make_cls(return_vals=None, calls_out=None):
    """Return a fake AmoreSubprocess class that pops from return_vals and logs to calls_out."""
    rv = list(return_vals or [])
    cl = calls_out if calls_out is not None else []

    class Fake:
        def __init__(self, binary=None):
            pass
        def call(self, tool, args):
            cl.append((tool, args))
            if not rv:
                raise AssertionError(f"Unexpected call: {tool!r}")
            return rv.pop(0)
        def close(self): pass

    return Fake, cl


class TestMemoryAdd(unittest.TestCase):
    def test_add_returns_id_and_status(self):
        cls, _ = _make_cls([{"id": "abc-123", "status": "added"}])
        with patch(PATCH, cls):
            m = Memory()
            result = m.add("Alice prefers dark mode", user_id="alice")
        self.assertEqual(result["id"], "abc-123")
        self.assertEqual(result["status"], "added")

    def test_add_passes_user_id(self):
        cls, calls = _make_cls([{"id": "x", "status": "added"}])
        with patch(PATCH, cls):
            m = Memory()
            m.add("test", user_id="bob")
        self.assertEqual(calls[0][0], "observe")
        self.assertEqual(calls[0][1]["user_id"], "bob")

    def test_add_no_user_id(self):
        cls, calls = _make_cls([{"id": "y", "status": "added"}])
        with patch(PATCH, cls):
            m = Memory()
            m.add("no user")
        self.assertNotIn("user_id", calls[0][1])


class TestMemorySearch(unittest.TestCase):
    def test_search_returns_results(self):
        canned = {"results": [{"id": "abc-123", "memory": "Alice prefers dark mode", "score": 0.95}]}
        cls, calls = _make_cls([canned])
        with patch(PATCH, cls):
            m = Memory()
            results = m.search("UI preferences", user_id="alice", limit=3)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0]["memory"], "Alice prefers dark mode")
        self.assertAlmostEqual(results[0]["score"], 0.95)

    def test_search_empty_results(self):
        cls, _ = _make_cls([{"results": []}])
        with patch(PATCH, cls):
            m = Memory()
            results = m.search("nothing here")
        self.assertEqual(results, [])


class TestMemoryGetAll(unittest.TestCase):
    def test_get_all_returns_list(self):
        canned = {"results": [{"id": "1", "memory": "mem one"}, {"id": "2", "memory": "mem two"}]}
        cls, calls = _make_cls([canned])
        with patch(PATCH, cls):
            m = Memory()
            results = m.get_all(user_id="alice")
        self.assertEqual(len(results), 2)
        self.assertEqual(calls[0][0], "list")
        self.assertEqual(calls[0][1].get("user_id"), "alice")


class TestMemoryDelete(unittest.TestCase):
    def test_delete_returns_status(self):
        cls, calls = _make_cls([{"id": "abc-123", "status": "deleted"}])
        with patch(PATCH, cls):
            m = Memory()
            result = m.delete("abc-123")
        self.assertEqual(result["status"], "deleted")
        self.assertEqual(calls[0][0], "forget")
        self.assertEqual(calls[0][1]["id"], "abc-123")


class TestMemoryContextManager(unittest.TestCase):
    def test_context_manager_calls_close(self):
        cls, _ = _make_cls()
        with patch(PATCH, cls):
            with Memory() as m:
                self.assertIsInstance(m, Memory)


if __name__ == "__main__":
    unittest.main()
