# 9. Add gRPC server mode for shared multi-client scenarios

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

The default `amore-mcp` server uses stdio transport: the IDE spawns the
binary, communicates over stdin/stdout, and the process exits when the
IDE closes. This works perfectly for one-IDE-at-a-time usage.

In v0.7.0 (Phase H), Amore must support shared multi-client scenarios:
a developer running Claude Code, Cursor, and a custom agent CLI all
attached to the same memory backbone simultaneously. stdio locks a
single spawned process to one client; the second IDE spawn is a
separate process with a separate in-memory state, which fragments the
memory store.

Which server mode should support multi-client attachment?

## Decision Drivers

* Shared multi-client: multiple IDEs attach to the SAME memory process
* stdio MCP must remain the default (ADR-0003; zero network attack
  surface for single-IDE users)
* Unix socket or TCP must be available for inter-process communication
  on the same machine
* TLS opt-in for team deployments where the server is on a remote node
* Strongly-typed wire protocol preferred over JSON-over-HTTP for
  performance-sensitive recall calls
* Rust ecosystem support for the chosen transport

## Considered Options

* Stdio-only (current; no multi-client)
* gRPC via tonic 0.12 (Unix socket default; TCP+TLS opt-in)
* REST via axum (HTTP/1.1 or HTTP/2)
* WebSocket via tokio-tungstenite

## Decision Outcome

Chosen option: **gRPC via tonic 0.12**.

stdio MCP remains the default transport for single-IDE users (ADR-0003
unchanged). The gRPC server is an additional opt-in mode:

```toml
# ~/.amore/config.toml
[server]
grpc_enabled = true
grpc_socket   = "/tmp/amore.sock"   # Unix domain socket (default)
# grpc_addr  = "127.0.0.1:50051"   # TCP (opt-in)
# grpc_tls   = true                 # TLS (opt-in; requires cert paths)
```

The gRPC server binds a Unix domain socket by default. IDE adapters
that want multi-client mode connect via the socket instead of spawning
a new stdio process. TCP + TLS is available for team deployments where
the Amore server runs on a remote node.

Proto file: `proto/amore.proto`. The `Recall`, `Observe`, and
`CanonicalDocLookup` RPC methods mirror the MCP tool surface.

gRPC mode is scheduled for v0.7.0 (Phase H).

### Consequences

* Good: shared multi-client solved without spawning N separate processes
* Good: Unix domain socket is local-only by default; same attack surface
  as stdio for single-machine use
* Good: tonic is the de-facto Rust gRPC library; well-maintained,
  Tokio-native
* Good: strongly-typed proto schema; client codegen available for
  Go/Python IDE adapters that don't use the MCP path
* Bad: second transport surface to maintain (stdio + gRPC)
* Bad: Unix socket path must be agreed by all clients; platform differences
  (Windows named pipes vs POSIX sockets) require extra handling
* Bad: TCP+TLS path adds a certificate management concern for team deploys

## Pros and Cons of the Options

### Stdio-only (current)

* Good: zero network surface; simplest
* Bad: one spawned process per IDE; fragmented memory state
* Bad: no path to multi-client without a process-level shared state layer

### gRPC via tonic 0.12 (CHOSEN)

* Good: strongly-typed; client stubs generated from proto
* Good: Unix socket default = local-only attack surface
* Good: TCP+TLS available for team/remote scenarios
* Good: Tokio-native; fits existing async runtime
* Bad: second transport to maintain alongside stdio MCP
* Bad: Windows named-pipe vs POSIX socket abstraction adds code

### REST via axum

* Good: universal tooling; curl / Postman for debugging
* Bad: JSON overhead vs proto binary for high-frequency recall calls
* Bad: HTTP/1.1 multiplexing weaker than HTTP/2 (gRPC)
* Bad: no generated client stubs; adapters must hand-write JSON parsing

### WebSocket via tokio-tungstenite

* Good: bidirectional streaming; good for push notifications
* Bad: no generated typed stubs; wire format is custom
* Bad: multiplexing and back-pressure semantics are application-defined
* Bad: less prior art in the Rust MCP ecosystem

## More Information

* tonic crate: https://crates.io/crates/tonic
* Proto file: `proto/amore.proto` (Phase H)
* Windows named-pipe fallback: `\\.\pipe\amore` when Unix socket not
  available (Windows pre-WSL2 installs)
* The gRPC server and stdio server share the same `RecallEngine`
  instance via `Arc<RecallEngine>`; no code duplication
* See ADR-0003 for the stdio MCP rationale (unchanged)
* Scheduled for v0.7.0 (Phase H) alongside cluster mode (ADR-0007)
