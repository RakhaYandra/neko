import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useUi } from "../../stores/ui";

async function get(key: string): Promise<string | null> {
  try {
    return await invoke<string | null>("get_setting", { key });
  } catch {
    return null;
  }
}

async function set(key: string, value: string): Promise<boolean> {
  try {
    await invoke("set_setting", { key, value });
    return true;
  } catch {
    // Settings failures stay local; caller rolls back the toggle.
    return false;
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
  label,
  on,
  onFlip,
}: {
  label: string;
  on: boolean;
  onFlip: () => void;
}) {
  return (
    <button
      onClick={onFlip}
      aria-label={label}
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
  const [companion, setCompanion] = useState(false);
  const animations = useUi((s) => s.animations);
  const setAnimations = useUi((s) => s.setAnimations);
  const opacity = useUi((s) => s.opacity);
  const setOpacity = useUi((s) => s.setOpacity);
  const setExpanded = useUi((s) => s.setExpanded);

  useEffect(() => {
    let mounted = true;
    void (async () => {
      if ((await get("notifications")) === "0" && mounted) setNotif(false);
      if ((await get("always_on_top")) === "0" && mounted) setOnTop(false);
      if ((await get("companion_visible")) === "1" && mounted) setCompanion(true);
      const anim = await get("animations");
      if (anim !== null && mounted) setAnimations(anim !== "0");
      const op = await get("opacity");
      if (op !== null && mounted) {
        const v = Number.parseFloat(op);
        if (Number.isFinite(v)) setOpacity(Math.min(1, Math.max(0.4, v)));
      }
      try {
        const auto = await invoke<boolean>("is_autostart", {});
        if (mounted) setAuto(auto);
      } catch {
        // Autostart state unknown; leave default.
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

  async function flipNotif() {
    const prev = notif;
    const next = !prev;
    setNotif(next);
    if (!(await set("notifications", next ? "1" : "0"))) setNotif(prev);
  }

  async function flipOnTop() {
    const prev = onTop;
    const next = !prev;
    setOnTop(next);
    try {
      if (!(await set("always_on_top", next ? "1" : "0"))) throw new Error("persist failed");
      await invoke("set_always_on_top", { enabled: next });
    } catch {
      setOnTop(prev);
    }
  }

  async function flipCompanion() {
    const prev = companion;
    const next = !prev;
    setCompanion(next);
    try {
      await invoke("set_companion_visible", { enabled: next });
    } catch {
      setCompanion(prev);
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
    const prev = animations;
    const next = !prev;
    setAnimations(next);
    if (!(await set("animations", next ? "1" : "0"))) setAnimations(prev);
  }

  const opacityTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (opacityTimer.current) clearTimeout(opacityTimer.current);
    };
  }, []);

  function changeOpacity(v: number) {
    setOpacity(v);
    if (opacityTimer.current) clearTimeout(opacityTimer.current);
    opacityTimer.current = setTimeout(() => {
      void set("opacity", String(v));
    }, 200);
  }

  function commitOpacity(v: number) {
    if (opacityTimer.current) clearTimeout(opacityTimer.current);
    void set("opacity", String(v));
  }

  return (
    <div style={{ marginTop: 10 }}>
      <div style={{ display: "flex", alignItems: "center", marginBottom: 4 }}>
        <div style={{ fontSize: 12, fontWeight: 700, flex: 1 }}>Settings</div>
        <button
          onClick={() => setExpanded(false)}
          aria-label="Close settings"
          style={{ background: "none", border: "none", color: "#fff", cursor: "pointer", fontSize: 14, opacity: 0.7 }}
        >
          ✕
        </button>
      </div>
      <Row label="Notifications" hint="Permission, completed, error, question" control={<Toggle label="Notifications" on={notif} onFlip={() => void flipNotif()} />} />
      <Row label="Companion window" hint="Off = bar-only mode" control={<Toggle label="Companion window" on={companion} onFlip={() => void flipCompanion()} />} />
      <Row label="Always on top" control={<Toggle label="Always on top" on={onTop} onFlip={() => void flipOnTop()} />} />
      <Row label="Start on login" control={<Toggle label="Start on login" on={auto} onFlip={() => void flipAuto()} />} />
      <Row label="Animations" control={<Toggle label="Animations" on={animations} onFlip={() => void flipAnimations()} />} />
      <Row
        label={`Opacity ${Math.round(opacity * 100)}%`}
        control={
          <input
            type="range"
            aria-label="Opacity"
            min={40}
            max={100}
            value={Math.round(opacity * 100)}
            onChange={(e) => {
              changeOpacity(Number(e.target.value) / 100);
            }}
            onPointerUp={(e) => {
              commitOpacity(Number((e.target as HTMLInputElement).value) / 100);
            }}
            onBlur={(e) => {
              commitOpacity(Number((e.target as HTMLInputElement).value) / 100);
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
