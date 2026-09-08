import assert from "node:assert/strict";
import test from "node:test";
import { learningCases, learningPairs } from "../bench-learning-store-cases.mjs";

test("learning benchmark rotates configurations without dropping or duplicating any pair", () => {
  const original = structuredClone(learningCases);
  const first = [];
  for (let round = 0; round < 3; round++) {
    const pairs = learningPairs(round);
    assert.equal(pairs.length, 12);
    assert.deepEqual(pairs.map(pair => pair.item.name).sort(), original.map(item => item.name).sort());
    for (const pair of pairs) assert.deepEqual([...pair.phases].sort(), ["after", "before"]);
    first.push(pairs[0].item.name);
  }
  assert.equal(new Set(first).size, 3);
  assert.deepEqual(learningCases, original, "ordering must not mutate shared configurations");
});

test("every learning benchmark configuration swaps its before/after order each round", () => {
  for (const item of learningCases) {
    const orders = [0, 1, 2].map(round => learningPairs(round).find(pair => pair.item.name === item.name).phases);
    assert.notDeepEqual(orders[0], orders[1], item.name);
    assert.notDeepEqual(orders[1], orders[2], item.name);
    assert.deepEqual(orders[0], orders[2], item.name);
  }
});
