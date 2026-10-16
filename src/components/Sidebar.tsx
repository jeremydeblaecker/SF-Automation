import { Activity, Castle, Home, Settings, Shield, Swords, UserRound } from "lucide-react";

const items = [
  [Home, "Dashboard", true], [UserRound, "Personnages", false], [Activity, "Taverne", false], [Swords, "Arène", false], [Castle, "Donjons", false], [Shield, "Forteresse", false], [Settings, "Paramètres", false]
] as const;

export function Sidebar() {
  return <aside className="sidebar"><div className="brand"><div className="brand-mark">M</div><div><strong>MercySF</strong><span>Reborn</span></div></div><nav>{items.map(([Icon, label, active]) => <button key={label} className={active ? "nav active" : "nav"}><Icon size={18}/><span>{label}</span>{!active && ["Forteresse"].includes(label) && <em>V2</em>}</button>)}</nav><div className="sidebar-note">V0.1 · mode simulation</div></aside>;
}
