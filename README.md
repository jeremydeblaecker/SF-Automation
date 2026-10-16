# MercySF Reborn

Réimplémentation indépendante d'un client d'automatisation Shakes & Fidget.

## V0.4 — automatisations prudentes

Le backend Tauri/Rust s'appuie sur `sf-api` 0.4.1. Les modules **Taverne**, **Arène** et **Donjons** sont activés par défaut ; **Expéditions**, **Familiers** et **Forteresse** sont disponibles mais restent désactivés par défaut afin de conserver un comportement prudent.

### Connexions prises en charge

- **Compte S&F** : e-mail/identifiant + mot de passe ; récupère les personnages liés au compte.
- **Ancien personnage** : nom du personnage + mot de passe + serveur, par exemple `f1.sfgame.net`.

Les identifiants ne sont pas enregistrés dans `localStorage`. Les sessions restent en mémoire côté Rust.

## Protection anti-spam et anti-premium

La sécurité réseau est imposée par le backend :

- minimum **5 secondes entre deux requêtes S&F**, globalement et par session ;
- cooldown propre à chaque module et respect des timers connus du jeu ;
- cache de 30 secondes pour le rafraîchissement manuel ;
- backoff exponentiel après erreur, de 30 secondes jusqu'à 15 minutes ;
- déduplication des actions en vol dans le scheduler ;
- une seule porte de sortie réseau récurrente : `send_limited` ;
- allowlist *fail closed* : une commande non auditée est bloquée ;
- aucun combat automatique ne demande volontairement de champignon ;
- aucun skip de quête ou d'expédition payant n'est autorisé.

## Modules

### Taverne

- attend localement la fin d'une quête ;
- sélectionne la quête au meilleur ratio XP/temps ;
- ne remplace jamais un objet d'inventaire ;
- évite les quêtes avec objet lorsque l'inventaire est plein.

### Arène

- attend le cooldown gratuit avant `CheckArena` ;
- simule localement le combat ;
- combat uniquement au-dessus du seuil de victoire configuré dans le backend ;
- envoie toujours `use_mushroom = false`.

### Donjons

- actualise les donjons uniquement lorsque nécessaire ;
- simule le combat lorsqu'un monstre exploitable est disponible ;
- ignore la Tour ;
- envoie toujours `use_mushroom = false`.

### Expéditions

- démarre une expédition disponible ;
- progresse dans les étapes sans payer pour raccourcir l'attente ;
- choisit actuellement le premier embranchement ou la première récompense disponible.

### Familiers

- nourrit un familier déjà possédé lorsqu'un fruit est disponible ;
- privilégie actuellement le familier de plus haut niveau pouvant être nourri.

### Forteresse

- n'utilise pas de champignon ;
- conserve une réserve de soldats avant une attaque ;
- utilise une heuristique prudente car `sf-api` n'expose pas de simulation fiable de l'armée adverse.

## Qualité et vérifications

Le dépôt contient désormais :

- des tests Rust sur la politique anti-champignons ;
- le script `scripts/verify_safety.py` pour les invariants statiques ;
- une CI GitHub Actions qui compile le frontend, exécute les vérifications de sécurité, `cargo fmt`, `cargo clippy` et `cargo test` ;
- des versions de dépendances explicites plutôt que `latest`.

## Lancer

Prérequis : Node.js 22+, Rust, Cargo et les prérequis système Tauri 2.

```bash
npm ci
npm run dev
```

Pour le desktop :

```bash
npm run tauri dev
```

## Important

L'automatisation d'un jeu peut contrevenir à ses conditions d'utilisation et son protocole peut changer. Cette application est une réimplémentation indépendante et doit être utilisée en connaissance de cause.
