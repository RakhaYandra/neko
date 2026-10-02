import { test } from "node:test";
import assert from "node:assert";
import { NekoPlugin } from "../dist/index.js";

test("plugin exposes event + tool hooks without running shell", async () => {
  const hooks = await NekoPlugin({});
  assert.equal(typeof hooks.event, "function");
  assert.equal(typeof hooks["tool.execute.before"], "function");
  assert.equal(typeof hooks["tool.execute.after"], "function");
});

// Fake `$` template tag: captures the printf payload instead of shelling out.
function fakeCtx(captured) {
  const $ = (strings, ...values) => {
    captured.push(String(values[0] ?? ""));
    return Promise.resolve("");
  };
  return { $ };
}

function lastEnvelope(captured) {
  assert.ok(captured.length > 0, "expected one socket line");
  return JSON.parse(captured[captured.length - 1]);
}

test("event hook emits ADR-003 envelope for session.created", async () => {
  const captured = [];
  const hooks = await NekoPlugin(fakeCtx(captured));
  await hooks.event({
    event: { type: "session.created", properties: { info: { id: "ses_1", title: "pulse" } } },
  });
  const m = lastEnvelope(captured);
  assert.equal(m.v, 1);
  assert.equal(m.type, "session.created");
  assert.equal(typeof m.at, "number");
  assert.equal(m.sessionId, "ses_1");
  assert.equal(m.payload.project, "pulse");
});

test("event hook adapts session.idle and permission.asked to neko types", async () => {
  const captured = [];
  const hooks = await NekoPlugin(fakeCtx(captured));
  await hooks.event({ event: { type: "session.idle", properties: { sessionID: "ses_2" } } });
  assert.deepEqual(lastEnvelope(captured), {
    v: 1,
    type: "session.status",
    at: lastEnvelope(captured).at,
    sessionId: "ses_2",
    payload: { status: "completed" },
  });
  await hooks.event({
    event: { type: "permission.asked", properties: { sessionID: "ses_2", permission: "bash" } },
  });
  const perm = lastEnvelope(captured);
  assert.equal(perm.type, "permission.requested");
  assert.equal(perm.payload.action, "bash");
});

test("tool hook emits tool.started envelope", async () => {
  const captured = [];
  const hooks = await NekoPlugin(fakeCtx(captured));
  await hooks["tool.execute.before"](
    { sessionID: "ses_3", tool: "bash" },
    { args: { command: "npm install" } },
  );
  const m = lastEnvelope(captured);
  assert.equal(m.type, "tool.started");
  assert.equal(m.sessionId, "ses_3");
  assert.deepEqual(m.payload, { tool: "bash", ref: "npm install" });
});

test("event hook adapts error, diff, replied, file, todo", async () => {
  const captured = [];
  const hooks = await NekoPlugin(fakeCtx(captured));
  const fire = (type, properties) => hooks.event({ event: { type, properties } });
  await fire("session.error", { sessionID: "s", error: "boom" });
  assert.deepEqual(lastEnvelope(captured).payload, { message: "boom" });
  await fire("session.diff", { sessionID: "s", files: ["a.ts"] });
  assert.deepEqual(lastEnvelope(captured).payload, { files: ["a.ts"] });
  await fire("permission.replied", { sessionID: "s", permission: "bash", response: "reject" });
  const r = lastEnvelope(captured);
  assert.equal(r.type, "permission.resolved");
  assert.deepEqual(r.payload, { action: "bash", decision: "deny" });
  await fire("file.edited", { sessionID: "s", file: "src/a.ts" });
  assert.deepEqual(lastEnvelope(captured).payload, { path: "src/a.ts" });
  await fire("todo.updated", { sessionID: "s" });
  assert.equal(lastEnvelope(captured).type, "todo.updated");
  await hooks["tool.execute.after"]({ sessionID: "s", tool: "bash" });
  const t = lastEnvelope(captured);
  assert.equal(t.type, "tool.completed");
  assert.deepEqual(t.payload, { tool: "bash" });
});
