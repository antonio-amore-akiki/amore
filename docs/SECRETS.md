<!-- stable: true -->
# Secrets Storage

## OS Keyring (primary)

Amore stores API keys in the OS keyring via the `keyring` crate (docs.rs/keyring/3.x):
- **Windows**: Windows Credential Manager
- **Linux**: Secret Service (libsecret daemon required)
- **macOS**: Keychain (present but not validated — no Apple hardware on dev machine)

## CLI

```
amore secrets set qdrant_api_key      # prompts, no-echo
amore secrets get qdrant_api_key      # prints to stdout
```

Service name: `amore`. Per-user storage; no root required.

## Fallback

If keyring read fails (e.g., headless Linux without a Secret Service daemon), Amore falls
back to `$config_dir/amore/secrets.toml`:
- Windows: `%APPDATA%/amore/secrets.toml`
- Linux: `~/.config/amore/secrets.toml`

File mode MUST be `0600` on Linux — Amore warns at read time if permissions are loose.
Windows ACL check is pending.

## Sources

- docs.rs/keyring/3.x
- github.com/hwchen/keyring-rs
- OSSF Scorecard Token-Permissions: github.com/ossf/scorecard/blob/main/docs/checks.md
