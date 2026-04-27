<!-- stable: true -->
# Amore Fuzzing

`cargo-fuzz` (libFuzzer-rs) — coverage-guided fuzzing for safety-critical parsers and decoders.

## Targets

| Target | Surface | Entry point |
|---|---|---|
| `canonical_json` | Canonical-JSON provenance parser | `amore_core::provenance::Envelope::seal` |
| `mcp_protocol` | MCP IDE config parser | `amore_core::ide_adapter::merge_mcp_servers` |

## Prerequisites

- Nightly Rust: `rustup toolchain install nightly`
- cargo-fuzz: `cargo +nightly install cargo-fuzz --locked`
- Windows only: add the MSVC ASan DLL to PATH before running:

```powershell
$env:PATH = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\<ver>\bin\HostX64\x64;$env:PATH"
```

## Run

```bash
cd crates/amore-core
cargo +nightly fuzz run canonical_json -- -max_total_time=60
cargo +nightly fuzz run mcp_protocol   -- -max_total_time=300
```

Drop `-max_total_time` for unbounded fuzzing. Crash corpus written to
`crates/amore-core/fuzz/corpus/<target>/`.

## CI

None (no GHA minutes). Local runs only. Recommended cadence: weekly 1-hour run per target
on developer machine.

## Baseline (W4-4B, 2026-05-27)

| Target | Runs | Crashes | Duration |
|---|---|---|---|
| `canonical_json` | 733 370 | 0 | 61 s |
| `mcp_protocol` | 1 767 332 | 0 | 61 s |

## Source

- rust-fuzz.github.io/book/
- github.com/rust-fuzz/cargo-fuzz
- OSSF Scorecard Fuzzing check: github.com/ossf/scorecard/blob/main/docs/checks.md#fuzzing
