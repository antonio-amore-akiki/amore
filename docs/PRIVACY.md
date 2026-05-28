<!-- stable: true -->
# Amore Privacy Policy

## Zero telemetry

Amore collects no telemetry, analytics, or usage data. No data is ever sent to any
external server. The project does not include any tracking SDK, beacon, or phone-home
mechanism.

## Crash diagnostics (local-only)

When a crash or panic occurs, Amore may write a local crash dump file to disk:

- Windows: `%LOCALAPPDATA%\Amore\crashes\<timestamp>-<id>.dmp`
- macOS/Linux: `$XDG_CACHE_HOME/amore/crashes/<timestamp>-<id>.dmp`

These files contain the panic message, source location, and timestamp. They are stored
locally and never transmitted. You must explicitly share them (e.g. via
`amore diag bundle`) to include them in a bug report.

## Opt-out

Set `AMORE_NO_CRASH_DIAG=1` in your environment to disable all crash dump writing.
No files will be created and no hooks will be registered.

## Known constraint

Silent crashes (e.g. out-of-memory kills by the OS, hard power loss) may not produce
a dump. In these cases the crash stays invisible to the diagnostics system. This is an
honest limitation, not a workaround.

## Update checks

`amore update check` makes a single HTTPS GET request to:

```
https://api.github.com/repos/antonio-amore-akiki/amore/releases/latest
```

No authentication, no cookies, no user identifiers are sent. Set
`AMORE_NO_AUTOUPDATE=1` to disable all update checks.

## Data retention

Crash dumps are never automatically deleted. Use `amore diag bundle` to collect them
for sharing, then delete the `crashes/` directory manually if desired.

## Contact

Security issues: open a private advisory at
`https://github.com/antonio-amore-akiki/amore/security/advisories`.
