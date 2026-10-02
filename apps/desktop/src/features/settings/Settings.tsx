import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useUi } from "../../stores/ui";

async function get(key: string): Promise<string | null> {
  try {
    return await invoke<string | null>("get_setting", { key });
  } catch {
    return null;
  }
}

async function set(key: string, value: string): Promise<void> {
  try {
    await invoke("set_setting", { key, value });
  } catch {
    // Settings failures stay local; the toggle still reflects intent.
  }
}

function Row({
  label,
  hint,
  control,
}: {
  label: string;
  hint?: string;
  control: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 0" }}>
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 12, fontWeight: 600 }}>{label}</div>
        {hint && <div style={{ fontSize: 11, opacity: 0.65 }}>{hint}</div>}
      </div>
      {control}
    </div>
  );
}

function Toggle({
  on,
  onFlip,
}: {
  on: boolean;
  onFlip: () => void;
}) {
  return (
    <button
      onClick={onFlip}
      style={{
        width: 36,
        height: 20,
        borderRadius: 9999,
        border: "1px solid #30363d",
        background: on ? "#1f6feb" : "rgba(255,255,255,0.1)",
        cursor: "pointer",
        position: "relative",
        flexShrink: 0,
      }}
      aria-pressed={on}
    >
      <span
        style={{
          position: "absolute",
          top: 2,
          left: on ? 18 : 2,
          width: 14,
          height: 14,
          borderRadius: 9999,
          background: "#fff",
        }}
      />
    </button>
  );
}

export function Settings() {
  const [notif, setNotif] = useState(true);
  const [onTop, setOnTop] = useState(true);
  const [auto, setAuto] = useState(false);
  const animations = useUi((s) => s.animations);
  const setAnimations = useUi((s) => s.setAnimations);
  const opacity = useUi((s) => s.opacity);
  const setOpacity = useUi((s) => s.setOpacity);
  const setExpanded = useUi((s) => s.setExpanded);

  useEffect(() => {
    void (async () => {
      if ((await get("notifications")) === "0") setNotif(false);
      if ((await get("always_on_top")) === "0") setOnTop(false);
      const anim = await get("animations");
      if (anim !== null) setAnimations(anim !== "0");
      const op = await get("opacity");
      if (op !== null) {
        const v = Number.parseFloat(op);
        if (Number.isFinite(v)) setOpacity(Math.min(1, Math.max(0.4, v)));
      }
      try {
        setAuto(await invoke<boolean>("is_autostart", {}));
      } catch {
        // Autostart state unknown; leave default.
      }
    })();
  }, []);

  async function flipNotif() {
    const next = !notif;
    setNotif(next);
    await set("notifications", next ? "1" : "0");
  }

  async function flipOnTop() {
    const next = !onTop;
    setOnTop(next);
    await set("always_on_top", next ? "1" : "0");
    try {
      await invoke("set_always_on_top", { enabled: next });
    } catch {
      // Window call failed; stored value still applies next launch.
    }
  }

  async function flipAuto() {
    const next = !auto;
    setAuto(next);
    try {
      await invoke("set_autostart", { enabled: next });
    } catch {
      setAuto(!next);
    }
  }

  async function flipAnimations() {
    const next = !animations;
    setAnimations(next);
    await set("animations", next ? "1" : "0");
  }

  return (
    <div style={{ marginTop: 10 }}>
      <div style={{ display: "flex", alignItems: "center", marginBottom: 4 }}>
        <div style={{ fontSize: 12, fontWeight: 700, flex: 1 }}>Settings</div>
        <button
          onClick={() => setExpanded(false)}
          style={{ background: "none", border: "none", color: "#fff", cursor: "pointer", fontSize: 14, opacity: 0.7 }}
        >
          ✕
        </button>
      </div>
      <Row label="Notifications" hint="Permission, completed, error" control={<Toggle on={notif} onFlip={() => void flipNotif()} />} />
      <Row label="Always on top" control={<Toggle on={onTop} onFlip={() => void flipOnTop()} />} />
      <Row label="Start on login" control={<Toggle on={auto} onFlip={() => void flipAuto()} />} />
      <Row label="Animations" control={<Toggle on={animations} onFlip={() => void flipAnimations()} />} />
      <Row
        label={`Opacity ${Math.round(opacity * 100)}%`}
        control={
          <input
            type="range"
            min={40}
            max={100}
            value={Math.round(opacity * 100)}
            onChange={(e) => {
              const v = Number(e.target.value) / 100;
              setOpacity(v);
              void set("opacity", String(v));
            }}
            style={{ width: 110 }}
          />
        }
      />
      <Row
        label="Shortcut"
        hint="Toggle companion"
        control={<span style={{ fontSize: 11, opacity: 0.8, fontFamily: "monospace" }}>Super+Alt+N</span>}
      />
      <button
        onClick={() => {
          invoke("recenter", {}).catch(() => {});
        }}
        style={{
          width: "100%",
          marginTop: 4,
          padding: "6px 0",
          borderRadius: 8,
          border: "1px solid #30363d",
          background: "rgba(255,255,255,0.06)",
          color: "#fff",
          cursor: "pointer",
          fontSize: 12,
        }}
      >
        Recenter top-center
      </button>
    </div>
  );
}
