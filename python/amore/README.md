# amore — Python client

mem0-compatible Python wrapper for [Amore](https://github.com/antonio-amore-akiki/amore) agent memory.

## Install

```bash
pip install amore
```

Requires `amore-mcp` on `PATH` (install once via `npm install -g amore-mcp`).

## Quick start

```python
from amore import Memory

m = Memory()

# Store a memory
m.add("Alice prefers dark mode", user_id="alice")

# Recall
results = m.search("UI preferences", user_id="alice", limit=3)
for r in results:
    print(r["memory"], r["score"])

# List all memories for a user
all_mems = m.get_all(user_id="alice")

# Delete
m.delete(all_mems[0]["id"])

m.close()
```

Context manager form:

```python
with Memory() as m:
    m.add("Bob uses vim", user_id="bob")
    hits = m.search("editor", user_id="bob")
```

## Migration from mem0

Replace `from mem0 import Memory` with `from amore import Memory`.
`add`, `search`, `get_all`, and `delete` have the same signatures.

## Binary path override

```python
m = Memory(binary="/usr/local/bin/amore-mcp")
```

## License

Apache-2.0
