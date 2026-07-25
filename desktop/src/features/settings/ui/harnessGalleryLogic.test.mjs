import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  sortedPresetEntries,
  customEntries,
  isEditableEntry,
} from "./harnessGalleryLogic.ts";

// ── Minimal catalog entry factory ────────────────────────────────────────────

function entry(overrides = {}) {
  return {
    id: overrides.id ?? "test-id",
    label: overrides.label ?? "Test",
    source: overrides.source ?? "custom",
    availability: overrides.availability ?? "not_installed",
    avatarUrl: "",
    command: overrides.command ?? null,
    binaryPath: null,
    defaultArgs: [],
    mcpCommand: null,
    modelEnvVar: null,
    providerEnvVar: null,
    thinkingEnvVar: null,
    installHint: "",
    installInstructionsUrl: "",
    canAutoInstall: false,
    underlyingCliPath: null,
    nodeRequired: false,
    authStatus: { status: "not_applicable" },
    loginHint: null,
  };
}

// ── sortedPresetEntries ───────────────────────────────────────────────────────

describe("sortedPresetEntries", () => {
  it("returns only preset-source entries", () => {
    const catalog = [
      entry({ id: "p1", source: "preset" }),
      entry({ id: "c1", source: "custom" }),
      entry({ id: "b1", source: "builtin" }),
    ];
    const result = sortedPresetEntries(catalog);
    assert.equal(result.length, 1);
    assert.equal(result[0].id, "p1");
  });

  it("places detected (available) entries before not-installed", () => {
    const catalog = [
      entry({
        id: "not-there",
        source: "preset",
        availability: "not_installed",
        label: "Alpha",
      }),
      entry({
        id: "detected",
        source: "preset",
        availability: "available",
        label: "Beta",
      }),
    ];
    const result = sortedPresetEntries(catalog);
    assert.equal(result[0].id, "detected", "detected entry must come first");
    assert.equal(result[1].id, "not-there");
  });

  it("sorts alphabetically within detected group", () => {
    const catalog = [
      entry({
        id: "z",
        source: "preset",
        availability: "available",
        label: "Zebra",
      }),
      entry({
        id: "a",
        source: "preset",
        availability: "available",
        label: "Aardvark",
      }),
    ];
    const result = sortedPresetEntries(catalog);
    assert.equal(result[0].id, "a");
    assert.equal(result[1].id, "z");
  });

  it("sorts alphabetically within not-installed group", () => {
    const catalog = [
      entry({
        id: "z",
        source: "preset",
        availability: "not_installed",
        label: "Zebra",
      }),
      entry({
        id: "a",
        source: "preset",
        availability: "not_installed",
        label: "Aardvark",
      }),
    ];
    const result = sortedPresetEntries(catalog);
    assert.equal(result[0].id, "a");
    assert.equal(result[1].id, "z");
  });

  it("returns empty array when no preset entries", () => {
    const catalog = [entry({ source: "custom" }), entry({ source: "builtin" })];
    assert.deepEqual(sortedPresetEntries(catalog), []);
  });

  it("does not mutate the input array", () => {
    const catalog = [
      entry({
        id: "z",
        source: "preset",
        availability: "available",
        label: "Z",
      }),
      entry({
        id: "a",
        source: "preset",
        availability: "available",
        label: "A",
      }),
    ];
    const original = [...catalog];
    sortedPresetEntries(catalog);
    assert.deepEqual(
      catalog.map((e) => e.id),
      original.map((e) => e.id),
      "input array must not be mutated",
    );
  });
});

// ── customEntries ─────────────────────────────────────────────────────────────

describe("customEntries", () => {
  it("returns only custom-source entries", () => {
    const catalog = [
      entry({ id: "p1", source: "preset" }),
      entry({ id: "c1", source: "custom" }),
      entry({ id: "c2", source: "custom" }),
    ];
    const result = customEntries(catalog);
    assert.equal(result.length, 2);
    assert.ok(result.every((e) => e.source === "custom"));
  });

  it("returns empty when no custom entries", () => {
    const catalog = [entry({ source: "preset" }), entry({ source: "builtin" })];
    assert.deepEqual(customEntries(catalog), []);
  });
});

// ── isEditableEntry ───────────────────────────────────────────────────────────

describe("isEditableEntry", () => {
  it("returns true for custom entries", () => {
    assert.ok(isEditableEntry(entry({ source: "custom" })));
  });

  it("returns false for preset entries", () => {
    assert.ok(!isEditableEntry(entry({ source: "preset" })));
  });

  it("returns false for builtin entries", () => {
    assert.ok(!isEditableEntry(entry({ source: "builtin" })));
  });
});
