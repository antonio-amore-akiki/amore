# fr/main.ftl — French translations for Amore.
# machine-seeded 2026-05-28. Mark every string with "# machine-seeded" for community-PR review.
# To refine: open a PR targeting this file with native-speaker corrections.

## Application identity
# machine-seeded
app-name = Amore
# machine-seeded
app-tagline = Mémoire persistante locale pour tous vos outils IA
# machine-seeded
app-version = Version { $version }

## Wizard — navigation
# machine-seeded
wizard-next = Suivant
# machine-seeded
wizard-back = Retour
# machine-seeded
wizard-finish = Terminer
# machine-seeded
wizard-cancel = Annuler

## Wizard — screens
# machine-seeded
wizard-welcome = Bienvenue dans Amore — votre mémoire IA locale
# machine-seeded
wizard-welcome-subtitle = Local, privé et gratuit. Aucun cloud requis.
# machine-seeded
wizard-step-1 = Connexion à vos outils IA…
# machine-seeded
wizard-step-2 = Choix de l'emplacement de la mémoire
# machine-seeded
wizard-step-3 = Installation des composants inclus
# machine-seeded
wizard-step-4 = Connexion automatique des IDE IA
# machine-seeded
wizard-step-5 = Presque terminé !

## Installation outcomes
# machine-seeded
install-success = Amore installé avec succès
# machine-seeded
install-error = Échec de l'installation : { $reason }
# machine-seeded
install-progress = Installation… { $percent } %
# machine-seeded
install-components = Installation des composants inclus…
# machine-seeded
install-ollama-wait = Ollama est installé mais n'a pas démarré dans les 60 secondes. Essayez d'ouvrir Ollama depuis votre menu Démarrer.

## Uninstall
# machine-seeded
uninstall-confirm = Êtes-vous sûr de vouloir supprimer toutes les données Amore ?
# machine-seeded
uninstall-success = Amore désinstallé avec succès
# machine-seeded
uninstall-in-progress = Suppression d'Amore…

## Memory operations
# machine-seeded
observe-success = Mémoire enregistrée
# machine-seeded
observe-error = Échec de l'enregistrement de la mémoire : { $reason }
# machine-seeded
recall-empty = Aucune mémoire correspondant à votre requête
# machine-seeded
recall-results = { $count } { $count ->
    [one] mémoire trouvée
   *[other] mémoires trouvées
 }
# machine-seeded
recall-query-placeholder = Rechercher dans vos souvenirs…

## Tray menu
# machine-seeded
tray-tooltip = Amore — mémoire IA locale
# machine-seeded
tray-open-dashboard = Ouvrir le tableau de bord
# machine-seeded
tray-pause = Pause
# machine-seeded
tray-resume = Reprendre
# machine-seeded
tray-recent-activity = Activité récente
# machine-seeded
tray-check-updates = Vérifier les mises à jour
# machine-seeded
tray-quit = Quitter

## Status / health
# machine-seeded
status-healthy = Amore est en cours d'exécution
# machine-seeded
status-degraded = Amore fonctionne en mode dégradé
# machine-seeded
status-offline = Amore est hors ligne — vérifiez Ollama et Qdrant
# machine-seeded
doctor-ok = Tous les systèmes opérationnels
# machine-seeded
doctor-fail = Bilan de santé échoué : { $component } inaccessible

## Error messages
# machine-seeded
error-network = Erreur réseau : { $reason }
# machine-seeded
error-disk-full = Disque plein — au moins 500 Mo requis
# machine-seeded
error-permission-denied = Permission refusée : { $path }
# machine-seeded
error-unknown = Une erreur inattendue s'est produite
