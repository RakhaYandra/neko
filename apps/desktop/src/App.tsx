import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export default function App() {
  const [last, setLast] = useState<string | null>(null);
  useEffect(() => {
    let off: (() => void) | undefined;
    listen<string>("neko-event", (e) => setLast(e.payload)).then((f) => (off = f));
    return () => off?.();
  }, []);
  return (
    <main style={{ fontFamily: "sans-serif", padding: 16, color: "#fff", background: "rgba(20,20,28,0.85)", borderRadius: 12 }}>
      <div>🐱 {last ? "OpenCode connected" : "waiting for OpenCode…"}</div>
      {last && <pre style={{ fontSize: 11, opacity: 0.8 }}>{last}</pre>}
    </main>
  );
}
