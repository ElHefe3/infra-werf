# D058 — Plugin install: HTTPS, minisign and two archive formats

Date: 2026-11-17. Related: S12 / F22 T045; [D052](D052-plugin-platform-direction.md), [D055](D055-plugin-choices-belong-to-the-owner-in-settings.md).

## Owner decisions

T045 installs plugins from a folder, an archive file or an HTTPS address, with an optional author signature. Three choices were the owner's; asked on 2026-11-17, the owner chose:

1. **HTTPS built in.** InfraWerf gets its first HTTPS client (`ureq` with rustls and Mozilla's root certificates). Only `https://` is accepted, a redirect to `http://` is refused, and only the exact address the owner typed (plus the same address with `.minisig` when the owner gives a public key) is fetched, once, when the owner presses Install. Downloads are limited to 5 MiB and 30 seconds. This is the first time InfraWerf itself contacts the internet, and it happens only then.
2. **minisign signatures.** An author signs an archive with the common minisign tool. InfraWerf trusts no key by itself: the owner pastes the author's public key when they want the signature checked, and then a missing or wrong signature refuses the install. The verified key ID is shown on the plugin's page, so T046 can require the same key for updates.
3. **Zip and .tar.gz.** Both formats are accepted, recognised by content rather than name, with the same path, link and size checks.

## Consequences

- New dependencies in `atlas-core`, pinned exactly: `zip` (deflate only), `tar` (no extended attributes), `flate2`, `minisign-verify`, `ureq` (rustls).
- `ATLAS_PLUGIN_TEST_CA` replaces the trusted roots with one test certificate, for the native suite's loopback HTTPS server only, like the other `ATLAS_*` test overrides.
- Signatures were tested with files produced by a Python implementation of minisign's format (`scripts/plugin_archive_fixtures.py`), not by the minisign tool itself.
