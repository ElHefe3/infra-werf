# D045 — Product branding and storage identity are separate

Date: 2026-10-27<br>
State: verified<br>
Affected: post-release InfraWerf branding

## Finding

Tauri's display product name can change while the application identifier `dev.atlas.workspace` and its default native store remain stable. The NSIS template derives uninstall registration and shortcuts from the product name, so the renamed InfraWerf package receives a separate registration from Atlas.

## Evidence

The generated NSIS template uses `${PRODUCTNAME}` in `UNINSTKEY` and shortcut names. InfraWerf 0.1.1 installed into a separate program directory, with an InfraWerf shortcut pointing at its new executable. Its Windows UI displayed the existing 4-server/540-workload workspace after restart. Three database SHA-256 comparisons were unchanged. See [branding acceptance](../evidence/BRANDING-acceptance.md).

Automatic approval review rejected uninstalling the prior program because silent removal could affect app data. No uninstall occurred. The Computer Use launcher initially resolved the shared application ID to the old Atlas shortcut; launching the new executable directly and then using its actual InfraWerf shortcut opened the correct release.

## Consequence

Keep the established application identifier, data/schema and compatibility names during cosmetic renames. Use the InfraWerf shortcut or its exact installed executable for review. The two program installations share one native store and are not independent data backups.

## Follow-up

Use InfraWerf as the current desktop program. Retain the original Python database and native recovery snapshot; retirement of the old program remains a separate maintenance action if requested.
