import type { ActionLog } from "../types/domain";
export function Logs({ logs }: { logs: ActionLog[] }) {
  return <section className="panel logs"><div className="section-title"><div><h2>Activité</h2><p>Décisions et actions du moteur</p></div><span>{logs.length} événements</span></div><div className="log-list">{logs.length === 0 && <div className="empty">Aucune activité.</div>}{logs.map((log) => <div className="log" key={log.id}><time>{new Date(log.at).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</time><span className={`badge ${log.level}`}>{log.level}</span><p>{log.message}</p></div>)}</div></section>;
}
