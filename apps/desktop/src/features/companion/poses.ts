import spriteUrl from "../../assets/neko-sprite.svg";

// Atlas geometry (verified from SVG source + icon extraction):
// 512px canvas, cards at x = 24 + 120*col, y = 64 + 112*row, card 104x100.
// Companion crops the cat region: card-relative (18,18,80,80).
export const SHEET = 512;
const ORIG_X = 24;
const ORIG_Y = 64;
const PITCH_X = 120;
const PITCH_Y = 112;
const CROP_OFF = 18;
const CROP = 80;

export const SPRITE_URL = spriteUrl;

// Card index per Neko state (POSE-MAP.md). Secondary poses reserved.
export const STATE_POSE: Record<string, number> = {
  disconnected: 6,
  idle: 0,
  working: 1,
  tool_running: 2,
  waiting_permission: 3,
  completed: 4,
  error: 5,
};

export const STATUS_META: Record<string, { label: string; color: string }> = {
  disconnected: { label: "Disconnected", color: "#6e7681" },
  idle: { label: "Idle", color: "#8b949e" },
  working: { label: "Working", color: "#58a6ff" },
  tool_running: { label: "Tool running", color: "#58a6ff" },
  waiting_permission: { label: "Needs you", color: "#ffb224" },
  completed: { label: "Done", color: "#3fb950" },
  error: { label: "Error", color: "#f85149" },
};

// Active states get a gentle float; waiting pulses for attention.
export const ACTIVE_FLOAT = new Set(["working", "tool_running"]);
export const ATTENTION_PULSE = new Set(["waiting_permission"]);

export function poseCrop(status: string): { xPct: number; yPct: number; scale: number } {
  const idx = STATE_POSE[status] ?? 0;
  const col = idx % 4;
  const row = Math.floor(idx / 4);
  const x = ORIG_X + col * PITCH_X + CROP_OFF;
  const y = ORIG_Y + row * PITCH_Y + CROP_OFF;
  // <img> rendered at 512/80 = 6.4x inside a square crop container.
  const scale = SHEET / CROP;
  return { xPct: (x / CROP) * 100, yPct: (y / CROP) * 100, scale };
}
