import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert";
import { createServer } from "node:net";
import { rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { NekoPlugin } from "../dist/index.js";

let sock = "";
let server = null;
let captured = [];
let prevSock = process.env.NEKO_SOCK;

function linesOf(buf) {
  return buf
    .split("\n")
    .filter((l) => l.trim() !== "")
    .map((l) => JSON.parse(l));
}

beforeEach(async () => {
  captured = [];
  sock = join(tmpdir(), `neko-test-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.sock`);
  try {
    rmSync(sock, { force: true });
  } catch {}
  server = createServer((conn) => {
    let buf = "";
    conn.on("data", (d) => {
      buf += d.toString();
    });
    conn.on("end", () => {
      try {
        captured.push(...linesOf(buf));
      } catch {}
    });
  });
  await new Promise((res) => server.listen(sock, res));
  process.env.NEKO_SOCK = sock;
});

afterEach(async () => {
  await new Promise((res) => server.close(res));
  try {
    rmSync(sock, { force: true });
  } catch {}
  process.env.NEKO_SOCK = prevSock;
});

async function waitFor(n, timeoutMs = 1000) {
  const start = Date.now();
  while (captured.length < n) {
    if (Date.now() - start > timeoutMs) break;
    await new Promise((r) => setTimeout(r, 20));
  }
}

function last() {
  assert.ok(captured.length > 0, "expected one socket line");
  return captured[captured.length - 1];
}

test("plugin exposes event + tool hooks without shell", async () => {
  const hooks = await NekoPlugin({});
  assert.equal(typeof hooks.event, "function");
  assert.equal(typeof hooks["tool.execute.before"], "function");
  assert.equal(typeof hooks["tool.execute.after"], "function");
});

test("event hook emits ADR-003 envelope for session.created", async () => {
  const hooks = await NekoPlugin({});
  await hooks.event({
    event: { type: "session.created", properties: { info: { id: "ses_1", title: "alpha" } } },
  });
  await waitFor(1);
  const m = last();
  assert.equal(m.v, 1);
  assert.equal(m.type, "session.created");
  assert.equal(typeof m.at, "number");
  assert.equal(m.sessionId, "ses_1");
  assert.equal(m.payload.project, "alpha");
});

test("payload quotes cannot inject shell (raw JSON preserved)", async () => {
  const hooks = await NekoPlugin({});
  const evil = `x'; touch /tmp/pwned; echo '`;
  await hooks.event({
    event: { type: "session.created", properties: { info: { id: "ses_evil", title: evil } } },
  });
  await waitFor(1);
  const m = last();
  assert.equal(m.payload.project, evil);
});

test("event hook adapts session.idle and permission.asked to neko types", async () => {
  const hooks = await NekoPlugin({});
  await hooks.event({ event: { type: "session.idle", properties: { sessionID: "ses_2" } } });
  await waitFor(1);
  assert.equal(last().type, "session.status");
  assert.equal(last().payload.status, "completed");
  assert.equal(last().sessionId, "ses_2");
  await hooks.event({
    event: {
      type: "permission.asked",
      properties: {
        id: "per_1",
        sessionID: "ses_2",
        permission: "bash",
        tool: { messageID: "m", callID: "c" },
      },
    },
  });
  await waitFor(2);
  const perm = last();
  assert.equal(perm.type, "permission.requested");
  assert.equal(perm.payload.action, "bash");
  assert.equal(perm.payload.requestId, "per_1");
  assert.ok(!("tool" in perm.payload));
});

test("permission.asked accepts requestID variant", async () => {
  const hooks = await NekoPlugin({});
  await hooks.event({
    event: {
      type: "permission.asked",
      properties: { requestID: "per_9", sessionID: "s", permission: "bash" },
    },
  });
  await waitFor(1);
  assert.equal(last().payload.requestId, "per_9");
});

test("unknown status defaults to working", async () => {
  const hooks = await NekoPlugin({});
  await hooks.event({
    event: { type: "session.status", properties: { sessionID: "s", status: { type: "zzz" } } },
  });
  await waitFor(1);
  assert.equal(last().payload.status, "working");
});

test("tool hook emits tool.started envelope", async () => {
  const hooks = await NekoPlugin({});
  await hooks["tool.execute.before"](
    { sessionID: "ses_3", tool: "bash" },
    { args: { command: "npm install" } },
  );
  await waitFor(1);
  const m = last();
  assert.equal(m.type, "tool.started");
  assert.equal(m.sessionId, "ses_3");
  assert.deepEqual(m.payload, { tool: "bash", ref: "npm install" });
});

test("event hook adapts error, diff, replied, file, todo", async () => {
  const hooks = await NekoPlugin({});
  const fire = (type, properties) => hooks.event({ event: { type, properties } });
  await fire("session.error", { sessionID: "s", error: "boom" });
  await fire("session.diff", { sessionID: "s", files: ["a.ts"] });
  await fire("permission.replied", { sessionID: "s", permission: "bash", response: "reject" });
  await fire("file.edited", { sessionID: "s", file: "src/a.ts" });
  await fire("todo.updated", { sessionID: "s" });
  await hooks["tool.execute.after"]({ sessionID: "s", tool: "bash" });
  await waitFor(6);
  const byType = (t) => captured.find((m) => m.type === t);
  assert.deepEqual(byType("session.error").payload, { message: "boom" });
  assert.deepEqual(byType("session.diff").payload, { files: ["a.ts"] });
  assert.equal(byType("permission.resolved").payload.decision, "deny");
  assert.deepEqual(byType("file.edited").payload, { path: "src/a.ts" });
  assert.equal(byType("todo.updated").type, "todo.updated");
  assert.equal(byType("tool.completed").type, "tool.completed");
});
