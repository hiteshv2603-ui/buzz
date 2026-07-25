/**
 * Pure logic helpers for the harness gallery (HarnessManagementCard).
 *
 * Extracted for deterministic unit-testing — no React, no Tauri, no network.
 */

import type { AcpRuntimeCatalogEntry } from "@/shared/api/types";

/**
 * Filter catalog entries to preset-only, sorted detected-first then
 * alphabetically within each group.
 *
 * "Detected" means availability === "available". This mirrors the
 * React.useMemo sort inside HarnessManagementCard.
 */
export function sortedPresetEntries(
  catalog: readonly AcpRuntimeCatalogEntry[],
): AcpRuntimeCatalogEntry[] {
  const presets = catalog.filter((e) => e.source === "preset");
  return [...presets].sort((a, b) => {
    const aDetected = a.availability === "available" ? 0 : 1;
    const bDetected = b.availability === "available" ? 0 : 1;
    if (aDetected !== bDetected) return aDetected - bDetected;
    return a.label.localeCompare(b.label);
  });
}

/**
 * Filter catalog entries to custom-only.
 */
export function customEntries(
  catalog: readonly AcpRuntimeCatalogEntry[],
): AcpRuntimeCatalogEntry[] {
  return catalog.filter((e) => e.source === "custom");
}

/**
 * Returns true iff the given catalog entry is editable by the user.
 * Only `source === "custom"` entries are editable/deletable.
 */
export function isEditableEntry(entry: AcpRuntimeCatalogEntry): boolean {
  return entry.source === "custom";
}
