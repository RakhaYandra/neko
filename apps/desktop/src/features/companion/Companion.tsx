import { AnimatePresence, motion } from "motion/react";
import {
  ACTIVE_FLOAT,
  ATTENTION_PULSE,
  SPRITE_URL,
  STATUS_META,
  poseCrop,
} from "./poses";
import type { NekoSession } from "../../stores/sessions";

export function Sprite({ status, size = 56 }: { status: string; size?: number }) {
  const { xPct, yPct, scale } = poseCrop(status);
  return (
    <div
      style={{
        width: size,
        height: size,
        overflow: "hidden",
        borderRadius: 12,
        position: "relative",
        flexShrink: 0,
      }}
    >
      <AnimatePresence mode="wait">
        <motion.img
          key={status}
          src={SPRITE_URL}
          alt=""
          draggable={false}
          className="pixelated"
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.9 }}
          transition={{ duration: 0.18 }}
          style={{
            position: "absolute",
            width: `${scale * 100}%`,
            maxWidth: "none",
            left: `-${xPct}%`,
            top: `-${yPct}%`,
          }}
        />
      </AnimatePresence>
    </div>
  );
}

export function StatusDot({ status, size = 8 }: { status: string; size?: number }) {
  const color = STATUS_META[status]?.color ?? "#8b949e";
  const pulse = ATTENTION_PULSE.has(status);
  return (
    <motion.span
      animate={pulse ? { opacity: [1, 0.35, 1] } : { opacity: 1 }}
      transition={pulse ? { duration: 1.1, repeat: Infinity } : {}}
      style={{
        width: size,
        height: size,
        borderRadius: 9999,
        background: color,
        display: "inline-block",
        flexShrink: 0,
      }}
    />
  );
}

export function Companion({ session }: { session: NekoSession | null }) {
  const status = session?.status ?? "disconnected";
  const meta = STATUS_META[status] ?? STATUS_META.idle;
  const float = ACTIVE_FLOAT.has(status);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <motion.div
        animate={float ? { y: [0, -3, 0] } : { y: 0 }}
        transition={float ? { duration: 2, repeat: Infinity, ease: "easeInOut" } : {}}
      >
        <Sprite status={status} />
      </motion.div>
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {session?.project ?? "Neko"}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, opacity: 0.85 }}>
          <StatusDot status={status} />
          {session ? meta.label : "waiting for OpenCode…"}
        </div>
      </div>
    </div>
  );
}
