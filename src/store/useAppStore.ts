import { useEffect, useMemo, useState } from "react";
import { createGameClient, type LoginRequest } from "../core/gameClient";
import { AutomationScheduler } from "../core/scheduler";
import type { AppState, AutomationId } from "../types/domain";

const STORAGE_KEY = "mercysf-reborn-state-v4";

const initialState: AppState = {
  running: false,
  connected: false,
  connecting: false,
  activeCharacterId: "",
  characters: [],
  automations: [
    { id: "tavern", name: "Taverne", description: "Quêtes et expéditions", enabled: true, status: "ready", lastReason: "En attente d’un personnage connecté" },
    { id: "arena", name: "Arène", description: "Combats simulés avant envoi (seuil 60% de victoire)", enabled: true, status: "ready", lastReason: "En attente d’un personnage connecté" },
    { id: "dungeons", name: "Donjons", description: "Tentatives disponibles, combats simulés avant envoi", enabled: true, status: "ready", lastReason: "En attente d’un personnage connecté" },
    { id: "expeditions", name: "Expéditions", description: "Progression automatique (embranchements, récompenses, boss)", enabled: false, status: "idle", lastReason: "En attente d’un personnage connecté" },
    { id: "pets", name: "Familiers", description: "Nourrit automatiquement le meilleur familier disponible", enabled: false, status: "idle", lastReason: "En attente d’un personnage connecté" },
    { id: "fortress", name: "Forteresse", description: "Attaque heuristique avec réserve de soldats (pas de simulation de victoire)", enabled: false, status: "idle", lastReason: "En attente d’un personnage connecté" },
    { id: "guild", name: "Guilde", description: "Hydre, portail et participation aux combats de guilde", enabled: false, status: "idle", lastReason: "En attente d’un personnage connecté" },
    { id: "gem-mine", name: "Mine de gemmes", description: "Recherche et récupération automatique des gemmes sans champignons", enabled: false, status: "idle", lastReason: "En attente d’un personnage connecté" },
    { id: "wheel", name: "Roue de la fortune", description: "Tours gratuits puis Lucky Coins uniquement, jamais de champignon", enabled: false, status: "idle", lastReason: "En attente d’un personnage connecté" }
  ],
  logs: [{ id: "boot", at: new Date().toISOString(), level: "info", message: "MercySF Reborn initialisé." }]
};

function loadState(): AppState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return initialState;
    const stored = JSON.parse(raw) as Partial<AppState>;
    return { ...initialState, ...stored, connected: false, connecting: false, running: false, characters: [], activeCharacterId: "" };
  } catch {
    return initialState;
  }
}

let latestState: AppState = initialState;

export function useAppStore() {
  const [state, setState] = useState<AppState>(loadState);
  const client = useMemo(() => createGameClient([]), []);
  const scheduler = useMemo(() => new AutomationScheduler(client, (recipe) => setState((current) => recipe(current)), () => latestState), [client]);
  latestState = state;

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...state, connected: false, connecting: false, running: false, characters: [], activeCharacterId: "", connectionError: undefined }));
  }, [state]);

  useEffect(() => {
    scheduler.start();
    return () => scheduler.stop();
  }, [scheduler]);

  const login = async (request: LoginRequest) => {
    setState((current) => ({ ...current, connecting: true, connectionError: undefined }));
    try {
      const characters = await client.login(request);
      if (characters.length === 0) throw new Error("Aucun personnage trouvé sur ce compte S&F.");
      setState((current) => ({
        ...current,
        connecting: false,
        connected: true,
        characters,
        activeCharacterId: characters[0].id,
        logs: [{ id: crypto.randomUUID(), at: new Date().toISOString(), level: "success", message: `${characters.length} personnage(s) connecté(s) à S&F.` }, ...current.logs].slice(0, 100),
      }));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setState((current) => ({ ...current, connecting: false, connected: false, connectionError: message, logs: [{ id: crypto.randomUUID(), at: new Date().toISOString(), level: "error", message }, ...current.logs].slice(0, 100) }));
    }
  };

  const logout = async () => {
    await client.logout();
    setState((current) => ({ ...current, connected: false, running: false, characters: [], activeCharacterId: "", connectionError: undefined }));
  };

  const refreshCharacter = async () => {
    if (!state.activeCharacterId) return;
    try {
      const updated = await client.refresh(state.activeCharacterId);
      setState((current) => ({ ...current, characters: current.characters.map((item) => item.id === updated.id ? updated : item) }));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setState((current) => ({ ...current, logs: [{ id: crypto.randomUUID(), at: new Date().toISOString(), level: "error", message }, ...current.logs].slice(0, 100) }));
    }
  };

  const toggleRunning = () => setState((current) => ({ ...current, running: current.connected ? !current.running : false }));
  const selectCharacter = (id: string) => setState((current) => ({
    ...current,
    activeCharacterId: id,
    automations: current.automations.map((item) => ({
      ...item,
      status: item.enabled ? "ready" : "idle",
      nextRunAt: undefined,
      lastReason: "Personnage changé : nouvel état à évaluer sans réutiliser l’ancien cooldown UI.",
    })),
  }));
  const toggleAutomation = (id: AutomationId) => setState((current) => ({ ...current, automations: current.automations.map((item) => item.id === id ? { ...item, enabled: !item.enabled, status: item.enabled ? "idle" : "ready", nextRunAt: undefined } : item) }));
  const simulateDungeon = (iterations: number) => {
    if (!state.activeCharacterId) throw new Error("Aucun personnage actif");
    return client.simulateDungeon(state.activeCharacterId, iterations);
  };

  return {
    state,
    nativeMode: client.isNative(),
    login,
    logout,
    refreshCharacter,
    toggleRunning,
    selectCharacter,
    toggleAutomation,
    simulateDungeon,
    runNow: (id: AutomationId) => scheduler.runNow(id),
  };
}
