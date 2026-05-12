import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const pageSource = readFileSync(
  join(process.cwd(), "app/responsible-gaming/limits/page.tsx"),
  "utf8",
);

function extractLimitOptionsBlock(source: string): string {
  const match = source.match(/const limitOptions = \[([\s\S]*?)\];/);
  assert.ok(match, "limits page should define fixed limitOptions");
  return match[1] ?? "";
}

test("monthly deposit limit options match product policy", () => {
  const optionBlock = extractLimitOptionsBlock(pageSource);
  const values = [...optionBlock.matchAll(/value:\s*(\d+)/g)].map(([, value]) =>
    Number(value),
  );

  assert.deepEqual(values, [
    1000,
    3000,
    5000,
    10000,
    30000,
    50000,
    100000,
    300000,
    500000,
  ]);
});

test("monthly deposit limit options do not display fiat currency labels", () => {
  const optionBlock = extractLimitOptionsBlock(pageSource);

  assert.doesNotMatch(optionBlock, /\bTWD\b/);
});
