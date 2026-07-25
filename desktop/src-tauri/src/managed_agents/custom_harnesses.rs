//! Loader for user-defined ACP harness definitions.
//!
//! Users drop JSON files into `<app-data>/custom_harnesses/` to register
//! arbitrary ACP-speaking agents without modifying the app or opening a PR.
//! Each file describes a single harness; the loader validates, warns on
//! invalid entries, and never propagates errors to the discovery caller.
//!
//! **Security constraint (Will-ratified):** custom definitions carry NO install
//! shell commands. `can_auto_install` is always `false` for custom entries.
//! Only tier-1 compiled-in runtimes retain install-script power.
//!
//! **Avatar URL security:** custom/preset catalog entries MUST NOT carry
//! user-supplied avatar URLs. `HarnessDefinition` intentionally omits
//! `avatar_url` — all icons are bundled assets keyed via `RUNTIME_LOGOS`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Regex-equivalent predicate for a valid harness ID.
///
/// IDs must match `[a-z0-9_][a-z0-9_-]*` — lowercase alphanumeric plus
/// hyphens and underscores, starting with an alphanumeric or underscore.
/// This mirrors goose's `generate_id` validation and is intentionally
/// more restrictive than the filesystem to prevent path-traversal tricks.
fn is_valid_harness_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Public re-export of `is_valid_harness_id` for callers outside this module
/// (e.g., the `delete_custom_harness` command that must validate caller-supplied ids).
pub(crate) fn is_valid_harness_id_pub(id: &str) -> bool {
    is_valid_harness_id(id)
}

/// User-supplied harness definition deserialized from a JSON file.
///
/// Only the fields a custom harness definition is permitted to carry are
/// included here — install commands and avatar URLs are intentionally absent
/// (security line: no remote icon URLs from user-editable config).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HarnessDefinition {
    /// Unique identifier, must match `[a-z0-9_][a-z0-9_-]*`.
    pub id: String,
    /// Human-readable name shown in the UI.
    pub label: String,
    /// Primary executable name or absolute path. May not be empty.
    pub command: String,
    /// Default CLI arguments passed to the command (array, not split-string).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables injected at spawn time. Definition env is applied
    /// first and LOSES on conflict with Buzz-injected vars — `BUZZ_MANAGED_AGENT`
    /// is always authoritative and cannot be overridden here.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Link to external docs for manual install/setup instructions.
    #[serde(default)]
    pub install_instructions_url: String,
    /// Human-readable install hint shown in Doctor.
    #[serde(default)]
    pub install_hint: String,
}

/// Scan `dir` for `*.json` files and deserialize each into a `HarnessDefinition`.
///
/// Errors per file are logged with `tracing::warn` and skipped — a single
/// malformed file never fails discovery for the rest.  Returns only
/// structurally valid, individually validated definitions.
///
/// **Callers must supply a fresh `dir` path on every `discover_acp_runtimes`
/// call** — this function performs no caching, mirroring goose's
/// `refresh_custom_providers()` pattern.
pub(crate) fn load_custom_harnesses(dir: &Path) -> Vec<HarnessDefinition> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(err) => {
            tracing::warn!(
                "custom_harnesses: cannot read directory {}: {err}",
                dir.display()
            );
            return vec![];
        }
    };

    let mut definitions = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!("custom_harnesses: failed to read {}: {err}", path.display());
                continue;
            }
        };

        let def: HarnessDefinition = match serde_json::from_str(&contents) {
            Ok(d) => d,
            Err(err) => {
                tracing::warn!(
                    "custom_harnesses: invalid JSON in {}: {err}",
                    path.display()
                );
                continue;
            }
        };

        if let Err(reason) = validate_harness_definition(&def) {
            tracing::warn!("custom_harnesses: skipping {} — {reason}", path.display());
            continue;
        }

        definitions.push(def);
    }

    definitions
}

/// Validate a deserialized `HarnessDefinition` against the invariants that
/// the rest of the discovery code depends on.
fn validate_harness_definition(def: &HarnessDefinition) -> Result<(), String> {
    if def.id.is_empty() {
        return Err("id must not be empty".into());
    }
    if !is_valid_harness_id(&def.id) {
        return Err(format!(
            "id {:?} does not match [a-z0-9_][a-z0-9_-]* — use lowercase letters, digits, hyphens, and underscores only",
            def.id
        ));
    }
    if def.command.trim().is_empty() {
        return Err("command must not be empty".into());
    }
    if def.label.trim().is_empty() {
        return Err("label must not be empty".into());
    }
    Ok(())
}

/// Public wrapper so the `save_custom_harness` Tauri command can validate
/// without duplicating the rules.
pub(crate) fn validate_harness_definition_pub(def: &HarnessDefinition) -> Result<(), String> {
    validate_harness_definition(def)
}

// ── Built-in ID set ──────────────────────────────────────────────────────────

/// IDs reserved for the compiled-in catalog. A custom definition whose `id`
/// collides with a built-in or preset is rejected to prevent shadowing (e.g. a
/// file called `cursor.json` hiding the pre-existing tier-2 preset).
///
/// NOTE: keep this list in sync with `PRESET_HARNESSES` in `discovery.rs`.
const BUILTIN_IDS: &[&str] = &[
    // Tier-1 first-class runtimes:
    "goose",
    "claude",
    "codex",
    "buzz-agent",
    // Tier-2 preset harnesses:
    "cursor",
    "omp",
    "grok",
    "opencode",
    "kimi",
    "amp",
    "hermes",
    "openclaw",
];

/// Return an error string if `id` conflicts with a built-in harness ID.
pub(crate) fn check_id_collision(id: &str) -> Result<(), String> {
    if BUILTIN_IDS.contains(&id) {
        return Err(format!(
            "id {:?} is reserved for a built-in harness and cannot be overridden",
            id
        ));
    }
    Ok(())
}

// ── Loaded harness registry (F2 — spawn resolution for custom/preset) ────────
//
// `known_acp_runtime` / `known_acp_runtime_exact` only search the static
// `KNOWN_ACP_RUNTIMES` table, so custom and preset harnesses were invisible at
// spawn time, causing silent fallback to buzz-agent.
//
// The fix: `discover_acp_runtimes_from` populates this registry with every
// non-builtin definition after each discovery run. Spawn, readiness, and
// summary paths query `lookup_loaded_harness` to get the live definition for a
// given id or command. If a harness id that an agent references is gone from the
// registry, the caller gets a typed error — never a silent buzz-agent fallback.

use std::sync::{Arc, RwLock};

/// Thread-safe registry of non-builtin (preset + custom) harness definitions,
/// populated on every `discover_acp_runtimes_from` call and queried at spawn time.
fn loaded_harness_registry() -> &'static RwLock<Vec<Arc<HarnessDefinition>>> {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<RwLock<Vec<Arc<HarnessDefinition>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// Replace the registry contents with `definitions`. Called once per
/// `discover_acp_runtimes_from` run AND on `save_custom_harness` /
/// `delete_custom_harness` so spawn can always resolve the harness without
/// waiting for the next full discovery.
pub(crate) fn update_loaded_harness_registry(definitions: Vec<HarnessDefinition>) {
    let arcs: Vec<Arc<HarnessDefinition>> = definitions.into_iter().map(Arc::new).collect();
    // Use `into_inner` to recover from a poisoned lock — the registry is a
    // plain replaceable Vec with no torn invariant, so poison recovery is safe.
    let mut guard = match loaded_harness_registry().write() {
        Ok(g) => g,
        Err(poisoned) => {
            tracing::warn!("custom_harnesses: loaded-harness registry was poisoned; recovering");
            poisoned.into_inner()
        }
    };
    *guard = arcs;
}

/// Look up a loaded (non-builtin) harness by **id**. Returns `None` when the id
/// is unknown. Uses `into_inner` to recover from a poisoned lock so a panic in
/// one thread never permanently blocks all spawn attempts.
pub(crate) fn lookup_loaded_harness_by_id(id: &str) -> Option<Arc<HarnessDefinition>> {
    let guard = match loaded_harness_registry().read() {
        Ok(g) => g,
        Err(poisoned) => {
            tracing::warn!(
                "custom_harnesses: loaded-harness registry read lock was poisoned; recovering"
            );
            poisoned.into_inner()
        }
    };
    guard.iter().find(|d| d.id == id).cloned()
}

/// Warm the loaded-harness registry synchronously from `custom_dir`.
///
/// Must be called **before** `restore_managed_agents_on_launch` so that cold
/// relaunches can resolve custom/preset harness ids without a full discover
/// round-trip (which is driven by the frontend and arrives later).
///
/// This is intentionally lightweight: it only loads the custom JSON files and
/// the static preset list — no PATH probing, no availability checks.
pub(crate) fn warm_harness_registry_from_dir(custom_dir: Option<&std::path::Path>) {
    // Load only the preset list from the discovery module (static, free).
    let preset_defs = crate::managed_agents::discovery::preset_harness_definitions();
    let custom_defs = custom_dir.map(load_custom_harnesses).unwrap_or_default();
    let mut all: Vec<HarnessDefinition> = preset_defs;
    all.extend(custom_defs);
    update_loaded_harness_registry(all);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── ID validation ────────────────────────────────────────────────────────

    #[test]
    fn valid_id_lowercase_with_hyphen() {
        assert!(is_valid_harness_id("my-agent"));
    }

    #[test]
    fn valid_id_underscore_start() {
        assert!(is_valid_harness_id("_my_agent"));
    }

    #[test]
    fn valid_id_alphanumeric() {
        assert!(is_valid_harness_id("agent42"));
    }

    #[test]
    fn invalid_id_uppercase() {
        assert!(!is_valid_harness_id("MyAgent"));
    }

    #[test]
    fn invalid_id_starts_with_hyphen() {
        assert!(!is_valid_harness_id("-bad-id"));
    }

    #[test]
    fn invalid_id_empty() {
        assert!(!is_valid_harness_id(""));
    }

    #[test]
    fn invalid_id_path_traversal() {
        assert!(!is_valid_harness_id("../etc/passwd"));
    }

    // ── Collision check ──────────────────────────────────────────────────────

    #[test]
    fn builtin_ids_are_rejected() {
        for id in BUILTIN_IDS {
            assert!(check_id_collision(id).is_err(), "{id} should be rejected");
        }
    }

    #[test]
    fn unknown_id_passes_collision_check() {
        assert!(check_id_collision("my-custom-agent").is_ok());
    }

    // ── File loading ─────────────────────────────────────────────────────────

    #[test]
    fn load_valid_json_returns_definition() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("my-agent.json"),
            r#"{"id":"my-agent","label":"My Agent","command":"my-agent-bin"}"#,
        )
        .unwrap();

        let defs = load_custom_harnesses(dir.path());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].id, "my-agent");
        assert_eq!(defs[0].label, "My Agent");
        assert_eq!(defs[0].command, "my-agent-bin");
    }

    #[test]
    fn load_skips_non_json_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("my-agent.toml"), r#"id = "my-agent""#).unwrap();

        let defs = load_custom_harnesses(dir.path());
        assert_eq!(defs.len(), 0, "non-JSON file should be ignored");
    }

    #[test]
    fn load_skips_invalid_json_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bad.json"), "{ not valid json").unwrap();

        // Must not panic or propagate an error.
        let defs = load_custom_harnesses(dir.path());
        assert_eq!(defs.len(), 0);
    }

    #[test]
    fn load_skips_definition_with_invalid_id() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Bad.json"),
            r#"{"id":"Bad-Id","label":"Bad","command":"bad"}"#,
        )
        .unwrap();

        let defs = load_custom_harnesses(dir.path());
        assert_eq!(
            defs.len(),
            0,
            "invalid id should cause the entry to be skipped"
        );
    }

    #[test]
    fn load_skips_definition_with_empty_command() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("empty-cmd.json"),
            r#"{"id":"empty-cmd","label":"Empty","command":""}"#,
        )
        .unwrap();

        let defs = load_custom_harnesses(dir.path());
        assert_eq!(
            defs.len(),
            0,
            "empty command should cause the entry to be skipped"
        );
    }

    #[test]
    fn load_missing_dir_returns_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("does_not_exist");

        let defs = load_custom_harnesses(&nonexistent);
        assert_eq!(defs.len(), 0);
    }

    #[test]
    fn load_continues_after_one_bad_entry() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bad.json"), "!!!").unwrap();
        fs::write(
            dir.path().join("good.json"),
            r#"{"id":"good-one","label":"Good","command":"good-binary"}"#,
        )
        .unwrap();

        let defs = load_custom_harnesses(dir.path());
        assert_eq!(defs.len(), 1, "bad entry skipped, good entry loaded");
        assert_eq!(defs[0].id, "good-one");
    }

    #[test]
    fn load_applies_id_collision_check() {
        // A custom file named "goose.json" with id "goose" must be rejected.
        // The collision check is applied inside discover_acp_runtimes_from, not
        // in load_custom_harnesses — the file loader only validates the struct.
        // We test the check_id_collision fn directly here.
        assert!(check_id_collision("goose").is_err());
        assert!(check_id_collision("custom-goose").is_ok());
    }

    // ── Round-trip: save → load → delete → load ──────────────────────────────

    #[test]
    fn round_trip_save_then_load_then_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mut env_map = BTreeMap::new();
        env_map.insert("MY_KEY".to_string(), "my_value".to_string());
        let def = HarnessDefinition {
            id: "my-rt".to_string(),
            label: "My Runtime".to_string(),
            command: "my-rt-bin".to_string(),
            args: vec!["--flag".to_string()],
            env: env_map,
            install_instructions_url: "https://example.com".to_string(),
            install_hint: "Install from example.com".to_string(),
        };

        // Serialize and write (simulating save_custom_harness logic).
        let json = serde_json::to_string_pretty(&def).unwrap();
        let target = dir.path().join(format!("{}.json", def.id));
        fs::write(&target, &json).unwrap();

        // Load should return exactly one entry.
        let loaded = load_custom_harnesses(dir.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "my-rt");
        assert_eq!(loaded[0].command, "my-rt-bin");
        assert_eq!(loaded[0].args, vec!["--flag"]);
        assert_eq!(
            loaded[0].env.get("MY_KEY").map(String::as_str),
            Some("my_value")
        );

        // Delete the file (simulating delete_custom_harness).
        fs::remove_file(&target).unwrap();

        // Load should now return an empty list.
        let after_delete = load_custom_harnesses(dir.path());
        assert!(
            after_delete.is_empty(),
            "directory should be empty after delete"
        );
    }

    #[test]
    fn round_trip_overwrite_existing_definition() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rt.json");

        // Write v1.
        fs::write(&path, r#"{"id":"rt","label":"V1","command":"rt-bin"}"#).unwrap();

        let v1 = load_custom_harnesses(dir.path());
        assert_eq!(v1[0].label, "V1");

        // Overwrite with v2 (simulates save on an existing definition).
        fs::write(&path, r#"{"id":"rt","label":"V2","command":"rt-bin-v2"}"#).unwrap();

        let v2 = load_custom_harnesses(dir.path());
        assert_eq!(v2.len(), 1, "overwrite must not duplicate entries");
        assert_eq!(v2[0].label, "V2");
        assert_eq!(v2[0].command, "rt-bin-v2");
    }

    // ── Registry warm path ───────────────────────────────────────────────────

    /// After `warm_harness_registry_from_dir` the registry contains preset +
    /// custom definitions and `lookup_loaded_harness_by_id` resolves them.
    #[test]
    fn warm_registry_then_lookup_finds_custom_and_preset_entries() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("my-custom.json"),
            r#"{"id":"my-custom","label":"My Custom","command":"my-custom-bin"}"#,
        )
        .unwrap();

        warm_harness_registry_from_dir(Some(dir.path()));

        // Custom entry must be findable.
        let found = lookup_loaded_harness_by_id("my-custom");
        assert!(
            found.is_some(),
            "warm registry must contain the custom entry"
        );
        assert_eq!(found.unwrap().command, "my-custom-bin");

        // At least one preset entry must be in the registry (e.g. "cursor").
        let preset = lookup_loaded_harness_by_id("cursor");
        assert!(
            preset.is_some(),
            "warm registry must contain preset entries"
        );
    }

    /// `warm_harness_registry_from_dir` with `None` still loads presets.
    #[test]
    fn warm_registry_with_no_custom_dir_loads_presets_only() {
        warm_harness_registry_from_dir(None);
        // At least the "cursor" preset must be present.
        assert!(
            lookup_loaded_harness_by_id("cursor").is_some(),
            "presets must be reachable even without a custom dir"
        );
    }

    /// `warm_harness_registry_from_dir` followed by `update_loaded_harness_registry`
    /// with an empty slice clears the registry (transactional save/delete contract).
    #[test]
    fn warm_then_clear_registry_empties_lookup() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("tmp-agent.json"),
            r#"{"id":"tmp-agent","label":"Tmp","command":"tmp-bin"}"#,
        )
        .unwrap();

        warm_harness_registry_from_dir(Some(dir.path()));
        assert!(lookup_loaded_harness_by_id("tmp-agent").is_some());

        // Simulate delete — re-warm with empty dir.
        let empty_dir = tempfile::tempdir().unwrap();
        warm_harness_registry_from_dir(Some(empty_dir.path()));
        assert!(
            lookup_loaded_harness_by_id("tmp-agent").is_none(),
            "deleted harness must not appear after re-warm"
        );
    }

    // ── Legacy avatarUrl regression (F1) ─────────────────────────────────────

    /// A JSON file that contains a legacy `avatarUrl` field (from pre-BYOH code)
    /// must still deserialize without error (unknown-field handling) and the
    /// loaded `HarnessDefinition` must NOT carry the URL — the field is absent
    /// from the struct so serde drops it.
    #[test]
    fn legacy_avatar_url_in_json_is_silently_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("legacy.json"),
            r#"{
                "id": "legacy-agent",
                "label": "Legacy Agent",
                "command": "legacy-bin",
                "avatarUrl": "https://tracking.example.com/logo.png"
            }"#,
        )
        .unwrap();

        let defs = load_custom_harnesses(dir.path());
        // The file must deserialize successfully (serde ignores unknown fields).
        assert_eq!(defs.len(), 1, "legacy file with avatarUrl must still load");
        assert_eq!(defs[0].id, "legacy-agent");
        // HarnessDefinition has no avatar_url field — prove the URL cannot
        // be routed to a catalog entry by serializing back and checking.
        let json = serde_json::to_string(&defs[0]).unwrap();
        assert!(
            !json.contains("https://tracking.example.com"),
            "serialized HarnessDefinition must not contain the legacy avatar URL"
        );
    }

    // ── Preset id reservation ────────────────────────────────────────────────

    /// All preset ids must be blocked by `check_id_collision`.
    #[test]
    fn preset_ids_are_reserved_and_cannot_be_used_as_custom_ids() {
        let preset_ids = [
            "cursor", "omp", "grok", "opencode", "kimi", "amp", "hermes", "openclaw",
        ];
        for id in preset_ids {
            assert!(
                check_id_collision(id).is_err(),
                "preset id {id:?} should be rejected by check_id_collision"
            );
        }
    }
}
