export type AutomationId = "tavern" | "arena" | "dungeons" | "expeditions" | "pets" | "fortress" | "guild" | "gem-mine" | "wheel";

export interface Character {
  id: string;
  name: string;
  server: string;
  level: number;
  className: string;
  online: boolean;
}

export interface AutomationState {
  id: AutomationId;
  name: string;
  description: string;
  enabled: boolean;
  status: "idle" | "ready" | "running" | "cooldown" | "error";
  nextRunAt?: string;
  lastReason?: string;
}

export interface ActionLog {
  id: string;
  at: string;
  automationId?: AutomationId;
  level: "info" | "success" | "warning" | "error";
  message: string;
}

export interface AppState {
  running: boolean;
  connected: boolean;
  connecting: boolean;
  connectionError?: string;
  activeCharacterId: string;
  characters: Character[];
  automations: AutomationState[];
  logs: ActionLog[];
}

export interface DungeonSimulationResult {
  dungeon: string;
  iterations: number;
  winRatio: number;
  threshold: number;
  recommended: boolean;
  recommendation: string;
}
