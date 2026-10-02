import { test } from "node:test";
import assert from "node:assert";
import { NekoPlugin } from "../dist/index.js";

test("plugin exposes event + tool hooks without running shell", async () => {
  const hooks = await NekoPlugin({});
  assert.equal(typeof hooks.event, "function");
  assert.equal(typeof hooks["tool.execute.before"], "function");
});
