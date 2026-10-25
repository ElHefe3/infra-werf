# Shared contract boundary

Version 1 is defined in `crates/atlas-core/src/contracts.rs`. `ui/contracts.gen.ts` is generated from Rust with ts-rs 12.0.1; do not edit it by hand. `mise run contracts:generate` intentionally writes bindings; `contracts:check` compares without rewriting. Both native and coordinator adapters import the same definitions and reject an incompatible contract version.

IDs remain opaque stable strings. Missing live metrics and observation/check times remain null. Saved inventory freshness is computed in Rust with the existing 15-minute threshold; saved workload state does not imply current service health or host reachability. Input text/hostname/coordinate validation is shared Rust domain code, not duplicated in client adapters. Domain operations must validate their input DTOs; possessing a DTO or handle grants no authority.

ts-rs reports unsupported Serde `transparent` and custom `deserialize_with` attributes. The reviewed generated bindings use string IDs and optional, non-null patch fields; focused Rust serialization/input tests and TypeScript checks cover those representations. These warnings are not globally suppressed and do not implement the Rust validators in TypeScript. Baseline retention is classified in [INVENTORY.md](INVENTORY.md).

`BaselineInventory` is an explicit persistable allowlist. `ProtectedText` is an ephemeral authenticated-result envelope and is not part of that allowlist. Local private keys, passwords and authentication credentials have no serializable contract fields. Authentication/session enforcement and clearing protected results remain F05/F06; this feature does not implement them.

Structured errors have distinct input, unauthenticated, denied, trust, expiry, I/O, missing/conflict and version/migration codes. Public error constructors emit safe fixed messages; do not forward raw subprocess/database output through them. Operation byte counts are bounded to JavaScript's exact integer range and unknown totals stay null.

The coordinator adapter currently accepts an injected authenticated transport. It starts no listener and supplies no authentication bypass. F05 will supply that transport. Native status uses the actual Tauri command and the same generated status type on Windows and Android's shared frontend.
