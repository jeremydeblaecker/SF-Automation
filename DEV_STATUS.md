# MercySF Reborn — état du développement

## V0.3

### Réseau / sessions

- Backend Tauri/Rust avec `sf-api` 0.4.1.
- Connexion compte S&F et ancien personnage.
- Sessions uniquement en mémoire.
- `Update` manuel avec cache de 30 secondes.
- Une seule porte de sortie pour les commandes récurrentes : `send_limited`.

### Anti-spam

- intervalle global minimum : 5 secondes ;
- intervalle minimum par session : 5 secondes ;
- cooldown backend par module : 8 secondes minimum ;
- cooldown dynamique basé sur les vrais timers S&F quand ils sont disponibles ;
- déduplication des actions en vol dans le scheduler ;
- `runNow` ignore les clics avant `nextRunAt` ;
- backoff réseau après erreur : 30 secondes, exponentiel, plafond 15 minutes ;
- scheduler à tick 1 s mais **sans requête à chaque tick** : un appel backend n'est fait que si un module est arrivé à échéance ;
- backend capable de répondre « cooldown local » sans contacter S&F si l'UI appelle trop tôt.

### Taverne

- `StartQuest` réel ;
- `FinishQuest` réel ;
- attente basée sur `busy_until` ;
- sélection XP/temps ;
- `overwrite_inv = false` ;
- protection inventaire plein.

### Arène

- `CheckArena` réel uniquement lorsque le cooldown local est expiré ;
- `Fight` réel ;
- `use_mushroom = false` ;
- revalidation de `next_free_fight` avant le combat ;
- premier adversaire valide pour limiter le nombre de consultations.

### Donjons

- `UpdateDungeons` réel seulement lorsque nécessaire ;
- `FightDungeon` réel ;
- revalidation du cooldown après `UpdateDungeons` ;
- sélection du premier donjon standard ouvert ;
- Tour exclue ;
- `use_mushroom = false`.

## Validation disponible dans cet environnement

Le conteneur de génération n'a pas Rust/Cargo installé et les dépendances npm React ne sont pas présentes, donc un build Tauri complet ne peut pas être certifié ici. Des vérifications statiques de sécurité sont néanmoins exécutées avant l'archive V0.3 : unicité du point `send_command`, valeurs anti-spam, absence d'usage volontaire de champignons et protections scheduler.


## V0.3.1 — verrou anti-champignons

- allowlist stricte dans `send_limited` ;
- seules `Update`, `CheckArena`, `UpdateDungeons`, `ViewPlayer`, `StartQuest(overwrite_inv=false)`, `FinishQuest(skip=None)`, `Fight(use_mushroom=false)` et `FightDungeon(use_mushroom=false)` sont autorisées ;
- toute autre commande est bloquée localement avant tout appel réseau ;
- politique *fail closed* : une future fonctionnalité ne peut pas dépenser de champignons par accident sans modifier explicitement cette allowlist.
