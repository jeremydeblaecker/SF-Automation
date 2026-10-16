import { Activity, Castle, FlaskConical, Home, Settings, Shield, Swords, UserRound } from "lucide-react";

export type AppView = "dashboard" | "dungeon-simulator";

const items = [
  [Home, "Dashboard", "dashboard"],
  [UserRound, "Personnages", "dashboard"],
  [Activity, "Taverne", "dashboard"],
  [Swords, "Arène", "dashboard"],
  [Castle, "Donjons", "dashboard"],
  [FlaskConical, "Simulateur donjon", "dungeon-simulator"],
  [Shield, "Forteresse", "dashboard"],
  [Settings, "Paramètres", "dashboard"],
] as const;

export function Sidebar({ activeView, onNavigate }: { activeView: AppView; onNavigate: (view: AppView) => void }) {
  return <aside className="sidebar">
    <div className="brand"><div className="brand-mark">M</div><div><strong>MercySF</strong><span>Reborn</span></div></div>
    <nav>{items.map(([Icon, label, view]) => <button key={label} className={activeView === view ? "nav active" : "nav"} onClick={() => onNavigate(view)}><Icon size={18}/><span>{label}</span></button>)}</nav>
    <div className="sidebar-note">V0.4 · mode prudent</div>
  </aside>;
}
