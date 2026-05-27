stable: true

# Uninstallation — Amore

## Remove binary

### Homebrew

```bash
brew uninstall amore
brew untap antonio-amore-akiki/amore
```

### WinGet

```powershell
winget uninstall Antonio.Amore
```

### AUR

```bash
yay -R amore-bin
```

### Docker

```bash
docker rm -f amore
docker rmi ghcr.io/antonio-amore-akiki/amore:1.0.0
```

### Manual binary

Delete `amore` and `amore-mcp` from the `$PATH` location.

---

## Remove data + config

### Windows

```powershell
Remove-Item -Recurse "$env:APPDATA\amore"
Remove-Item -Recurse "$env:LOCALAPPDATA\Amore"
```

### Linux

```bash
rm -rf ~/.local/share/amore ~/.config/amore
```

### macOS

```bash
rm -rf "$HOME/Library/Application Support/amore"
rm -rf "$HOME/Library/Preferences/amore"
```

---

## Remove secrets from OS keyring

```bash
amore secrets list    # lists stored secret keys (read-only; values not shown)
```

Manual removal: search for "amore" in Credential Manager (Windows) / GNOME Keyring / macOS Keychain.

---

## Source

- docs/SECRETS.md (keyring storage paths)
