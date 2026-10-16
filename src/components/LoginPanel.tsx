import { FormEvent, useState } from "react";
import { KeyRound, LogIn, Server, UserRound } from "lucide-react";
import type { LoginRequest } from "../core/gameClient";

interface Props {
  loading: boolean;
  error?: string;
  nativeMode: boolean;
  onLogin: (request: LoginRequest) => Promise<void>;
}

export function LoginPanel({ loading, error, nativeMode, onLogin }: Props) {
  const [kind, setKind] = useState<"account" | "character">("account");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [server, setServer] = useState("");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const request: LoginRequest = kind === "account"
      ? { kind, username, password }
      : { kind, username, password, server };
    await onLogin(request);
  };

  return <div className="login-wrap">
    <section className="login-panel">
      <div className="login-icon"><KeyRound size={24}/></div>
      <span className="eyebrow">CONNEXION SHAKES &amp; FIDGET</span>
      <h1>Connecter tes personnages</h1>
      <p className="login-copy">La connexion réelle est exécutée dans le backend Rust local via sf-api. Le mot de passe n’est pas enregistré par MercySF Reborn.</p>

      <div className="login-tabs">
        <button type="button" className={kind === "account" ? "active" : ""} onClick={() => setKind("account")}>Compte S&amp;F</button>
        <button type="button" className={kind === "character" ? "active" : ""} onClick={() => setKind("character")}>Ancien personnage</button>
      </div>

      <form onSubmit={submit}>
        <label><span><UserRound size={15}/> {kind === "account" ? "E-mail / identifiant S&F" : "Nom du personnage"}</span><input autoComplete="username" value={username} onChange={(e) => setUsername(e.target.value)} required /></label>
        <label><span><KeyRound size={15}/> Mot de passe</span><input type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} required /></label>
        {kind === "character" && <label><span><Server size={15}/> Serveur</span><input placeholder="ex. f1.sfgame.net" value={server} onChange={(e) => setServer(e.target.value)} required /></label>}
        {error && <div className="login-error">{error}</div>}
        {!nativeMode && <div className="login-warning">Prévisualisation navigateur : la connexion est simulée. Lance l’application Tauri pour joindre réellement S&amp;F.</div>}
        <button className="login-submit" disabled={loading}><LogIn size={17}/>{loading ? "Connexion…" : "Se connecter"}</button>
      </form>
    </section>
  </div>;
}
