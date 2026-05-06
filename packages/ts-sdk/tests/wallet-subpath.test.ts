import assert from "node:assert/strict";
import { test } from "node:test";

import { createWalletClient } from "../src/wallet/index.js";

test("wallet subpath exports the wallet client factory", () => {
  assert.equal(typeof createWalletClient, "function");
});
