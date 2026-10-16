# MercySF Reborn — état du développement

## V0.4

### Réseau / sessions

- Backend Tauri/Rust avec `sf-api` 0.4.1.
- Connexion compte S&F et ancien personnage.
- Sessions conservées uniquement en mémoire.
- `Update` manuel avec cache de 30 secondes.
- Toutes les commandes réseau récurrentes passent par `send_limited`.

### Protections

- intervalle global minimum : 5 secondes ;
- intervalle minimum par session : 5 secondes ;
- cooldown backend par module : 8 secondes minimum ;
- cooldown dynamique basé sur les timers S&F disponibles ;
- backoff exponentiel : 30 s à 15 min ;
- allowlist anti-champignons *fail closed* ;
- scheduler frontend sans polling réseau à chaque tick ;
- doubles exécutions d'un même module bloquées côté frontend ;
- commandes payantes connues explicitement refusées.

### Modules

- **Taverne** : réel, activé par défaut, choix XP/temps et protection inventaire.
- **Arène** : réel, activé par défaut, simulation locale et seuil de victoire.
- **Donjons** : réel, activé par défaut, simulation locale et Tour exclue.
- **Expéditions** : réel, désactivé par défaut, stratégie de choix encore simple.
- **Familiers** : réel, désactivé par défaut, sélection du meilleur familier nourrissable.
- **Forteresse** : réel, désactivé par défaut, heuristique de réserve de soldats.
- **Guilde** : réel, désactivé par défaut, participation attaque/défense, Hydre gratuite et portail quotidien.

### Validation

- tests unitaires Rust sur la politique anti-champignons ;
- vérifications statiques via `scripts/verify_safety.py` ;
- CI GitHub Actions :
  - `npm ci` ;
  - `npm run build` ;
  - vérification de sécurité Python ;
  - `cargo fmt --check` ;
  - `cargo clippy -D warnings` ;
  - `cargo test`.

### Prochaines améliorations

1. découper `src-tauri/src/main.rs` en modules sans modifier le comportement ;
2. isoler les verrous par session afin qu'un personnage ne bloque pas les autres pendant une requête ;
3. rendre les seuils de stratégie configurables tout en gardant les protections premium non configurables ;
4. améliorer le scoring des expéditions, familiers et attaques de forteresse ;
5. compléter les tests métier du scheduler et des stratégies.
