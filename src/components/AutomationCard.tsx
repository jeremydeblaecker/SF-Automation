import { Play, Power } from "lucide-react";
import type { AutomationState } from "../types/domain";

function remaining(date?: string) {
  if (!date) return "Prêt";
  const sec = Math.max(0, Math.ceil((new Date(date).getTime() - Date.now()) / 1000));
  return sec > 0 ? `${sec}s` : "Prêt";
}

export function AutomationCard({ automation, onToggle, onRun }: { automation: AutomationState; onToggle: () => void; onRun: () => void }) {
  return <article className="automation-card"><div className="automation-head"><div><h3>{automation.name}</h3><p>{automation.description}</p></div><button className={automation.enabled ? "power on" : "power"} onClick={onToggle} title="Activer / désactiver"><Power size={17}/></button></div><div className="status-row"><span className={`dot ${automation.status}`}/><strong>{automation.status === "running" ? "En cours" : automation.enabled ? "Actif" : "Désactivé"}</strong><span className="countdown">{automation.enabled ? remaining(automation.nextRunAt) : "—"}</span></div><p className="reason">{automation.lastReason}</p><button className="run-button" onClick={onRun} disabled={!automation.enabled || automation.status === "running"}><Play size={15}/> Exécuter maintenant</button></article>;
}
