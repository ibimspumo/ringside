import assert from "node:assert/strict";
import test from "node:test";

import { STAGE, STAGE_MIDDLE, STAGE_SIZES, clear, layout } from "../src/lib/table/seats.ts";

test("the fixed table keeps seats, chips and the middle apart at 2 to 10 seats", () => {
  for (let n = 2; n <= 10; n++) assert.ok(clear(layout(n, STAGE.w, STAGE.h, STAGE_SIZES), STAGE_SIZES, STAGE_MIDDLE), `${n} seats`);
});

test("the felt is shaped like a table, about twice as wide as high", () => {
  const { felt } = layout(9, STAGE.w, STAGE.h, STAGE_SIZES);
  const ratio = felt.rx / felt.ry;
  assert.ok(ratio > 1.8 && ratio < 2.2, `felt ${ratio.toFixed(2)} : 1`);
});
