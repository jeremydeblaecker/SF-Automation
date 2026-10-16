import type { AutomationId, Character, DungeonSimulationResult } from "../types/domain";

export interface AutomationRunResult {
  action: string;
  reason: string;
  message: string;
  cooldownMs: number;
  didRequest: boolean;
}

export interface AccountLogin {
  kind: "account";
  username: string;
  password: string;
}

export interface CharacterLogin {
  kind: "character";
  username: string;
  password: string;
  server: string;
}

export type LoginRequest = AccountLogin | CharacterLogin;

export interface GameClient {
  getCharacter(characterId: string): Promise<Character>;
  runAutomation(automationId: AutomationId, characterId: string): Promise<AutomationRunResult>;
  simulateDungeon(characterId: string, iterations: number): Promise<DungeonSimulationResult>;
}

export interface ConnectedGameClient extends GameClient {
  login(request: LoginRequest): Promise<Character[]>;
  logout(): Promise<void>;
  refresh(characterId: string): Promise<Character>;
  isNative(): boolean;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export class MockGameClient implements ConnectedGameClient {
  constructor(private characters: Character[]) {}

  isNative() { return false; }

  async login(): Promise<Character[]> {
    await sleep(400);
    return this.characters;
  }

  async logout() { await sleep(80); }

  async refresh(characterId: string) { return this.getCharacter(characterId); }

  async getCharacter(characterId: string) {
    await sleep(80);
    const character = this.characters.find((item) => item.id === characterId);
    if (!character) throw new Error("Personnage introuvable");
    return character;
  }

  async simulateDungeon(characterId: string, iterations: number): Promise<DungeonSimulationResult> {
    await this.getCharacter(characterId);
    await sleep(150);
    const winRatio = 0.72;
    return {
      dungeon: "Donjon simulé",
      iterations,
      winRatio,
      threshold: 0.60,
      recommended: winRatio >= 0.60,
      recommendation: "Simulation locale de démonstration.",
    };
  }

  async runAutomation(automationId: AutomationId, characterId: string): Promise<AutomationRunResult> {
    await this.getCharacter(characterId);
    await sleep(180);
    switch (automationId) {
      case "tavern":
        return { action: "Accepter une quête", reason: "Meilleur ratio XP / temps parmi les choix simulés", message: "Quête simulée démarrée.", cooldownMs: 18_000, didRequest: false };
      case "arena":
        return { action: "Combattre dans l'arène", reason: "Taux de victoire simulé de 74% (seuil 60%)", message: "Combat simulé lancé.", cooldownMs: 12_000, didRequest: false };
      case "dungeons":
        return { action: "Combattre dans un donjon", reason: "Taux de victoire simulé de 81% (seuil 60%)", message: "Donjon simulé lancé.", cooldownMs: 30_000, didRequest: false };
      case "expeditions":
        return { action: "Choisir un embranchement d'expédition", reason: "Premier embranchement proposé choisi (simulation)", message: "Embranchement simulé choisi.", cooldownMs: 12_000, didRequest: false };
      case "pets":
        return { action: "Nourrir le meilleur familier", reason: "Familier de plus haut niveau avec fruit disponible (simulation)", message: "Familier simulé nourri.", cooldownMs: 15_000, didRequest: false };
      case "fortress":
        return { action: "Attaquer la forteresse ennemie", reason: "Heuristique de réserve de soldats (simulation, pas de vraie prédiction)", message: "Attaque de forteresse simulée.", cooldownMs: 30_000, didRequest: false };
      case "guild":
        return { action: "Vérifier la guilde", reason: "Hydre, portail et combats de guilde simulés", message: "Cycle guilde simulé.", cooldownMs: 30_000, didRequest: false };
      case "gem-mine":
        return { action: "Vérifier la mine de gemmes", reason: "Recherche simulée sans champignons", message: "Cycle mine simulé.", cooldownMs: 30_000, didRequest: false };
      case "wheel":
        return { action: "Vérifier la roue de la fortune", reason: "Tour gratuit ou Lucky Coins uniquement", message: "Cycle roue simulé.", cooldownMs: 30_000, didRequest: false };
    }
  }
}

export class SfTauriClient implements ConnectedGameClient {
  private readonly characterCache = new Map<string, Character>();

  isNative() { return true; }

  private async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }

  private remember(characters: Character[]) {
    characters.forEach((character) => this.characterCache.set(character.id, character));
    return characters;
  }

  async login(request: LoginRequest): Promise<Character[]> {
    const characters = request.kind === "account"
      ? await this.invoke<Character[]>("sf_login_account", { username: request.username, password: request.password })
      : await this.invoke<Character[]>("sf_login_character", { username: request.username, password: request.password, server: request.server });
    this.characterCache.clear();
    return this.remember(characters);
  }

  async logout() {
    await this.invoke<void>("sf_logout");
    this.characterCache.clear();
  }

  async refresh(characterId: string) {
    const character = await this.invoke<Character>("sf_refresh_character", { characterId });
    this.characterCache.set(character.id, character);
    return character;
  }

  async getCharacter(characterId: string) {
    return this.characterCache.get(characterId) ?? this.refresh(characterId);
  }

  async simulateDungeon(characterId: string, iterations: number): Promise<DungeonSimulationResult> {
    return this.invoke<DungeonSimulationResult>("sf_simulate_dungeon", { characterId, iterations });
  }

  async runAutomation(automationId: AutomationId, characterId: string): Promise<AutomationRunResult> {
    return this.invoke<AutomationRunResult>("sf_automation_cycle", { automationId, characterId });
  }
}

export function createGameClient(characters: Character[]): ConnectedGameClient {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  return isTauri ? new SfTauriClient() : new MockGameClient(characters);
}
