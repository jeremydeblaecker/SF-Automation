from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
rust = (root / 'src-tauri/src/main.rs').read_text(encoding='utf-8')
production_rust = rust.split('\n#[cfg(test)]', 1)[0]
ts = (root / 'src/core/scheduler.ts').read_text(encoding='utf-8')

arena_start = production_rust.find('async fn run_arena')
dungeon_start = production_rust.find('async fn run_dungeons')
arena = production_rust[arena_start:dungeon_start] if arena_start >= 0 and dungeon_start > arena_start else ''
dungeons = production_rust[dungeon_start:] if dungeon_start >= 0 else ''

checks = []
def check(name, ok):
    checks.append((name, bool(ok)))

check('Une seule sortie send_command dans le backend', production_rust.count('.send_command(') == 1)
check('Intervalle global >= 5 secondes', 'MIN_GLOBAL_REQUEST_INTERVAL: Duration = Duration::from_secs(5)' in production_rust)
check('Cache refresh de 30 secondes', 'REFRESH_CACHE_TTL: Duration = Duration::from_secs(30)' in production_rust)
check('Cooldown backend minimum de 8 secondes', 'MIN_MODULE_RECHECK: Duration = Duration::from_secs(8)' in production_rust)
check('Backoff exponentiel avec plafond 15 min', '.min(15 * 60)' in production_rust and 'error_streak' in production_rust)
check('StartQuest ne remplace jamais un objet', 'overwrite_inv: false' in production_rust and 'overwrite_inv: true' not in production_rust)
check('Aucun Fight ne demande de champignon', 'use_mushroom: true' not in production_rust and production_rust.count('use_mushroom: false') >= 2)
check('Allowlist anti-champignons active', 'fn is_mushroom_safe_command' in production_rust and 'Fail closed' in production_rust)
check('Toute commande réseau passe par la protection premium', production_rust.count('session.send_command(') == 1 and 'is_mushroom_safe_command(&command)' in production_rust)
check('Les skips de quête sont bloqués', 'Command::FinishQuest { skip: None } => true' in production_rust)
check('Les commandes inconnues sont bloquées par défaut', '_ => false' in production_rust)
check('Arène vérifie next_free_fight avant CheckArena', arena.find('gs.arena.next_free_fight') >= 0 and arena.find('gs.arena.next_free_fight') < arena.find('Command::CheckArena'))
check('Donjons utilisent UpdateDungeons avant FightDungeon', dungeons.find('Command::UpdateDungeons') >= 0 and dungeons.find('Command::UpdateDungeons') < dungeons.find('Command::FightDungeon'))
check('Scheduler bloque les exécutions simultanées', 'this.busy.has(id)' in ts and 'this.busy.add(id)' in ts)
check('Scheduler respecte nextRunAt avant runNow', 'if (nextRun > Date.now())' in ts)
check('Scheduler ne contacte rien à chaque tick', 'if (due) void this.runNow(automation.id);' in ts)

failed = [name for name, ok in checks if not ok]
for name, ok in checks:
    print(('PASS' if ok else 'FAIL') + '  ' + name)

if failed:
    raise SystemExit(f'\n{len(failed)} vérification(s) en échec')
print(f'\n{len(checks)} vérifications de sécurité réussies.')
