import { Activity, Castle, FlaskConical, Home, Settings, Shield, Swords, UserRound } from "lucide-react";

export type AppView = "dashboard" | "dungeon-simulator";

const items = [
  { icon: Home, label: "Dashboard", view: "dashboard" as AppView },
  { icon: UserRound, label: "Personnages" },
  { icon: Activity, label: "Taverne" },
  { icon: Swords, label: "Arène" },
  { icon: Castle, label: "Donjons" },
  { icon: FlaskConical, label: "Simulateur donjon", view: "dungeon-simulator" as AppView },
  { icon: Shield, label: "Forteresse" },
  { icon: Settings, label: "Paramètres" },
];

export function Sidebar({ activeView, onNavigate }: { activeView: AppView; onNavigate: (view: AppView) => void }) {
  return <aside className="sidebar">
    <div className="brand"><div className="brand-mark">M</div><div><strong>MercySF</strong><span>Reborn</span></div></div>
    <nav>{items.map(({ icon: Icon, label, view }) => <button key={label} className={view === activeView ? "nav active" : "nav"} onClick={() => view && onNavigate(view)}><Icon size={18}/><span>{label}</span>{!view && <em>Bientôt</em>}</button>)}</nav>
    <div className="sidebar-note">V0.4 · mode prudent</div>
  </aside>;
}
