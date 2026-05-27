<!-- stable: true -->
# Amore Auto-Update

Amore ships a built-in update mechanism backed by [self_update](https://crates.io/crates/self_update).

## Commands

```
amore update check    # Check for a newer release (respects 24h cooldown)
amore update apply    # Download and replace the binary after user confirmation
```

## How it works

1. `amore update check` queries the GitHub Releases API for `antonio-amore-akiki/amore`.
2. The current binary version is compared against the latest `tag_name`.
3. If a newer version exists, the user is informed and prompted to run `apply`.
4. `amore update apply` prompts for confirmation via stdin before replacing the binary.

## 24-hour cooldown

Checks are rate-limited to one per 24 hours. The last-check timestamp is stored at:

- Windows: `%LOCALAPPDATA%\Amore\.last-update-check`
- macOS/Linux: `$XDG_CACHE_HOME/amore/.last-update-check`

## Opt-out

Set `AMORE_NO_AUTOUPDATE=1` in your environment to disable all update checks.
`check_for_update()` returns immediately with `UpdateStatus::Disabled`.

## Security

Releases are fetched over HTTPS from GitHub. The `signatures` feature of `self_update`
enables signature verification when release assets include a `.sig` sidecar. This aligns
with SLSA L2 provenance requirements (build integrity via signed artifacts).

## Privacy

No telemetry is sent. The only network call is a standard HTTPS GET to
`https://api.github.com/repos/antonio-amore-akiki/amore/releases/latest`.
