# MercySF Reborn

Réimplémentation indépendante d'un client d'automatisation Shakes & Fidget.

## V0.3 — automatisations prudentes

Cette version active réellement **Taverne**, **Arène** et **Donjons** depuis le backend Tauri/Rust via `sf-api` 0.4.1, tout en imposant plusieurs protections pour éviter le polling agressif.

### Connexions prises en charge

- **Compte S&F** : e-mail/identifiant + mot de passe ; récupère les personnages liés au compte.
- **Ancien personnage** : nom du personnage + mot de passe + serveur, par exemple `f1.sfgame.net`.

Les identifiants ne sont pas enregistrés dans `localStorage`. Les sessions restent en mémoire côté Rust.

## Protection anti-spam

La sécurité réseau n'est pas confiée uniquement à l'interface : elle est appliquée dans le backend Rust.

- minimum **5 secondes entre deux requêtes S&F**, globalement et par session ;
- cooldown propre à chaque module ;
- le scheduler UI ne lance un module que lorsque son `nextRunAt` est atteint ;
- clics manuels répétés ignorés tant que le module n'est pas arrivé à échéance ;
- une seule exécution simultanée par module côté UI ;
- `sf_automation_cycle` regroupe décision + exécution pour éviter un cycle `decide()` / `execute()` générant des synchronisations doublées ;
- `sf_refresh_character` utilise un cache de **30 secondes** ;
- backoff exponentiel après erreur : 30 s, 60 s, 120 s... jusqu'à 15 min ;
- lorsqu'un timer du jeu est connu, aucun polling n'est effectué avant son expiration.

Toutes les commandes réseau récurrentes du backend passent par une seule fonction `send_limited`.

## Modules

### Taverne

- attend localement la fin d'une quête sans requête intermédiaire ;
- termine la quête uniquement après le timer ;
- choisit la quête au meilleur ratio XP/temps ;
- n'écrase jamais un objet d'inventaire (`overwrite_inv = false`) ;
- si l'inventaire est plein, évite les quêtes donnant un objet ;
- ne force pas encore le basculement quête/expédition.

### Arène

- n'envoie pas `CheckArena` avant le cooldown gratuit ;
- utilise le premier adversaire valide afin de ne pas multiplier les requêtes d'analyse ;
- ne demande jamais volontairement l'usage d'un champignon (`use_mushroom = false`) ;
- applique une **allowlist backend anti-champignons** : seules les commandes explicitement auditées comme gratuites peuvent atteindre le serveur ;
- bloque par défaut toute nouvelle commande non auditée (politique *fail closed*) ;
- revérifie le timer avant d'envoyer le combat.

### Donjons

- ne rafraîchit pas les donjons pendant un cooldown connu ;
- utilise `UpdateDungeons` uniquement lorsque le combat est potentiellement disponible ;
- revérifie ensuite le cooldown avant `FightDungeon` ;
- choisit le premier donjon standard ouvert et ignore la Tour ;
- ne demande jamais volontairement l'usage d'un champignon.

## Lancer

Prérequis : Node.js, Rust, Cargo et les prérequis système Tauri 2.

```bash
npm install
npm run dev
```

Pour le desktop, lancer ensuite l'application avec le CLI Tauri de l'environnement.

## Important

L'automatisation d'un jeu peut contrevenir à ses conditions d'utilisation et son protocole peut changer. Cette application est une réimplémentation indépendante et doit être utilisée en connaissance de cause.
