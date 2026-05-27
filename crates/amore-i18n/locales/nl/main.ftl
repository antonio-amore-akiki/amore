# nl/main.ftl — Dutch (Nederlands) translations for Amore.
# machine-seeded 2026-05-28. Mark every string with "# machine-seeded" for community-PR review.
# To refine: open a PR targeting this file with native-speaker corrections.

## Application identity
# machine-seeded
app-name = Amore
# machine-seeded
app-tagline = Lokaal-eerste persistente geheugen voor al uw AI-tools
# machine-seeded
app-version = Versie { $version }

## Wizard — navigation
# machine-seeded
wizard-next = Volgende
# machine-seeded
wizard-back = Terug
# machine-seeded
wizard-finish = Voltooien
# machine-seeded
wizard-cancel = Annuleren

## Wizard — screens
# machine-seeded
wizard-welcome = Welkom bij Amore — uw lokale AI-geheugenruggengraat
# machine-seeded
wizard-welcome-subtitle = Lokaal, privé en gratis. Geen cloud vereist.
# machine-seeded
wizard-step-1 = Verbinding maken met uw AI-tools…
# machine-seeded
wizard-step-2 = Locatie van het geheugen kiezen
# machine-seeded
wizard-step-3 = Meegeleverde componenten installeren
# machine-seeded
wizard-step-4 = AI-IDE's automatisch koppelen
# machine-seeded
wizard-step-5 = Bijna klaar!

## Installation outcomes
# machine-seeded
install-success = Amore succesvol geïnstalleerd
# machine-seeded
install-error = Installatie mislukt: { $reason }
# machine-seeded
install-progress = Installeren… { $percent }%
# machine-seeded
install-components = Meegeleverde componenten installeren…
# machine-seeded
install-ollama-wait = Ollama is geïnstalleerd maar startte niet binnen 60 seconden. Probeer Ollama te openen vanuit uw Startmenu.

## Uninstall
# machine-seeded
uninstall-confirm = Weet u zeker dat u alle Amore-gegevens wilt verwijderen?
# machine-seeded
uninstall-success = Amore succesvol verwijderd
# machine-seeded
uninstall-in-progress = Amore verwijderen…

## Memory operations
# machine-seeded
observe-success = Geheugen opgeslagen
# machine-seeded
observe-error = Geheugen opslaan mislukt: { $reason }
# machine-seeded
recall-empty = Geen herinneringen gevonden die overeenkomen met uw zoekopdracht
# machine-seeded
recall-results = { $count } { $count ->
    [one] herinnering gevonden
   *[other] herinneringen gevonden
 }
# machine-seeded
recall-query-placeholder = Zoek in uw herinneringen…

## Tray menu
# machine-seeded
tray-tooltip = Amore — lokaal AI-geheugen
# machine-seeded
tray-open-dashboard = Dashboard openen
# machine-seeded
tray-pause = Pauzeren
# machine-seeded
tray-resume = Hervatten
# machine-seeded
tray-recent-activity = Recente activiteit
# machine-seeded
tray-check-updates = Controleren op updates
# machine-seeded
tray-quit = Afsluiten

## Status / health
# machine-seeded
status-healthy = Amore is actief
# machine-seeded
status-degraded = Amore werkt in gedegradeerde modus
# machine-seeded
status-offline = Amore is offline — controleer Ollama en Qdrant
# machine-seeded
doctor-ok = Alle systemen operationeel
# machine-seeded
doctor-fail = Gezondheidscontrole mislukt: { $component } onbereikbaar

## Error messages
# machine-seeded
error-network = Netwerkfout: { $reason }
# machine-seeded
error-disk-full = Schijf vol — minimaal 500 MB vrije ruimte vereist
# machine-seeded
error-permission-denied = Toegang geweigerd: { $path }
# machine-seeded
error-unknown = Er is een onverwachte fout opgetreden
