import { useState } from "react";
import { Bot, ChevronDown, CircleStop, LogOut, Play, RefreshCw, Server } from "lucide-react";
import { AutomationCard } from "./components/AutomationCard";
import { LoginPanel } from "./components/LoginPanel";
import { Logs } from "./components/Logs";
import { DungeonSimulator } from "./components/DungeonSimulator";
import { Sidebar, type AppView } from "./components/Sidebar";
import { useAppStore } from "./store/useAppStore";
import "./styles.css";

export default function App() {
  const [view, setView] = useState<AppView>("dashboard");
  const {
    state,
    nativeMode,
    login,
    logout,
    refreshCharacter,
    toggleRunning,
    selectCharacter,
    toggleAutomation,
    simulateDungeon,
    runNow,
  } = useAppStore();

  if (!state.connected || state.characters.length === 0) {
    return <LoginPanel loading={state.connecting} error={state.connectionError} nativeMode={nativeMode} onLogin={login}/>;
  }

  const character = state.characters.find((item) => item.id === state.activeCharacterId) ?? state.characters[0];

  return <div className="app-shell">
    <Sidebar activeView={view} onNavigate={setView}/>
    <main>
      <header className="topbar">
        <div><span className="eyebrow">AUTOMATION CONTROL CENTER</span><h1>{view === "dashboard" ? "Dashboard" : "Dungeon Simulator"}</h1></div>
        <div className="top-actions">
          <div className="connection"><span className="online-dot"/>S&amp;F connecté</div>
          <button className="icon-action" title="Rafraîchir le personnage" onClick={() => void refreshCharacter()}><RefreshCw size={17}/></button>
          <button className="icon-action" title="Déconnexion" onClick={() => void logout()}><LogOut size={17}/></button>
          <button className={state.running ? "master stop" : "master"} onClick={toggleRunning}>{state.running ? <CircleStop size={17}/> : <Play size={17}/>} {state.running ? "Arrêter" : "Démarrer"}</button>
        </div>
      </header>

      {view === "dungeon-simulator" ? <DungeonSimulator onSimulate={simulateDungeon}/> : <>
      <section className="hero">
        <div className="character"><div className="avatar">{character.name.slice(0,1).toUpperCase()}</div><div><span>Personnage actif</span><h2>{character.name}</h2><p>{character.level > 0 ? `Niveau ${character.level} · ` : ""}{character.className}</p></div></div>
        <label className="selector"><Server size={17}/><select value={character.id} onChange={(e) => selectCharacter(e.target.value)}>{state.characters.map((item) => <option value={item.id} key={item.id}>{item.name} — {item.server}</option>)}</select><ChevronDown size={16}/></label>
        <div className="engine-state"><Bot size={20}/><div><span>Moteur</span><strong>{state.running ? "En fonctionnement" : "En pause"}</strong></div></div>
      </section>

      <section className="read-only-banner">
        <strong>Automatisation réelle · mode prudent</strong>
        <span>Taverne, Arène et Donjons peuvent agir. Le backend impose ≥ 5 s entre deux requêtes, respecte les timers locaux, bloque les doubles appels et applique un backoff après erreur. Aucun champignon n’est volontairement dépensé.</span>
      </section>

      <section className="automation-section">
        <div className="section-title"><div><h2>Automatisations</h2><p>Active les modules et observe leur prochaine décision.</p></div><span>{state.automations.filter((item) => item.enabled).length}/{state.automations.length} actives</span></div>
        <div className="grid">{state.automations.map((automation) => <AutomationCard key={automation.id} automation={automation} onToggle={() => toggleAutomation(automation.id)} onRun={() => void runNow(automation.id)}/>)}</div>
      </section>
      <Logs logs={state.logs}/>
      </>}
    </main>
  </div>;
}
