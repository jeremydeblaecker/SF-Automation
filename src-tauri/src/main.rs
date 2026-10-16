#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use serde::Serialize;
use serde_json::Value;
use sf_api::{
    command::{Command, FortunePayment},
    gamestate::{
        dungeons::{Dungeon, DungeonProgress, LightDungeon},
        fortress::FortressBuildingType,
        tavern::{AvailableTasks, CurrentAction, ExpeditionStage},
        unlockables::HabitatType,
        GameState,
    },
    session::SimpleSession,
    simulate::{simulate_battle, Fighter, PlayerFighterSquad, UpgradeableFighter},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::State;
use tokio::sync::Mutex;

const MIN_GLOBAL_REQUEST_INTERVAL: Duration = Duration::from_secs(5);
const REFRESH_CACHE_TTL: Duration = Duration::from_secs(30);
const MIN_MODULE_RECHECK: Duration = Duration::from_secs(8);
const DEFAULT_IDLE_RECHECK: Duration = Duration::from_secs(5 * 60);

/// Un combat n'est tenté que si la simulation locale (basée sur le moteur de
/// combat de sf-api, le même que celui du jeu) prédit au moins ce taux de
/// victoire. Réglable : plus bas = plus agressif, plus haut = plus prudent.
const MIN_WIN_PROBABILITY: f64 = 0.60;

/// Nombre de combats simulés localement pour estimer le taux de victoire.
/// Aucune requête réseau n'est impliquée : tout se passe en mémoire côté Rust.
const FIGHT_SIMULATION_ITERATIONS: u32 = 300;

/// Nombre minimum de soldats de forteresse à conserver en réserve avant
/// d'en engager dans une attaque automatique (protection contre le vidage
/// accidentel de l'armée si la simulation d'ennemi n'est pas favorable).
const FORTRESS_MIN_SOLDIER_RESERVE_RATIO: f64 = 0.5;

#[derive(Default)]
struct SessionStore {
    sessions: Arc<Mutex<Vec<ConnectedSession>>>,
    last_global_request: Arc<Mutex<Option<Instant>>>,
}

struct ConnectedSession {
    id: String,
    session: SimpleSession,
    last_request_at: Option<Instant>,
    last_sync_at: Option<Instant>,
    error_streak: u32,
    backoff_until: Option<Instant>,
    module_cooldowns: HashMap<String, Instant>,
}

impl ConnectedSession {
    fn new(id: String, session: SimpleSession) -> Self {
        Self {
            id,
            session,
            last_request_at: None,
            last_sync_at: Some(Instant::now()),
            error_streak: 0,
            backoff_until: None,
            module_cooldowns: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CharacterSummary {
    id: String,
    name: String,
    server: String,
    level: u64,
    class_name: String,
    online: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DungeonSimulationResult {
    dungeon: String,
    iterations: u32,
    win_ratio: f64,
    threshold: f64,
    recommended: bool,
    recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutomationCycleResult {
    action: String,
    reason: String,
    message: String,
    cooldown_ms: u64,
    did_request: bool,
}

impl AutomationCycleResult {
    fn idle(action: &str, reason: impl Into<String>, cooldown: Duration) -> Self {
        Self {
            action: action.to_string(),
            reason: reason.into(),
            message: "Aucune requête S&F envoyée.".to_string(),
            cooldown_ms: duration_ms(cooldown),
            did_request: false,
        }
    }

    fn sent(
        action: &str,
        reason: impl Into<String>,
        message: impl Into<String>,
        cooldown: Duration,
    ) -> Self {
        Self {
            action: action.to_string(),
            reason: reason.into(),
            message: message.into(),
            cooldown_ms: duration_ms(cooldown),
            did_request: true,
        }
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn chrono_wait(until: chrono::DateTime<Local>) -> Duration {
    until
        .signed_duration_since(Local::now())
        .to_std()
        .unwrap_or_default()
        .saturating_add(Duration::from_secs(2))
        .max(MIN_MODULE_RECHECK)
}

/// Estime le taux de victoire contre `opponent` en simulant `n` combats en
/// mémoire, localement, avec le moteur de combat de sf-api (aucune requête
/// réseau). `is_arena_battle` doit être vrai pour l'arène (tour de jeu
/// différent des donjons).
fn estimate_win_ratio(state: &GameState, opponent: Fighter, is_arena_battle: bool) -> f64 {
    estimate_win_ratio_with_iterations(state, opponent, is_arena_battle, FIGHT_SIMULATION_ITERATIONS)
}

fn estimate_win_ratio_with_iterations(state: &GameState, opponent: Fighter, is_arena_battle: bool, iterations: u32) -> f64 {
    let squad = PlayerFighterSquad::new(state);
    let own = Fighter::from(&squad.character);
    simulate_battle(&[own], &[opponent], iterations, is_arena_battle).win_ratio
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |current, key| current.get(*key))
}

fn first_u64(value: &Value, paths: &[&[&str]]) -> u64 {
    paths
        .iter()
        .find_map(|path| value_at(value, path).and_then(Value::as_u64))
        .unwrap_or_default()
}

fn first_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let found = value_at(value, path)?;
        found
            .as_str()
            .map(ToOwned::to_owned)
            .or_else(|| found.as_u64().map(|v| v.to_string()))
    })
}

fn summarize(id: String, session: &SimpleSession) -> CharacterSummary {
    let state_json = session
        .game_state()
        .and_then(|gs| serde_json::to_value(gs).ok())
        .unwrap_or(Value::Null);

    let level = first_u64(
        &state_json,
        &[&["character", "level"], &["character", "lvl"], &["level"]],
    );

    let class_name = first_string(
        &state_json,
        &[
            &["character", "class"],
            &["character", "class_name"],
            &["class"],
        ],
    )
    .unwrap_or_else(|| "Classe S&F".to_string());

    CharacterSummary {
        id,
        name: session.username().to_string(),
        server: session
            .server_url()
            .host_str()
            .unwrap_or("serveur S&F")
            .to_string(),
        level,
        class_name,
        online: session.has_session_id(),
    }
}

async fn wait_global_slot(store: &SessionStore) {
    let mut last = store.last_global_request.lock().await;
    if let Some(previous) = *last {
        let elapsed = previous.elapsed();
        if elapsed < MIN_GLOBAL_REQUEST_INTERVAL {
            tokio::time::sleep(MIN_GLOBAL_REQUEST_INTERVAL - elapsed).await;
        }
    }
    *last = Some(Instant::now());
}

fn is_mushroom_safe_command(command: &Command) -> bool {
    match command {
        // Lecture / synchronisation : aucune dépense premium.
        Command::Update
        | Command::CheckArena
        | Command::UpdateDungeons
        | Command::ViewPlayer { .. } => true,

        // Quêtes : jamais de skip et jamais d'écrasement d'inventaire.
        Command::StartQuest {
            overwrite_inv: false,
            ..
        } => true,
        Command::FinishQuest { skip: None } => true,

        // Combats : le backend refuse catégoriquement le mode champignon.
        Command::Fight {
            use_mushroom: false,
            ..
        } => true,
        Command::FightDungeon {
            use_mushroom: false,
            ..
        } => true,

        // Expéditions : progression normale uniquement. ExpeditionSkipWait
        // n'est délibérément PAS autorisé, car il dépense des sabliers ou
        // des champignons pour sauter l'attente.
        Command::ExpeditionStart { .. }
        | Command::ExpeditionContinue
        | Command::ExpeditionPickEncounter { .. }
        | Command::ExpeditionPickReward { .. } => true,

        // Familiers : nourrir ne coûte que des fruits déjà en inventaire,
        // jamais de champignons.
        Command::PetFeed { .. } => true,

        // Forteresse : l'attaque consomme des soldats déjà entraînés (pas de
        // monnaie premium). Le nouvel adversaire gratuit est autorisé, mais
        // PAS le reroll payant (use_mushroom: true).
        Command::FortressAttack { .. } => true,
        Command::FortressNewEnemy { use_mushroom: false } => true,
        Command::FortressGemStoneSearch => true,
        Command::FortressGemStoneSearchFinish { mushrooms: 0 } => true,

        // Roue : seuls le tour gratuit et les Lucky Coins sont autorisés.
        // Le paiement en champignons reste bloqué par défaut.
        Command::SpinWheelOfFortune {
            payment: FortunePayment::FreeTurn | FortunePayment::LuckyCoins,
        } => true,

        // Guilde : participation personnelle gratuite uniquement. Le lancement
        // d'une attaque ou d'un raid reste une décision manuelle d'officier.
        Command::GuildJoinAttack
        | Command::GuildJoinDefense
        | Command::GuildPortalBattle => true,
        Command::GuildPetBattle { use_mushroom: false } => true,

        // Fail closed : toute nouvelle commande doit être auditée puis ajoutée
        // explicitement à cette allowlist avant de pouvoir partir au serveur.
        _ => false,
    }
}

async fn send_limited(
    connected: &mut ConnectedSession,
    store: &SessionStore,
    command: Command,
) -> Result<(), String> {
    if !is_mushroom_safe_command(&command) {
        return Err(
            "Commande bloquée par la protection anti-champignons : cette action n'est pas explicitement autorisée comme gratuite."
                .to_string(),
        );
    }
    if let Some(until) = connected.backoff_until {
        if until > Instant::now() {
            let wait = until.duration_since(Instant::now()).as_secs().max(1);
            return Err(format!(
                "Protection anti-spam active après erreur. Nouvelle tentative dans {wait}s."
            ));
        }
    }

    if let Some(previous) = connected.last_request_at {
        let elapsed = previous.elapsed();
        if elapsed < MIN_GLOBAL_REQUEST_INTERVAL {
            tokio::time::sleep(MIN_GLOBAL_REQUEST_INTERVAL - elapsed).await;
        }
    }

    wait_global_slot(store).await;
    connected.last_request_at = Some(Instant::now());

    match connected.session.send_command(command).await {
        Ok(_) => {
            connected.last_sync_at = Some(Instant::now());
            connected.error_streak = 0;
            connected.backoff_until = None;
            Ok(())
        }
        Err(error) => {
            connected.error_streak = connected.error_streak.saturating_add(1);
            let exponent = connected.error_streak.saturating_sub(1).min(5);
            let seconds = 30_u64.saturating_mul(1_u64 << exponent).min(15 * 60);
            connected.backoff_until = Some(Instant::now() + Duration::from_secs(seconds));
            Err(format!(
                "Requête S&F refusée/échouée: {error}. Pause automatique de {seconds}s."
            ))
        }
    }
}

fn module_guard(connected: &ConnectedSession, module: &str) -> Option<Duration> {
    connected
        .module_cooldowns
        .get(module)
        .copied()
        .filter(|until| *until > Instant::now())
        .map(|until| until.duration_since(Instant::now()))
}

fn remember_module_cooldown(
    connected: &mut ConnectedSession,
    module: &str,
    cooldown: Duration,
) {
    connected.module_cooldowns.insert(
        module.to_string(),
        Instant::now() + cooldown.max(MIN_MODULE_RECHECK),
    );
}

fn lookup_player_name(value: &Value, target_id: u32) -> Option<String> {
    match value {
        Value::Object(map) => {
            let object_id = map
                .get("player_id")
                .or_else(|| map.get("playerId"))
                .or_else(|| map.get("id"))
                .and_then(Value::as_u64);
            if object_id == Some(u64::from(target_id)) {
                if let Some(name) = map.get("name").and_then(Value::as_str) {
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
            map.values()
                .find_map(|child| lookup_player_name(child, target_id))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|child| lookup_player_name(child, target_id)),
        _ => None,
    }
}

async fn run_tavern(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État S&F indisponible".to_string())?;

    match state.tavern.current_action {
        CurrentAction::Quest { busy_until, .. } if busy_until > Local::now() => {
            return Ok(AutomationCycleResult::idle(
                "Attendre la fin de la quête",
                "Une quête est déjà en cours; aucun polling n'est nécessaire.",
                chrono_wait(busy_until),
            ));
        }
        CurrentAction::Quest { .. } => {
            send_limited(
                connected,
                store,
                Command::FinishQuest { skip: None },
            )
            .await?;
            return Ok(AutomationCycleResult::sent(
                "Terminer la quête",
                "Le timer local indique que la quête est terminée.",
                "Quête terminée et combat de quête récupéré.",
                Duration::from_secs(12),
            ));
        }
        CurrentAction::Idle => {}
        CurrentAction::Expedition => {
            return Ok(AutomationCycleResult::idle(
                "Attendre l'expédition",
                "Une expédition est en cours; ce cycle ne force aucune action.",
                DEFAULT_IDLE_RECHECK,
            ));
        }
        CurrentAction::CityGuard { busy_until, .. } => {
            let cooldown = if busy_until > Local::now() {
                chrono_wait(busy_until)
            } else {
                DEFAULT_IDLE_RECHECK
            };
            return Ok(AutomationCycleResult::idle(
                "Attendre la garde",
                "Le personnage travaille actuellement.",
                cooldown,
            ));
        }
        CurrentAction::Unknown(until) => {
            let cooldown = until
                .filter(|date| *date > Local::now())
                .map(chrono_wait)
                .unwrap_or(DEFAULT_IDLE_RECHECK);
            return Ok(AutomationCycleResult::idle(
                "Attendre",
                "Le personnage effectue une action non reconnue; aucune requête risquée n'est envoyée.",
                cooldown,
            ));
        }
    }

    if state.tavern.thirst_for_adventure_sec == 0 {
        return Ok(AutomationCycleResult::idle(
            "Repos",
            "Plus de soif d'aventure disponible aujourd'hui.",
            Duration::from_secs(15 * 60),
        ));
    }

    let AvailableTasks::Quests(quests) = state.tavern.available_tasks() else {
        return Ok(AutomationCycleResult::idle(
            "Expéditions disponibles",
            "Le compte préfère actuellement les expéditions; la V0.3 ne bascule pas ce réglage automatiquement.",
            DEFAULT_IDLE_RECHECK,
        ));
    };

    let has_free_slot = state.character.inventory.count_free_slots() > 0;
    let selected = quests
        .iter()
        .enumerate()
        .filter(|(_, quest)| has_free_slot || quest.item.is_none())
        .filter(|(_, quest)| quest.base_length > 0)
        .max_by(|(_, left), (_, right)| {
            let left_score = u64::from(left.base_experience) * u64::from(right.base_length);
            let right_score = u64::from(right.base_experience) * u64::from(left.base_length);
            left_score.cmp(&right_score)
        })
        .map(|(index, _)| index);

    let Some(quest_pos) = selected else {
        return Ok(AutomationCycleResult::idle(
            "Inventaire plein",
            "Aucune quête sûre sans récompense d'objet; overwrite_inv reste volontairement à false.",
            Duration::from_secs(10 * 60),
        ));
    };

    send_limited(
        connected,
        store,
        Command::StartQuest {
            quest_pos,
            overwrite_inv: false,
        },
    )
    .await?;

    let cooldown = connected
        .session
        .game_state()
        .and_then(|gs| match gs.tavern.current_action {
            CurrentAction::Quest { busy_until, .. } if busy_until > Local::now() => {
                Some(chrono_wait(busy_until))
            }
            _ => None,
        })
        .unwrap_or(Duration::from_secs(60));

    Ok(AutomationCycleResult::sent(
        "Démarrer une quête",
        "Quête choisie selon le meilleur ratio XP/temps, sans écraser l'inventaire.",
        format!("Quête {} démarrée.", quest_pos + 1),
        cooldown,
    ))
}

async fn run_arena(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    if let Some(next_free) = connected
        .session
        .game_state()
        .and_then(|gs| gs.arena.next_free_fight)
        .filter(|date| *date > Local::now())
    {
        return Ok(AutomationCycleResult::idle(
            "Attendre l'arène",
            "Le combat gratuit n'est pas encore disponible; aucun CheckArena n'est envoyé.",
            chrono_wait(next_free),
        ));
    }

    send_limited(connected, store, Command::CheckArena).await?;

    let (target_id, own_id) = {
        let state = connected
            .session
            .game_state()
            .ok_or_else(|| "État de l'arène indisponible".to_string())?;
        let own_id = state.character.player_id;
        let target = state
            .arena
            .enemy_ids
            .iter()
            .copied()
            .find(|id| *id != 0 && *id != own_id)
            .ok_or_else(|| "Aucun adversaire d'arène disponible".to_string())?;
        (target, own_id)
    };

    if target_id == own_id {
        return Err("La cible d'arène correspond au personnage actif".to_string());
    }

    let cached_name = connected
        .session
        .game_state()
        .and_then(|state| serde_json::to_value(state).ok())
        .and_then(|json| lookup_player_name(&json, target_id));

    let target_name = if let Some(name) = cached_name {
        name
    } else {
        send_limited(
            connected,
            store,
            Command::ViewPlayer {
                ident: target_id.to_string(),
            },
        )
        .await?;
        let json = connected
            .session
            .game_state()
            .and_then(|state| serde_json::to_value(state).ok())
            .ok_or_else(|| "Impossible de lire la cible d'arène".to_string())?;
        lookup_player_name(&json, target_id)
            .ok_or_else(|| "Nom de l'adversaire d'arène introuvable".to_string())?
    };

    if let Some(next_free) = connected
        .session
        .game_state()
        .and_then(|gs| gs.arena.next_free_fight)
        .filter(|date| *date > Local::now())
    {
        return Ok(AutomationCycleResult::sent(
            "Analyser l'arène",
            "Le cooldown a été réévalué après lecture des adversaires; le combat est annulé par sécurité.",
            "Adversaires synchronisés, aucun combat envoyé.",
            chrono_wait(next_free),
        ));
    }

    let win_ratio = {
        let state = connected
            .session
            .game_state()
            .ok_or_else(|| "État de l'arène indisponible pour la simulation".to_string())?;
        let opponent = state
            .lookup
            .lookup_pid(target_id)
            .ok_or_else(|| "Données de l'adversaire d'arène introuvables pour la simulation".to_string())?;
        let opponent_fighter = Fighter::from(&UpgradeableFighter::from_other(opponent));
        estimate_win_ratio(state, opponent_fighter, true)
    };

    if win_ratio < MIN_WIN_PROBABILITY {
        return Ok(AutomationCycleResult::sent(
            "Simuler le combat d'arène",
            format!(
                "Taux de victoire simulé de {:.0}% contre {target_name}, sous le seuil de {:.0}%; combat annulé.",
                win_ratio * 100.0,
                MIN_WIN_PROBABILITY * 100.0
            ),
            "Adversaire jugé trop fort, aucun combat envoyé.",
            Duration::from_secs(10 * 60),
        ));
    }

    send_limited(
        connected,
        store,
        Command::Fight {
            name: target_name.clone(),
            use_mushroom: false,
        },
    )
    .await?;

    let cooldown = connected
        .session
        .game_state()
        .and_then(|gs| gs.arena.next_free_fight)
        .filter(|date| *date > Local::now())
        .map(chrono_wait)
        .unwrap_or(Duration::from_secs(10 * 60));

    Ok(AutomationCycleResult::sent(
        "Combattre dans l'arène",
        format!(
            "Premier adversaire valide utilisé; taux de victoire simulé de {:.0}% (seuil {:.0}%). Aucun champignon n'est demandé.",
            win_ratio * 100.0,
            MIN_WIN_PROBABILITY * 100.0
        ),
        format!("Combat d'arène lancé contre {target_name}."),
        cooldown,
    ))
}

fn first_open_dungeon(state: &sf_api::gamestate::GameState) -> Option<Dungeon> {
    for (dungeon, progress) in state.dungeons.light.iter() {
        if dungeon != LightDungeon::Tower && matches!(*progress, DungeonProgress::Open { .. }) {
            return Some(Dungeon::Light(dungeon));
        }
    }
    for (dungeon, progress) in state.dungeons.shadow.iter() {
        if matches!(*progress, DungeonProgress::Open { .. }) {
            return Some(Dungeon::Shadow(dungeon));
        }
    }
    None
}

async fn run_dungeons(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    if let Some(next_free) = connected
        .session
        .game_state()
        .and_then(|gs| gs.dungeons.next_free_fight)
        .filter(|date| *date > Local::now())
    {
        return Ok(AutomationCycleResult::idle(
            "Attendre le donjon",
            "Le prochain combat gratuit n'est pas encore disponible; aucun rafraîchissement de donjon n'est envoyé.",
            chrono_wait(next_free),
        ));
    }

    // sf-api recommande explicitement cette commande avant FightDungeon, car
    // Update/FightDungeon ne garantissent pas la fraîcheur du timer/progrès.
    send_limited(connected, store, Command::UpdateDungeons).await?;

    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État des donjons indisponible".to_string())?;

    if let Some(next_free) = state.dungeons.next_free_fight.filter(|date| *date > Local::now()) {
        return Ok(AutomationCycleResult::sent(
            "Actualiser les donjons",
            "UpdateDungeons a révélé un cooldown encore actif; le combat est annulé.",
            "Donjons synchronisés, aucun combat envoyé.",
            chrono_wait(next_free),
        ));
    }

    let Some(dungeon) = first_open_dungeon(state) else {
        return Ok(AutomationCycleResult::sent(
            "Actualiser les donjons",
            "Aucun donjon standard ouvert n'a été trouvé.",
            "Donjons synchronisés, aucune cible disponible.",
            Duration::from_secs(15 * 60),
        ));
    };

    let win_ratio = {
        let state = connected
            .session
            .game_state()
            .ok_or_else(|| "État des donjons indisponible pour la simulation".to_string())?;
        let Some(monster) = state.dungeons.current_enemy(dungeon) else {
            // Pas de monstre connu à simuler (ex: étage "image miroir" avec
            // stats calquées sur le joueur) ; on tente le combat sans filtre
            // plutôt que de bloquer l'automatisation indéfiniment.
            send_limited(
                connected,
                store,
                Command::FightDungeon {
                    dungeon,
                    use_mushroom: false,
                },
            )
            .await?;
            let cooldown = connected
                .session
                .game_state()
                .and_then(|gs| gs.dungeons.next_free_fight)
                .filter(|date| *date > Local::now())
                .map(chrono_wait)
                .unwrap_or(Duration::from_secs(10 * 60));
            return Ok(AutomationCycleResult::sent(
                "Combattre dans un donjon",
                "Monstre non simulable (probablement une image miroir); combat tenté sans vérification préalable.",
                "Combat de donjon lancé.",
                cooldown,
            ));
        };
        estimate_win_ratio(state, Fighter::from(monster), false)
    };

    if win_ratio < MIN_WIN_PROBABILITY {
        return Ok(AutomationCycleResult::sent(
            "Simuler le combat de donjon",
            format!(
                "Taux de victoire simulé de {:.0}%, sous le seuil de {:.0}%; combat annulé.",
                win_ratio * 100.0,
                MIN_WIN_PROBABILITY * 100.0
            ),
            "Monstre jugé trop fort, aucun combat envoyé.",
            Duration::from_secs(15 * 60),
        ));
    }

    send_limited(
        connected,
        store,
        Command::FightDungeon {
            dungeon,
            use_mushroom: false,
        },
    )
    .await?;

    let cooldown = connected
        .session
        .game_state()
        .and_then(|gs| gs.dungeons.next_free_fight)
        .filter(|date| *date > Local::now())
        .map(chrono_wait)
        .unwrap_or(Duration::from_secs(10 * 60));

    Ok(AutomationCycleResult::sent(
        "Combattre dans un donjon",
        format!(
            "État revalidé puis premier donjon ouvert sélectionné; taux de victoire simulé de {:.0}% (seuil {:.0}%). Aucun champignon n'est demandé.",
            win_ratio * 100.0,
            MIN_WIN_PROBABILITY * 100.0
        ),
        "Combat de donjon lancé.",
        cooldown,
    ))
}

async fn run_expeditions(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État S&F indisponible".to_string())?;

    let Some(active) = state.tavern.expeditions.active() else {
        if !state.tavern.is_idle() {
            return Ok(AutomationCycleResult::idle(
                "Attendre",
                "Le personnage est occupé ailleurs (quête/garde); ce cycle ne force aucune action.",
                DEFAULT_IDLE_RECHECK,
            ));
        }

        let AvailableTasks::Expeditions(expeditions) = state.tavern.available_tasks() else {
            // Contrairement à la Taverne, ce module ne modifie JAMAIS le
            // réglage "quêtes / expéditions" du compte : on respecte le choix
            // existant du joueur plutôt que de le forcer.
            return Ok(AutomationCycleResult::idle(
                "Aucune expédition disponible",
                "Aucune expédition n'est proposée actuellement (préférence sur quêtes, ou événement inactif).",
                DEFAULT_IDLE_RECHECK,
            ));
        };

        let Some(target) = expeditions.first() else {
            return Ok(AutomationCycleResult::idle(
                "Aucune expédition proposée",
                "La liste d'expéditions disponibles est vide.",
                DEFAULT_IDLE_RECHECK,
            ));
        };

        if target.thirst_for_adventure_sec > state.tavern.thirst_for_adventure_sec {
            return Ok(AutomationCycleResult::idle(
                "Soif d'aventure insuffisante",
                "Pas assez de soif d'aventure pour démarrer cette expédition aujourd'hui.",
                Duration::from_secs(15 * 60),
            ));
        }

        send_limited(connected, store, Command::ExpeditionStart { pos: 0 }).await?;
        return Ok(AutomationCycleResult::sent(
            "Démarrer une expédition",
            "Première expédition proposée sélectionnée (choix affiné à venir).",
            "Expédition démarrée.",
            Duration::from_secs(12),
        ));
    };

    match active.current_stage() {
        ExpeditionStage::Boss(_) => {
            send_limited(connected, store, Command::ExpeditionContinue).await?;
            Ok(AutomationCycleResult::sent(
                "Combattre le boss d'expédition",
                "Le combat de boss d'expédition est le seul choix possible à cette étape.",
                "Combat de boss d'expédition lancé.",
                Duration::from_secs(15),
            ))
        }
        ExpeditionStage::Rewards(rewards) => {
            if rewards.is_empty() {
                return Ok(AutomationCycleResult::idle(
                    "Aucune récompense",
                    "Aucune récompense proposée à cette étape; en attente de resynchronisation.",
                    MIN_MODULE_RECHECK,
                ));
            }
            // V0.4: choix de la première récompense. Idée d'amélioration ci-dessous.
            send_limited(connected, store, Command::ExpeditionPickReward { pos: 0 }).await?;
            Ok(AutomationCycleResult::sent(
                "Choisir une récompense d'expédition",
                "Première récompense proposée choisie (pas encore de tri par valeur).",
                "Récompense d'expédition choisie.",
                Duration::from_secs(12),
            ))
        }
        ExpeditionStage::Encounters(roads) => {
            if roads.is_empty() {
                return Ok(AutomationCycleResult::idle(
                    "Aucun embranchement",
                    "Aucun embranchement proposé à cette étape; en attente de resynchronisation.",
                    MIN_MODULE_RECHECK,
                ));
            }
            // V0.4: premier embranchement. Idée d'amélioration ci-dessous.
            send_limited(connected, store, Command::ExpeditionPickEncounter { pos: 0 }).await?;
            Ok(AutomationCycleResult::sent(
                "Choisir un embranchement d'expédition",
                "Premier embranchement proposé choisi (pas encore de tri par récompense).",
                "Embranchement d'expédition choisi.",
                Duration::from_secs(12),
            ))
        }
        ExpeditionStage::Finished => Ok(AutomationCycleResult::idle(
            "Expédition terminée",
            "L'expédition vient de se terminer; le prochain cycle détectera la nouvelle situation.",
            MIN_MODULE_RECHECK,
        )),
        ExpeditionStage::Waiting { busy_until, .. } => Ok(AutomationCycleResult::idle(
            "Attendre l'étape suivante",
            "Attente locale basée sur le timer du jeu; les sabliers ne sont jamais dépensés automatiquement.",
            chrono_wait(busy_until),
        )),
        ExpeditionStage::Unknown => Ok(AutomationCycleResult::idle(
            "Étape non reconnue",
            "Étape d'expédition non reconnue par cette version; aucune action risquée n'est tentée.",
            DEFAULT_IDLE_RECHECK,
        )),
    }
}

/// Choisit, parmi les familiers déjà possédés d'un habitat, celui qui a le
/// plus haut niveau (le "meilleur" familier déjà développé) et qui n'a pas
/// encore reçu de fruit aujourd'hui.
fn best_feedable_pet(
    state: &sf_api::gamestate::GameState,
) -> Option<(HabitatType, u32)> {
    let pets = state.pets.as_ref()?;
    pets.habitats
        .iter()
        .filter(|(_, habitat)| habitat.fruits > 0)
        .flat_map(|(element, habitat)| {
            habitat
                .pets
                .iter()
                .filter(|pet| pet.level > 0 && pet.fruits_today == 0)
                .map(move |pet| (element, pet.id, pet.level))
        })
        .max_by_key(|(_, _, level)| *level)
        .map(|(element, id, _)| (element, id))
}

async fn run_pets(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État des familiers indisponible".to_string())?;

    let Some((habitat, pet_id)) = best_feedable_pet(state) else {
        return Ok(AutomationCycleResult::idle(
            "Aucun fruit disponible",
            "Aucun familier déjà possédé n'a de fruit disponible à lui donner aujourd'hui.",
            Duration::from_secs(30 * 60),
        ));
    };

    send_limited(
        connected,
        store,
        Command::PetFeed {
            pet_id,
            fruit_idx: habitat as u32,
        },
    )
    .await?;

    // Il peut rester d'autres familiers/habitats à nourrir : on revérifie
    // vite plutôt que d'attendre 30 minutes.
    Ok(AutomationCycleResult::sent(
        "Nourrir le meilleur familier",
        "Le familier déjà possédé avec le plus haut niveau dans un habitat ayant des fruits disponibles a été choisi.",
        format!("Familier #{pet_id} nourri."),
        Duration::from_secs(15),
    ))
}


fn json_member_has_joined(guild: &Value, username: &str, battle: &str) -> bool {
    guild
        .get("members")
        .and_then(Value::as_array)
        .and_then(|members| {
            members.iter().find(|member| {
                member.get("name").and_then(Value::as_str).is_some_and(|name| name.eq_ignore_ascii_case(username))
            })
        })
        .and_then(|member| member.get("battles_joined").or_else(|| member.get("battlesJoined")))
        .is_some_and(|joined| {
            let text = joined.to_string().to_ascii_lowercase();
            text.contains(battle) || text.contains("both")
        })
}

fn json_member_fought_portal(guild: &Value, username: &str) -> bool {
    guild
        .get("members")
        .and_then(Value::as_array)
        .and_then(|members| {
            members.iter().find(|member| {
                member.get("name").and_then(Value::as_str).is_some_and(|name| name.eq_ignore_ascii_case(username))
            })
        })
        .and_then(|member| member.get("portal_fought").or_else(|| member.get("portalFought")))
        .is_some_and(|value| !value.is_null())
}

async fn run_guild(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État S&F indisponible".to_string())?;
    let state_json = serde_json::to_value(state).map_err(|e| format!("Lecture de la guilde impossible: {e}"))?;
    let Some(guild) = state_json.get("guild").filter(|value| !value.is_null()) else {
        return Ok(AutomationCycleResult::idle(
            "Aucune guilde",
            "Le personnage n'appartient actuellement à aucune guilde.",
            Duration::from_secs(60 * 60),
        ));
    };
    let username = connected.session.username().to_string();

    if guild.get("attacking").is_some_and(|value| !value.is_null())
        && !json_member_has_joined(guild, &username, "attack")
    {
        send_limited(connected, store, Command::GuildJoinAttack).await?;
        return Ok(AutomationCycleResult::sent(
            "Participer à l'attaque de guilde",
            "Une attaque est planifiée et le personnage n'était pas encore inscrit.",
            "Participation à l'attaque enregistrée.",
            Duration::from_secs(30),
        ));
    }

    if guild.get("defending").is_some_and(|value| !value.is_null())
        && !json_member_has_joined(guild, &username, "defen")
    {
        send_limited(connected, store, Command::GuildJoinDefense).await?;
        return Ok(AutomationCycleResult::sent(
            "Participer à la défense de guilde",
            "Une défense est planifiée et le personnage n'était pas encore inscrit.",
            "Participation à la défense enregistrée.",
            Duration::from_secs(30),
        ));
    }

    let hydra = guild.get("hydra");
    let remaining_fights = hydra
        .and_then(|value| value.get("remaining_fights").or_else(|| value.get("remainingFights")))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let hydra_alive = hydra
        .and_then(|value| value.get("current_life").or_else(|| value.get("currentLife")))
        .and_then(Value::as_u64)
        .unwrap_or_default() > 0;

    if remaining_fights > 0 && hydra_alive {
        send_limited(
            connected,
            store,
            Command::GuildPetBattle {
                use_mushroom: false,
            },
        )
        .await?;
        return Ok(AutomationCycleResult::sent(
            "Combattre l'Hydre",
            format!("{remaining_fights} combat(s) gratuit(s) d'Hydre disponible(s)."),
            "Combat d'Hydre lancé sans champignon.",
            Duration::from_secs(30 * 60),
        ));
    }

    if !json_member_fought_portal(guild, &username) {
        send_limited(connected, store, Command::GuildPortalBattle).await?;
        return Ok(AutomationCycleResult::sent(
            "Combattre au portail de guilde",
            "Le combat quotidien du portail n'a pas encore été effectué.",
            "Combat du portail de guilde lancé.",
            Duration::from_secs(30 * 60),
        ));
    }

    Ok(AutomationCycleResult::idle(
        "Guilde à jour",
        "Aucune action personnelle gratuite de guilde n'est actuellement nécessaire.",
        Duration::from_secs(30 * 60),
    ))
}


async fn run_gem_mine(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État S&F indisponible".to_string())?;
    let Some(fortress) = &state.fortress else {
        return Ok(AutomationCycleResult::idle(
            "Forteresse non débloquée",
            "La mine de gemmes nécessite la forteresse.",
            Duration::from_secs(60 * 60),
        ));
    };

    if fortress.buildings[FortressBuildingType::GemMine].level == 0 {
        return Ok(AutomationCycleResult::idle(
            "Mine de gemmes non construite",
            "La mine de gemmes n'est pas encore disponible sur ce personnage.",
            Duration::from_secs(60 * 60),
        ));
    }

    if let Some(finish) = fortress.gem_search.finish {
        if finish > Local::now() {
            return Ok(AutomationCycleResult::idle(
                "Recherche de gemme en cours",
                "La recherche est déjà lancée; aucune requête n'est nécessaire avant sa fin.",
                chrono_wait(finish),
            ));
        }

        if state.character.inventory.count_free_slots() == 0 {
            return Ok(AutomationCycleResult::idle(
                "Inventaire plein",
                "La gemme est prête mais aucun emplacement d'inventaire n'est disponible.",
                Duration::from_secs(10 * 60),
            ));
        }

        send_limited(
            connected,
            store,
            Command::FortressGemStoneSearchFinish { mushrooms: 0 },
        )
        .await?;

        return Ok(AutomationCycleResult::sent(
            "Récupérer la gemme",
            "La recherche est terminée et la gemme est récupérée sans utiliser de champignon.",
            "Gemme récupérée.",
            Duration::from_secs(15),
        ));
    }

    let search_cost = fortress.gem_search.cost.silver;
    if state.character.silver < search_cost {
        return Ok(AutomationCycleResult::idle(
            "Or insuffisant",
            format!(
                "La prochaine recherche coûte {search_cost} or; le personnage n'en possède pas assez."
            ),
            Duration::from_secs(30 * 60),
        ));
    }

    send_limited(connected, store, Command::FortressGemStoneSearch).await?;

    let cooldown = connected
        .session
        .game_state()
        .and_then(|gs| gs.fortress.as_ref())
        .and_then(|fortress| fortress.gem_search.finish)
        .filter(|date| *date > Local::now())
        .map(chrono_wait)
        .unwrap_or(Duration::from_secs(60));

    Ok(AutomationCycleResult::sent(
        "Rechercher une gemme",
        format!("Nouvelle recherche lancée pour {search_cost} or, sans champignon."),
        "Recherche de gemme démarrée.",
        cooldown,
    ))
}

async fn run_wheel(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    const LUCKY_COIN_SPIN_COST: u32 = 10;
    const NORMAL_DAILY_SPIN_LIMIT: u8 = 20;

    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État S&F indisponible".to_string())?;
    let wheel = &state.specials.wheel;

    if wheel.spins_today >= NORMAL_DAILY_SPIN_LIMIT {
        return Ok(AutomationCycleResult::idle(
            "Roue terminée",
            format!(
                "{} tour(s) effectué(s) aujourd'hui; plafond quotidien normal atteint.",
                wheel.spins_today
            ),
            Duration::from_secs(60 * 60),
        ));
    }

    let free_turn_available = wheel.spins_today == 0
        || wheel
            .next_free_spin
            .is_some_and(|next_free| next_free <= Local::now());

    let payment = if free_turn_available {
        FortunePayment::FreeTurn
    } else if wheel.lucky_coins >= LUCKY_COIN_SPIN_COST {
        FortunePayment::LuckyCoins
    } else {
        return Ok(AutomationCycleResult::idle(
            "Aucun tour sûr disponible",
            format!(
                "{} Lucky Coin(s) disponible(s); il en faut {}. Aucun champignon ne sera utilisé.",
                wheel.lucky_coins, LUCKY_COIN_SPIN_COST
            ),
            Duration::from_secs(30 * 60),
        ));
    };

    let reason = if matches!(payment, FortunePayment::FreeTurn) {
        "Le tour gratuit est disponible.".to_string()
    } else {
        format!(
            "{} Lucky Coins sont disponibles; le tour coûte {} Lucky Coins.",
            wheel.lucky_coins, LUCKY_COIN_SPIN_COST
        )
    };

    send_limited(
        connected,
        store,
        Command::SpinWheelOfFortune { payment },
    )
    .await?;

    Ok(AutomationCycleResult::sent(
        "Tourner la roue de la fortune",
        reason,
        "Roue tournée sans utiliser de champignon.",
        Duration::from_secs(15),
    ))
}

/// IMPORTANT: contrairement à l'arène et aux donjons, sf-api n'expose pas les
/// statistiques de l'armée adverse pour une attaque de forteresse (seul
/// l'identifiant du joueur ciblé est connu). Il est donc impossible de
/// calculer un vrai taux de victoire ici. Cette fonction applique à la place
/// une heuristique prudente et documentée : n'attaquer que si une réserve
/// confortable de soldats entraînés est disponible, jamais l'intégralité de
/// l'armée. Ce n'est PAS une garantie de victoire.
async fn run_fortress(
    connected: &mut ConnectedSession,
    store: &SessionStore,
) -> Result<AutomationCycleResult, String> {
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État de la forteresse indisponible".to_string())?;

    let Some(fortress) = &state.fortress else {
        return Ok(AutomationCycleResult::idle(
            "Forteresse non débloquée",
            "Ce compte n'a pas encore de forteresse.",
            Duration::from_secs(60 * 60),
        ));
    };

    let Some(_target) = fortress.attack_target else {
        return Ok(AutomationCycleResult::idle(
            "Aucune cible",
            "Aucun adversaire de forteresse n'est actuellement assigné.",
            Duration::from_secs(30 * 60),
        ));
    };

    let soldiers = &fortress.units[sf_api::gamestate::fortress::FortressUnitType::Soldier];
    let available = u32::from(soldiers.count.saturating_sub(soldiers.in_training));

    if available == 0 {
        return Ok(AutomationCycleResult::idle(
            "Aucun soldat disponible",
            "Aucun soldat entraîné disponible pour attaquer.",
            Duration::from_secs(30 * 60),
        ));
    }

    let reserve = ((f64::from(available)) * FORTRESS_MIN_SOLDIER_RESERVE_RATIO).ceil() as u32;
    let sendable = available.saturating_sub(reserve);

    if sendable == 0 {
        return Ok(AutomationCycleResult::idle(
            "Réserve de soldats insuffisante",
            format!(
                "Seulement {available} soldat(s) disponible(s); en dessous du seuil de réserve prudent, aucune attaque envoyée."
            ),
            Duration::from_secs(30 * 60),
        ));
    }

    send_limited(
        connected,
        store,
        Command::FortressAttack { soldiers: sendable },
    )
    .await?;

    Ok(AutomationCycleResult::sent(
        "Attaquer la forteresse ennemie",
        format!(
            "Heuristique de réserve appliquée (pas de simulation de victoire disponible côté sf-api) : {sendable}/{available} soldats engagés."
        ),
        "Attaque de forteresse envoyée.",
        Duration::from_secs(30 * 60),
    ))
}

#[tauri::command]
async fn sf_login_account(
    username: String,
    password: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<CharacterSummary>, String> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("Identifiant et mot de passe requis".to_string());
    }

    let mut sessions = SimpleSession::login_sf_account(username.trim(), &password)
        .await
        .map_err(|e| format!("Connexion au compte S&F impossible: {e}"))?;

    let mut connected = Vec::with_capacity(sessions.len());
    let mut summaries = Vec::with_capacity(sessions.len());

    for (index, session) in sessions.drain(..).enumerate() {
        let id = format!("sf-account-{index}");
        let mut entry = ConnectedSession::new(id.clone(), session);
        send_limited(&mut entry, &store, Command::Update)
            .await
            .map_err(|e| format!("Connexion au personnage {} impossible: {e}", entry.session.username()))?;
        summaries.push(summarize(id, &entry.session));
        connected.push(entry);
    }

    *store.sessions.lock().await = connected;
    Ok(summaries)
}

#[tauri::command]
async fn sf_login_character(
    username: String,
    password: String,
    server: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<CharacterSummary>, String> {
    if username.trim().is_empty() || password.is_empty() || server.trim().is_empty() {
        return Err("Personnage, mot de passe et serveur requis".to_string());
    }

    let normalized_server = server
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    let session = SimpleSession::login(username.trim(), &password, normalized_server)
        .await
        .map_err(|e| format!("Connexion au personnage S&F impossible: {e}"))?;

    let id = "sf-character-0".to_string();
    let summary = summarize(id.clone(), &session);
    *store.sessions.lock().await = vec![ConnectedSession::new(id, session)];
    Ok(vec![summary])
}

#[tauri::command]
async fn sf_refresh_character(
    character_id: String,
    store: State<'_, SessionStore>,
) -> Result<CharacterSummary, String> {
    let mut guard = store.sessions.lock().await;
    let connected = guard
        .iter_mut()
        .find(|entry| entry.id == character_id)
        .ok_or_else(|| "Session S&F introuvable".to_string())?;

    let cache_fresh = connected
        .last_sync_at
        .is_some_and(|last| last.elapsed() < REFRESH_CACHE_TTL);
    if !cache_fresh {
        send_limited(connected, &store, Command::Update)
            .await
            .map_err(|e| format!("Mise à jour S&F impossible: {e}"))?;
    }

    Ok(summarize(connected.id.clone(), &connected.session))
}

#[tauri::command]
async fn sf_simulate_dungeon(
    character_id: String,
    iterations: u32,
    store: State<'_, SessionStore>,
) -> Result<DungeonSimulationResult, String> {
    let iterations = iterations.clamp(100, 20_000);
    let guard = store.sessions.lock().await;
    let connected = guard
        .iter()
        .find(|entry| entry.id == character_id)
        .ok_or_else(|| "Session S&F introuvable".to_string())?;
    let state = connected
        .session
        .game_state()
        .ok_or_else(|| "État des donjons indisponible".to_string())?;
    let dungeon = first_open_dungeon(state).ok_or_else(|| "Aucun donjon standard ouvert".to_string())?;
    let monster = state
        .dungeons
        .current_enemy(dungeon)
        .ok_or_else(|| "Monstre du donjon indisponible pour la simulation".to_string())?;
    let win_ratio = estimate_win_ratio_with_iterations(state, Fighter::from(monster), false, iterations);
    let recommended = win_ratio >= MIN_WIN_PROBABILITY;
    Ok(DungeonSimulationResult {
        dungeon: format!("{dungeon:?}"),
        iterations,
        win_ratio,
        threshold: MIN_WIN_PROBABILITY,
        recommended,
        recommendation: if recommended {
            format!("Combat conseillé : {:.1}% de victoire estimée.", win_ratio * 100.0)
        } else {
            format!("Attendre une amélioration : {:.1}% de victoire estimée.", win_ratio * 100.0)
        },
    })
}

#[tauri::command]
async fn sf_automation_cycle(
    character_id: String,
    automation_id: String,
    store: State<'_, SessionStore>,
) -> Result<AutomationCycleResult, String> {
    let mut guard = store.sessions.lock().await;
    let connected = guard
        .iter_mut()
        .find(|entry| entry.id == character_id)
        .ok_or_else(|| "Session S&F introuvable".to_string())?;

    if let Some(wait) = module_guard(connected, &automation_id) {
        return Ok(AutomationCycleResult::idle(
            "Cooldown local",
            "Le backend a ignoré cet appel avant l'échéance prévue.",
            wait,
        ));
    }

    let result = match automation_id.as_str() {
        "tavern" => run_tavern(connected, &store).await,
        "arena" => run_arena(connected, &store).await,
        "dungeons" => run_dungeons(connected, &store).await,
        "expeditions" => run_expeditions(connected, &store).await,
        "pets" => run_pets(connected, &store).await,
        "fortress" => run_fortress(connected, &store).await,
        "guild" => run_guild(connected, &store).await,
        "gem-mine" => run_gem_mine(connected, &store).await,
        "wheel" => run_wheel(connected, &store).await,
        _ => Err(format!("Automatisation inconnue: {automation_id}")),
    }?;

    let cooldown = Duration::from_millis(result.cooldown_ms).max(MIN_MODULE_RECHECK);
    remember_module_cooldown(connected, &automation_id, cooldown);
    Ok(result)
}

#[tauri::command]
async fn sf_logout(store: State<'_, SessionStore>) -> Result<(), String> {
    store.sessions.lock().await.clear();
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(SessionStore::default())
        .invoke_handler(tauri::generate_handler![
            sf_login_account,
            sf_login_character,
            sf_refresh_character,
            sf_simulate_dungeon,
            sf_automation_cycle,
            sf_logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running MercySF Reborn");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::Update));
    }

    #[test]
    fn arena_check_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::CheckArena));
    }

    #[test]
    fn safe_quest_start_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::StartQuest {
            quest_pos: 0,
            overwrite_inv: false,
        }));
    }

    #[test]
    fn safe_quest_finish_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::FinishQuest {
            skip: None,
        }));
    }

    #[test]
    fn free_arena_fight_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::Fight {
            name: "adversaire".to_string(),
            use_mushroom: false,
        }));
    }

    #[test]
    fn gem_search_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::FortressGemStoneSearch));
    }

    #[test]
    fn free_gem_search_finish_is_allowed() {
        assert!(is_mushroom_safe_command(
            &Command::FortressGemStoneSearchFinish { mushrooms: 0 }
        ));
    }

    #[test]
    fn paid_gem_search_finish_is_blocked() {
        assert!(!is_mushroom_safe_command(
            &Command::FortressGemStoneSearchFinish { mushrooms: 1 }
        ));
    }

    #[test]
    fn free_wheel_spin_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::SpinWheelOfFortune {
            payment: FortunePayment::FreeTurn,
        }));
    }

    #[test]
    fn lucky_coin_wheel_spin_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::SpinWheelOfFortune {
            payment: FortunePayment::LuckyCoins,
        }));
    }

    #[test]
    fn mushroom_wheel_spin_is_blocked() {
        assert!(!is_mushroom_safe_command(&Command::SpinWheelOfFortune {
            payment: FortunePayment::Mushrooms,
        }));
    }

    #[test]
    fn free_guild_hydra_fight_is_allowed() {
        assert!(is_mushroom_safe_command(&Command::GuildPetBattle {
            use_mushroom: false,
        }));
    }

    #[test]
    fn mushroom_guild_hydra_fight_is_blocked() {
        assert!(!is_mushroom_safe_command(&Command::GuildPetBattle {
            use_mushroom: true,
        }));
    }

    #[test]
    fn mushroom_arena_fight_is_blocked() {
        assert!(!is_mushroom_safe_command(&Command::Fight {
            name: "adversaire".to_string(),
            use_mushroom: true,
        }));
    }
}
