# LEVI BROWSER Progress

## Current milestone

**Mission 002B.1a — Tab registry and per-tab state model implemented; awaiting review.**

### Mission 002B.1a — Registry model

- Added a Rust `BrowserRegistry` with profile lifecycle, ordered stable tab IDs, active-tab selection, per-tab WebView labels, metadata, and independent navigation state.
- Added model operations for initial profile tabs, adding/selecting/closing tabs, neighboring-tab selection, creation lifecycle, profile close/removal, ownership validation, and stale-tab callback rejection.
- Wired the existing Alpha/Beta navigation store to each profile's initial registry tab. Existing native windows and single website WebViews remain unchanged; additional native tabs and UI are not implemented in this milestone.
- Alpha and Beta retain their existing WebView2 data-directory mapping. The registry is process-local and does not persist tabs across application restarts.
- Automated Rust tests: 12 passed. `npm run build`, `cargo fmt --check`, `cargo build`, and `git diff --check` passed. Native UI verification for additional tabs is not applicable because native multi-tab creation is explicitly out of scope.
- Changed files: `src-tauri/src/main.rs`, `src-tauri/src/tabs.rs`, and this progress record.
- Next task, only after approval: Mission 002B.1b — native child WebView lifecycle and tab UI integration.

## Architecture

Tauri 2 + Wry + WebView2 Runtime. Each Alpha/Beta website webview uses its existing separate persistent WebView2 User Data Folder under the LEVI app's local data directory. Browser chrome is a local Tauri child webview; website content is a separate child webview without app capabilities. See [architecture](docs/ARCHITECTURE.md).

## Verified results

- Mission 001 Windows proof: Alpha and Beta rendered in native WebView2 windows with separate `localStorage` and cookie markers (`ALPHA-LOCAL-7C2A` and `BETA-LOCAL-91DF`). No cross-profile leakage was observed on the local probe origin, and values persisted across close/reopen and app restart.
- Mission 002A frontend production build passed, including separate Alpha and Beta browser chrome pages.
- Mission 002A Rust tests pass for HTTP/HTTPS, scheme-less domains, configured HTTPS search URLs, and unsupported/malformed scheme rejection.
- Native screenshots confirm the launcher and Alpha browser chrome render in actual Tauri/WebView2 windows.
- The first native URL submission exposed a blocking child-webview creation path; the navigation command was changed to async.
- User-reported native verification: Alpha visibly rendered `https://example.com` and Wikipedia; address synchronization, Back, Forward, Reload and Home worked. These are manual results, not automated tests.
- User-reported profile verification: Alpha and Beta retained their distinct localStorage and cookie markers after application restart. These are manual results, not automated tests.
- User-reported Mission 002A.3 lifecycle verification: Alpha and Beta each closed and reopened within the same running application, began with fresh navigation state, and left the other profile unaffected. These are manual results, not automated tests.
- Final automated verification: `npm run build`, `cargo test` (5 passed), `cargo fmt --check`, `cargo build`, and `git diff --check` passed.
- The retained Mission 002A.2 Rust log records Alpha child-WebView creation/show, navigation start, page-load completion and state-event emission. It does not capture frontend DevTools event receipt or Beta navigation.
- Mission 002A.3 resets process-local navigation state when a profile window is destroyed, guards the closing interval, and returns empty state when no content WebView exists. Automated store lifecycle tests pass; native behavior remains user-reported rather than automatically tested.
- Vite development server remains pinned to `127.0.0.1:4173` with strict port selection.

### Mission 002A.2 — Manual verification diagnostics

- Added redacted `[LEVI_DIAG]` Rust events for navigation invocation, URL validation, child-WebView creation/reuse/show, parent-window visibility, navigation start, page-load start/finish, and address-state event emission/failure. URL diagnostics include scheme and host only; paths, queries, page contents and credentials are not logged.
- Browser chrome logs when the state listener is ready, when an address-state event arrives, and when the navigation invoke resolves or rejects. Initial state is queried only after listener registration completes.
- The diagnostic log contains Rust stderr events; frontend listener-receipt messages are available only in the WebView DevTools console. A visible address-bar update remains the user-facing confirmation.
- Native behavior and Alpha/Beta storage isolation have user-reported manual verification; there is no automated Tauri/WebView integration suite. Keep this distinction when reporting results.

## Mission 002A implementation

- Editable address field with Enter submission and Ctrl+L focus shortcut.
- HTTP and HTTPS support, automatic HTTPS for unschemed domains, and search queries sent through a configurable DuckDuckGo, Google or Bing search URL.
- Back, Forward, Reload, Stop and Home controls, with disabled states based on tracked top-level navigation.
- Native WebView2 website content in a separate child webview, attached to the existing Alpha or Beta UDF. Empty state and local chrome remain separate from website content.
- Loading feedback, Rust validation errors, and WebView2's built-in network error document.
- Rust caller-label checks restrict browser control commands to the local shell or matching local browser chrome.

## Limitations and pending verification

- No automated test verifies actual WebView pixel rendering or Windows window lifecycle; those results are user-confirmed manual checks.
- WebView2's built-in network error page is shown for load failures; Tauri's current page-load hook does not expose a structured network error reason.
- Back/forward tracking follows top-level navigation and does not observe same-document SPA `history.pushState` changes.
- New-window links, tabs, downloads, permissions UI, bookmarks, persistent browser history, crash recovery and Command Center integration are out of scope.
- `npm run lint` is not configured. Resource measurements remain indicative rather than controlled. Automated regression coverage for Alpha/Beta storage isolation remains absent. See [verification](docs/VERIFICATION.md).

## Recovery and source control

- Previous Git baseline: `6612b0175cc01fe2b80d6cef162ad967cee642ce` (`chore: establish LEVI Browser baseline`).
- GitHub owner: `Pragadeesh-asp`.
- GitHub repository: [https://github.com/Pragadeesh-asp/levi-browser](https://github.com/Pragadeesh-asp/levi-browser); previously verified private, owned by `Pragadeesh-asp`, default branch `main`.
- Remote: `origin` → `https://github.com/Pragadeesh-asp/levi-browser.git`.
- Branch: `main` tracks `origin/main`.
- Mission 002A was committed and pushed as `a19432fda30344f3da7a62400ddf36a8bddc17bc`; this Mission 002B.1a work is currently uncommitted and unpushed.
- Retained Alpha/Beta test UDFs are outside this repository under `%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab\alpha` and `...\beta`; they were not moved, deleted or reset. The app-local `EBWebView` directory remains untouched.
- Next task: obtain approval for the reviewed Mission 002A checkpoint. Do not start Mission 002B.

## Checkpoint rules

Follow [DEVELOPMENT_RULES.md](DEVELOPMENT_RULES.md). Stage and review a milestone's exact files, run relevant verification, update this document, commit the verified milestone, and push only after it is complete and verified. Keep secrets, local databases, generated output, and all WebView2 profile/session data out of Git.
