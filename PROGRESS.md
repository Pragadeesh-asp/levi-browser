# LEVI BROWSER Progress

## Current milestone

**Mission 001 — Isolation POC completed.** The native Windows proof verified separate Alpha/Beta localStorage and cookie values and persistence after profile close/reopen and app restart.

## Architecture

Tauri 2 + Wry + WebView2 Runtime. Each test profile uses a separate persistent WebView2 User Data Folder under the LEVI app's local data directory. See [architecture](docs/ARCHITECTURE.md).

## Verified results

- Alpha and Beta rendered in native Windows WebView2 windows.
- Alpha's synthetic marker was `ALPHA-LOCAL-7C2A`; Beta's was `BETA-LOCAL-91DF`.
- Each marker appeared in its profile's localStorage and test cookie; no cross-profile leakage was observed in the local test origin.
- Both profiles retained their values after close/reopen and a complete app restart.
- Frontend production build, Rust build, Rust test harness, and Rust formatting check passed. The Rust crate has no automated tests yet.
- Vite's development server is pinned to `127.0.0.1:4173` with strict port selection.
- WebView2 Runtime on the verification machine: `157.0.4341.46`.

## Limitations

This is an isolation proof, not a general-purpose browser. It has no address bar, general navigation, tab management, downloads UI, history/bookmarks, permissions UI, crash recovery, or Command Center integration. Resource measurements were indicative snapshots only and need controlled benchmarking. `npm run lint` is not configured; automated tests for profile isolation are not yet present. See [verification](docs/VERIFICATION.md).

## Recovery and source control

- Git baseline: `6612b0175cc01fe2b80d6cef162ad967cee642ce` (`chore: establish LEVI Browser baseline`).
- GitHub remote: not configured. The requested owner/repository is `levi28matthew/levi-browser`; the current `gh` login is `sunshine211104`, and it could not view the requested repository. Authenticate as `levi28matthew` (or grant the active account access) before creating/verifying the private remote.
- Branch: `main`.
- Next milestone: **Mission 002A — Address Bar and URL Navigation**. Do not begin it until the current milestone is reviewed and approved.
- Retained test UDFs are outside this repository under `%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab\alpha` and `...\beta`. Do not remove them as part of Git setup.

## Checkpoint rules

Follow [DEVELOPMENT_RULES.md](DEVELOPMENT_RULES.md). Stage and review a milestone's exact files, run relevant verification, update this document, commit the verified milestone, and push only after it is complete and verified. Keep secrets, local databases, generated output, and all WebView2 profile/session data out of Git.
