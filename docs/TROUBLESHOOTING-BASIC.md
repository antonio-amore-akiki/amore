stable: true

# Troubleshooting Amore

> Stuck? Try these first. If your issue isn't here, see the [developer troubleshooting guide](TROUBLESHOOTING.md) or [report a bug](https://github.com/antonio-amore-akiki/amore/issues/new).

## 1. Amore won't start

- **Windows**: if you see a SmartScreen warning, click "More info" then "Run anyway". This happens because Amore is a new app that Windows hasn't seen many people install yet — it's not a virus.
- **macOS**: if you see "Amore cannot be opened because the developer cannot be verified", right-click the app and choose "Open". This bypasses Gatekeeper for an app installed outside the App Store.
- **Linux (AppImage)**: make sure you've made the file executable: `chmod +x amore-*.AppImage`.

Still not opening? Restart your computer once. If it still won't start, [open a bug report](https://github.com/antonio-amore-akiki/amore/issues/new).

## 2. My AI tool doesn't remember things

- Make sure you finished the first-run wizard end-to-end (it shows a "ready" screen at the end).
- Restart your AI tool (Claude Desktop, Cursor, Cline, etc.) — the memory link only activates after a fresh start.
- Open the Amore tray icon → "Recent activity" — if you see your messages flowing in, memory is working. If not, your AI tool didn't pick up the connection.
- Re-run the wizard from the tray menu ("Re-detect AI tools") to wire it again.

## 3. Something is broken / asking for help

- Tray icon → "Open dashboard" shows what Amore is doing right now.
- Tray icon → "Check for updates" — newer versions may fix your issue.
- Report bugs at [github.com/antonio-amore-akiki/amore/issues](https://github.com/antonio-amore-akiki/amore/issues). Include your OS + Amore version (shown in the tray menu).

For technical / developer troubleshooting, see the [full guide](TROUBLESHOOTING.md).
