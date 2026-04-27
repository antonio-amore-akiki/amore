# Amore — Packaging

Distribution descriptors for `brew install amore` (macOS/Linux) and
`winget install amore` (Windows). SHAs are placeholder until the first GitHub
Release tag; replace at release time (see **Releasing checklist** below).

| Surface  | Platform      | Schema               | Status                              |
|----------|---------------|----------------------|-------------------------------------|
| Homebrew | macOS + Linux | Formula DSL (Ruby)   | descriptor ready; Phase J: submit tap PR |
| winget   | Windows       | singleton YAML 1.6.0 | descriptor ready; Phase J: submit winget-pkgs PR |

---

## Homebrew (`packaging/homebrew/amore.rb`)

### Per-release update

1. Build the release tarballs (`amore-v{VER}-x86_64-apple-darwin.tar.gz`,
   `amore-v{VER}-x86_64-unknown-linux-gnu.tar.gz`) and compute SHA-256:
   ```
   sha256sum amore-v*.tar.gz
   ```
2. Bump `version`, `sha256` fields in `amore.rb`, then run:
   ```
   brew bump-formula-pr --url=<tarball-url> --sha256=<sha> packaging/homebrew/amore.rb
   ```
3. For the first submission, open a PR to `Homebrew/homebrew-core` (Phase J).

### Local validation

Requires macOS or Linux with Homebrew installed:
```
brew audit --strict packaging/homebrew/amore.rb
brew install --build-from-source packaging/homebrew/amore.rb
```

Note: `brew audit` is unavailable on Windows. Validation is deferred to a macOS/Linux CI
runner. PLACEHOLDER_*_sha256 values will produce expected warnings until replaced.

---

## winget (`packaging/winget/manifests/a/Antonio/Amore/0.5.0/`)

### Per-release update

1. Build the Windows installer (`Amore-Setup-v{VER}.exe`) and compute SHA-256:
   ```
   certutil -hashfile Amore-Setup-v0.5.0.exe SHA256
   ```
2. Use `wingetcreate update` to generate the updated manifest:
   ```
   wingetcreate update Antonio.Amore --version 0.5.1 --urls <url> --out packaging/winget/manifests/
   ```
3. Validate locally:
   ```
   winget validate --manifest packaging/winget/manifests/a/Antonio/Amore/0.5.1/
   ```
4. For the first submission, open a PR to `microsoft/winget-pkgs` (Phase J).

### Local validation

```
winget validate --manifest packaging/winget/manifests/a/Antonio/Amore/0.5.0/
```

The current `InstallerSha256` is a zeroed 64-char hex string (schema-valid placeholder).
`winget validate` checks schema structure only (not live URL or hash correctness), so
validation passes. Replace with the real SHA-256 of the installer at release time.

---

## Releasing checklist

- [ ] `version` bumped in `amore.rb` and winget manifest directory name + `PackageVersion`
- [ ] Real SHA-256 values replace all PLACEHOLDER_* entries
- [ ] `brew audit --strict` exits 0 on macOS runner
- [ ] `winget validate --manifest` exits 0 on Windows runner
- [ ] Phase J: submit PR to Homebrew/homebrew-core (user-driven)
- [ ] Phase J: submit PR to microsoft/winget-pkgs (user-driven)
