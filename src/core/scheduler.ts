import type { AutomationId, AppState } from "../types/domain";
import type { GameClient } from "./gameClient";

export type StateUpdater = (recipe: (state: AppState) => AppState) => void;

const MIN_FRONTEND_RECHECK_MS = 8_000;
const ERROR_BACKOFF_MS = 60_000;

export class AutomationScheduler {
  private timer?: number;
  private busy = new Set<AutomationId>();

  constructor(private readonly client: GameClient, private readonly update: StateUpdater, private readonly getState: () => AppState) {}

  start() {
    if (this.timer) return;
    this.tick();
    this.timer = window.setInterval(() => this.tick(), 1000);
  }

  stop() {
    if (this.timer) window.clearInterval(this.timer);
    this.timer = undefined;
  }

  async runNow(id: AutomationId) {
    if (this.busy.has(id)) return;
    const state = this.getState();
    const automation = state.automations.find((item) => item.id === id);
    if (!automation || !state.activeCharacterId) return;

    const nextRun = automation.nextRunAt ? new Date(automation.nextRunAt).getTime() : 0;
    if (nextRun > Date.now()) {
      this.log(id, "info", `${this.label(id)} est déjà planifié; appel ignoré pour éviter un polling inutile.`);
      return;
    }

    this.busy.add(id);
    this.patchAutomation(id, { status: "running", lastReason: "Analyse de l’état local…" });

    try {
      const result = await this.client.runAutomation(id, state.activeCharacterId);
      const cooldownMs = Math.max(result.cooldownMs, MIN_FRONTEND_RECHECK_MS);
      const nextRunAt = new Date(Date.now() + cooldownMs).toISOString();
      this.patchAutomation(id, { status: "cooldown", nextRunAt, lastReason: result.reason });
      this.log(id, result.didRequest ? "success" : "info", `${result.action} — ${result.reason} ${result.message}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const nextRunAt = new Date(Date.now() + ERROR_BACKOFF_MS).toISOString();
      this.patchAutomation(id, { status: "error", lastReason: message, nextRunAt });
      this.log(id, "error", `${message} Nouvelle tentative UI au plus tôt dans 60 s.`);
    } finally {
      this.busy.delete(id);
    }
  }

  private tick() {
    const state = this.getState();
    if (!state.running || !state.connected || !state.activeCharacterId) return;
    for (const automation of state.automations) {
      if (!automation.enabled || this.busy.has(automation.id)) continue;
      const due = !automation.nextRunAt || new Date(automation.nextRunAt).getTime() <= Date.now();
      if (due) void this.runNow(automation.id);
    }
  }

  private patchAutomation(id: AutomationId, patch: Partial<AppState["automations"][number]>) {
    this.update((state) => ({ ...state, automations: state.automations.map((item) => item.id === id ? { ...item, ...patch } : item) }));
  }

  private log(automationId: AutomationId, level: AppState["logs"][number]["level"], message: string) {
    this.update((state) => ({ ...state, logs: [{ id: crypto.randomUUID(), at: new Date().toISOString(), automationId, level, message }, ...state.logs].slice(0, 100) }));
  }

  private label(id: AutomationId) {
    return this.getState().automations.find((item) => item.id === id)?.name ?? id;
  }
}
