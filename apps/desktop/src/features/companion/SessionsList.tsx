import { STATUS_META } from "./poses";
import type { NekoSession } from "../../stores/sessions";
import { StatusDot } from "./Companion";

export function SessionsList({
  sessions,
  selectedId,
  onSelect,
}: {
  sessions: NekoSession[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
      {sessions.map((s) => {
        const active = selectedId === s.id || (selectedId === null && sessions[0] === s);
        return (
          <button
            key={s.id}
            onClick={() => onSelect(active && selectedId !== null ? null : s.id)}
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
              color: "#fff",
              background: active ? "rgba(88,166,255,0.18)" : "rgba(255,255,255,0.06)",
            }}
          >
            <StatusDot status={s.status} />
            <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {s.project}
            </span>
            <span style={{ opacity: 0.7 }}>{STATUS_META[s.status]?.label ?? s.status}</span>
          </button>
        );
      })}
    </div>
  );
}
