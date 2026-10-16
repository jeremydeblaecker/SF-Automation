import { useState } from "react";
import { FlaskConical, ShieldCheck, ShieldX } from "lucide-react";
import type { DungeonSimulationResult } from "../types/domain";

interface Props {
  onSimulate: (iterations: number) => Promise<DungeonSimulationResult>;
}

export function DungeonSimulator({ onSimulate }: Props) {
  const [iterations, setIterations] = useState(5000);
  const [result, setResult] = useState<DungeonSimulationResult>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const run = async () => {
    setLoading(true);
    setError(undefined);
    try {
      setResult(await onSimulate(iterations));
    } catch (e) {
      setResult(undefined);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return <section className="simulator-page">
    <div className="simulator-card panel">
      <div className="simulator-title">
        <div className="simulator-icon"><FlaskConical size={24}/></div>
        <div>
          <span className="eyebrow">SIMULATEUR LOCAL</span>
          <h2>Dungeon Simulator</h2>
          <p>Calcule la probabilité de victoire contre le premier donjon ouvert, sans requête S&amp;F.</p>
        </div>
      </div>

      <div className="simulation-controls">
        <label>
          <span>Nombre de combats simulés</span>
          <select value={iterations} onChange={(e) => setIterations(Number(e.target.value))}>
            <option value={100}>100</option>
            <option value={1000}>1 000</option>
            <option value={5000}>5 000</option>
            <option value={10000}>10 000</option>
            <option value={20000}>20 000</option>
          </select>
        </label>
        <button className="master" disabled={loading} onClick={() => void run()}>
          <FlaskConical size={17}/>{loading ? "Simulation…" : "Simuler"}
        </button>
      </div>

      {error && <div className="login-error">{error}</div>}

      {result && <div className="simulation-result">
        <div className="simulation-dungeon">
          <span>Donjon ciblé</span>
          <strong>{result.dungeon}</strong>
        </div>
        <div className="probability">
          <strong>{(result.winRatio * 100).toFixed(1)}%</strong>
          <span>de victoire estimée</span>
        </div>
        <div className="probability-bar"><div style={{ width: `${Math.min(result.winRatio * 100, 100)}%` }}/></div>
        <div className="simulation-meta">
          <span>{result.iterations.toLocaleString("fr-FR")} combats simulés</span>
          <span>Seuil automatique : {(result.threshold * 100).toFixed(0)}%</span>
        </div>
        <div className={result.recommended ? "recommendation good" : "recommendation bad"}>
          {result.recommended ? <ShieldCheck size={20}/> : <ShieldX size={20}/>}
          <div><strong>{result.recommended ? "Combat conseillé" : "Combat déconseillé"}</strong><span>{result.recommendation}</span></div>
        </div>
      </div>}
    </div>
  </section>;
}
