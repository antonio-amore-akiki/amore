# en/main.ftl — English source-of-truth strings for Amore.
# All other locales must contain every key defined here (enforced by locale_load test).
# To add a string: add it here first, then add machine-seeded translations to all other locales.

## Application identity
app-name = Amore
app-tagline = Local-first persistent memory for every AI tool
app-version = Version { $version }

## Wizard — navigation
wizard-next = Next
wizard-back = Back
wizard-finish = Finish
wizard-cancel = Cancel

## Wizard — screens
wizard-welcome = Welcome to Amore — your local AI memory backbone
wizard-welcome-subtitle = Local-first, private, and free. No cloud required.
wizard-step-1 = Connecting to your AI tools…
wizard-step-2 = Choosing your memory location
wizard-step-3 = Installing bundled components
wizard-step-4 = Wiring AI IDEs automatically
wizard-step-5 = Almost done!

## Installation outcomes
install-success = Amore installed successfully
install-error = Installation failed: { $reason }
install-progress = Installing… { $percent }%
install-components = Installing bundled components…
install-ollama-wait = Ollama installed but didn't start within 60 seconds. Try opening Ollama from your Start menu.

## Uninstall
uninstall-confirm = Are you sure you want to delete all Amore data?
uninstall-success = Amore uninstalled successfully
uninstall-in-progress = Removing Amore…

## Memory operations
observe-success = Memory saved
observe-error = Failed to save memory: { $reason }
recall-empty = No memories found matching your query
recall-results = { $count } { $count ->
    [one] memory
   *[other] memories
 } found
recall-query-placeholder = Search your memories…

## Tray menu
tray-tooltip = Amore — local AI memory
tray-open-dashboard = Open dashboard
tray-pause = Pause
tray-resume = Resume
tray-recent-activity = Recent activity
tray-check-updates = Check for updates
tray-quit = Quit

## Status / health
status-healthy = Amore is running
status-degraded = Amore is running in degraded mode
status-offline = Amore is offline — check Ollama and Qdrant
doctor-ok = All systems operational
doctor-fail = Health check failed: { $component } unreachable

## Error messages
error-network = Network error: { $reason }
error-disk-full = Disk full — need at least 500 MB free
error-permission-denied = Permission denied: { $path }
error-unknown = An unexpected error occurred
