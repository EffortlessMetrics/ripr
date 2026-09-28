import { describe, expect, it, test, vi } from "vitest";
import {
  applyDiscount,
  isReady,
  loadProfile,
  notifyLiteral,
  notifyStatus,
  renderSummary,
} from "../src/signals";

describe("pricing", () => {
  describe("discounts", () => {
    test("pins threshold", () => {
      expect(applyDiscount(100, 100)).toStrictEqual(90);
    });
  });
});

test.each([
  [150, 100, 140],
])("discount table %#", (amount, threshold, expected) => {
  expect(applyDiscount(amount, threshold)).toBe(expected);
});

test("renders summary snapshot", () => {
  expect(renderSummary("ready")).toMatchSnapshot();
});

it("smoke readiness", () => {
  expect(isReady(1)).toBeTruthy();
});

it.each([
  ["ready"],
])("records status %s", (status) => {
  const sink = { record: vi.fn() };
  notifyStatus(status, sink);
  expect(sink.record).toHaveBeenCalledWith(status);
});

it("records literal status", () => {
  const sink = { record: vi.fn() };
  notifyLiteral(sink);
  expect(sink.record).toHaveBeenCalledWith("ready");
});

test("loads profile", async () => {
  await expect(loadProfile("ada")).resolves.toBe("profile:ada");
});
