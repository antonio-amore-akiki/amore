# de/main.ftl — German (Deutsch) translations for Amore.
# machine-seeded 2026-05-28. Mark every string with "# machine-seeded" for community-PR review.
# To refine: open a PR targeting this file with native-speaker corrections.

## Application identity
# machine-seeded
app-name = Amore
# machine-seeded
app-tagline = Lokaler, persistenter Speicher für alle Ihre KI-Tools
# machine-seeded
app-version = Version { $version }

## Wizard — navigation
# machine-seeded
wizard-next = Weiter
# machine-seeded
wizard-back = Zurück
# machine-seeded
wizard-finish = Fertigstellen
# machine-seeded
wizard-cancel = Abbrechen

## Wizard — screens
# machine-seeded
wizard-welcome = Willkommen bei Amore — Ihr lokales KI-Gedächtnissystem
# machine-seeded
wizard-welcome-subtitle = Lokal, privat und kostenlos. Keine Cloud erforderlich.
# machine-seeded
wizard-step-1 = Verbindung zu Ihren KI-Tools wird hergestellt…
# machine-seeded
wizard-step-2 = Speicherort auswählen
# machine-seeded
wizard-step-3 = Mitgelieferte Komponenten werden installiert
# machine-seeded
wizard-step-4 = KI-IDEs werden automatisch verbunden
# machine-seeded
wizard-step-5 = Fast fertig!

## Installation outcomes
# machine-seeded
install-success = Amore erfolgreich installiert
# machine-seeded
install-error = Installation fehlgeschlagen: { $reason }
# machine-seeded
install-progress = Installiere… { $percent }%
# machine-seeded
install-components = Mitgelieferte Komponenten werden installiert…
# machine-seeded
install-ollama-wait = Ollama wurde installiert, ist aber nicht innerhalb von 60 Sekunden gestartet. Versuchen Sie, Ollama über das Startmenü zu öffnen.

## Uninstall
# machine-seeded
uninstall-confirm = Sind Sie sicher, dass Sie alle Amore-Daten löschen möchten?
# machine-seeded
uninstall-success = Amore erfolgreich deinstalliert
# machine-seeded
uninstall-in-progress = Amore wird entfernt…

## Memory operations
# machine-seeded
observe-success = Erinnerung gespeichert
# machine-seeded
observe-error = Erinnerung konnte nicht gespeichert werden: { $reason }
# machine-seeded
recall-empty = Keine Erinnerungen gefunden, die Ihrer Anfrage entsprechen
# machine-seeded
recall-results = { $count } { $count ->
    [one] Erinnerung gefunden
   *[other] Erinnerungen gefunden
 }
# machine-seeded
recall-query-placeholder = Erinnerungen durchsuchen…

## Tray menu
# machine-seeded
tray-tooltip = Amore — lokales KI-Gedächtnis
# machine-seeded
tray-open-dashboard = Dashboard öffnen
# machine-seeded
tray-pause = Pause
# machine-seeded
tray-resume = Fortsetzen
# machine-seeded
tray-recent-activity = Letzte Aktivität
# machine-seeded
tray-check-updates = Nach Updates suchen
# machine-seeded
tray-quit = Beenden

## Status / health
# machine-seeded
status-healthy = Amore ist aktiv
# machine-seeded
status-degraded = Amore läuft im eingeschränkten Modus
# machine-seeded
status-offline = Amore ist offline — überprüfen Sie Ollama und Qdrant
# machine-seeded
doctor-ok = Alle Systeme betriebsbereit
# machine-seeded
doctor-fail = Systemprüfung fehlgeschlagen: { $component } nicht erreichbar

## Error messages
# machine-seeded
error-network = Netzwerkfehler: { $reason }
# machine-seeded
error-disk-full = Festplatte voll — mindestens 500 MB freier Speicher erforderlich
# machine-seeded
error-permission-denied = Zugriff verweigert: { $path }
# machine-seeded
error-unknown = Ein unerwarteter Fehler ist aufgetreten
