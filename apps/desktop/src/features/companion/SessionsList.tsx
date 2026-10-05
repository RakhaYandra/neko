import { STATUS_META } from "./poses";
import type { NekoSession } from "../../stores/sessions";
import { StatusDot } from "./Companion";

export function shortId(id: string): string {
  return id.length <= 18 ? id : `${id.slice(0, 10)}…${id.slice(-6)}`;
}

export function age(ts: number): string {
  const s = Math.max(0, Math.round((Date.now() - ts) / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
}

const MONO = "'JetBrains Mono', 'ui-monospace', 'monospace'";

export function SessionsList({
  sessions,
  selectedId,
  onSelect,
}: {
  sessions: NekoSession[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  if (sessions.length === 0) {
    return (
      <div style={{ marginTop: 10, fontSize: 12, opacity: 0.7 }}>
        No sessions — waiting for OpenCode.
      </div>
    );
  }
  return (
    <div role="list" style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
      {sessions.map((s) => {
        const active = selectedId === s.id || (selectedId === null && sessions[0] === s);
        const meta = STATUS_META[s.status];
        const pill = meta?.color ?? "#8b949e";
        return (
          <button
            key={s.id}
            onClick={() => onSelect(active && selectedId !== null ? null : s.id)}
            aria-current={active ? "true" : undefined}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              padding: "6px 8px",
              borderRadius: 8,
              border: "none",
              cursor: "pointer",
              textAlign: "left",
              fontSize: 12,
              fontFamily: MONO,
              color: "#c9d1d9",
              background: active ? "rgba(88,166,255,0.18)" : "rgba(255,255,255,0.06)",
            }}
          >
            <StatusDot status={s.status} />
            <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {shortId(s.id)}
            </span>
            <span style={{ color: "#6b7d91", fontSize: 11 }}>{age(s.lastActivityAt)}</span>
            <span
              style={{
                border: `1px solid ${pill}`,
                borderRadius: 4,
                padding: "1px 6px",
                fontSize: 10,
                fontWeight: 700,
                color: pill,
              }}
            >
              {meta?.label ?? s.status}
            </span>
          </button>
        );
      })}
    </div>
  );
}
