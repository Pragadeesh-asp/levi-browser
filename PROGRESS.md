# LEVI BROWSER Progress

## Week 1 stabilization (2026-11-22)

Branch `wip/week1-stabilize` (local; not pushed). It starts from `a80f217`, a snapshot of all previously uncommitted work (profile management, LEVI theme, tab work). `artifacts/` screenshots stay untracked.

- **New profiles can receive browser events.** `capabilities/browser-chrome.json` listed only `chrome-alpha` and `chrome-beta`, so a profile created with "Add Profile" (chrome label `chrome-p-…`) had its `listen()` calls rejected, `refresh()` never ran and the tab strip stayed empty. It now matches `chrome-*`; website content webviews are labeled `content-*` and stay excluded. A Rust test checks generated labels against the capability file.
- **Log files without browsing history.** Navigation diagnostics now go through `tauri-plugin-log`. Failures log as warnings; success events, which can name a website host, log at debug level. The rotating file in `%LOCALAPPDATA%\com.levi.browser.foundation\logs` keeps info and above only, so visited hosts are never written to disk. Debug builds still print the diagnostics to the console.
- **Type-checking and lint.** Added TypeScript 7.0 (same as Command Center) with a strict `tsconfig.json`, oxlint, and `typecheck`/`lint` scripts; `npm run build` now type-checks first. The first check found an unused `navigationState` variable and `profileMenu.hidden` (typed `boolean | "until-found"`) passed as a boolean; both are fixed.
- **Repository hygiene.** `.gitattributes` (LF everywhere; no files changed) and a GitHub Actions workflow on `windows-latest`: `npm ci`, type-check + build, oxlint, `cargo fmt --check`, clippy, `cargo test`.
- **Checks passed:** `npm run build` (with `tsc`), `npm run lint`, `cargo fmt --check`, 21 Rust tests. Clippy shows the 2 existing style warnings.
- **Not verified:** creating a new profile in the native app and seeing its tab strip load (the fix is confirmed against Tauri 2.12's capability rules and by test, not by a native run), and the CI workflow on GitHub (branch not pushed). The audit's history, popup, download and session-restore items are not part of this milestone.

## Premium desktop UI and profile management update (2026-11-17)

- Updated Browser chrome tokens to Gunmetal Black `#1C2126`, Lunar White `#F4F4F0`, and Neon Lime `#C7F000`; the same values are used by Command Center. Native website WebViews remain separate and are not themed or injected.
- The former two-card Alpha/Beta landing view now reads the local saved profile catalog. Alpha and Beta keep their established profile IDs and existing WebView2 directories. New profiles use distinct `profiles/<id>` directories, created only when website content is first opened. Profile creation and metadata changes notify all open Browser chrome windows to refresh their menus.
- Added local profile creation and metadata editing, a profile switch menu in browser chrome, clear local-storage isolation copy, and a validated append-only JSONL profile catalog under the app's local-data directory. No profile deletion is exposed. No Alpha/Beta data directories were removed, moved, or reset.
- Removed the redundant visible search-engine label above the selector. Inspected pointer styles: only ordinary interactive-control cursors are used; no custom cursor asset or JavaScript cursor overlay is present.
- `npm run build` passed; `cargo test` passed (20 tests), including profile persistence and malformed persisted-ID rejection; `cargo fmt --check` and `cargo build` passed. `npm run lint` is not available in this repository. Native profile-menu interaction and new-profile creation remain pending hands-on visual verification. Existing Mission 002B.1b native multi-tab lifecycle checks remain explicitly unverified as recorded below.
- No commit or push. The app identifier, Vite port, Alpha/Beta UDF paths, and native WebView architecture are unchanged.

## Current milestone

**Mission 002B.1b — Native multi-tab implementation complete locally; awaiting manual verification and review.**

### Shared LEVI Design System V2 — Browser foundation (2026-11-15; superseded palette)

- At that milestone, `src/levi-tokens.css` used Electric Black `#080B12`, Electric Navy Blue `#1646A0`, and Electric Crimson Red `#DC2638`. The active palette was replaced on 2026-11-16 by the Gunmetal/Lunar/Lime palette documented above.
- Applied the shared system to Browser chrome, active/inactive tabs, address controls, launcher, empty state and local profile inspection page. Crimson marks the active tab and primary launcher action; the lighter blue accent is used for navigation/focus; Electric Navy anchors surfaces.
- Added restrained layered card shadows, hover/press feedback, explicit keyboard focus and reduced-motion handling. External website WebViews are not styled or injected.
- Frontend production build passed. Rust tests (16), Rust formatting check and `git diff --check` passed in this session. Native visual inspection of the updated palette is still pending; the desktop automation input-geometry failure prevented controlled profile interaction. Mission 002B.1b multi-tab native verification remains incomplete, as listed below.
- No commit or push. Command Center has equivalent tokens and theme overrides in its own repository; the two repositories remain independent.

Phase 1 review update (2026-11-16): added shared semantic aliases for backgrounds, surfaces, text, borders, focus, buttons, status, shadows and interaction timing while retaining the existing token names. The tab strip now supports horizontal overflow and scrolls the active tab into view. `npm run build`, 16 Rust tests, `cargo fmt --check`, `cargo build`, and `git diff --check` passed. Existing Browser launcher screenshot artifacts were inspected. The two-profile event-target fix, full native multi-tab lifecycle matrix, resize behavior and post-restart tab behavior remain unverified; see the checklist below. No native website/profile verification was repeated in this review.

### Mission 002B.1b — Native multi-tab implementation

- Connected the Rust `BrowserRegistry` to genuine per-tab native Tauri/Wry website WebViews. Each tab has a collision-checked stable ID/label, generation-aware creation lifecycle, independent URL/history/loading/error/title state, and its own profile-scoped WebView surface.
- Alpha tabs reuse the existing Alpha WebView2 user-data directory; Beta tabs reuse the existing Beta directory. No profile directories were cleared or changed. Website WebViews remain outside browser-chrome capabilities, and uncontrolled popup/new-window requests are denied.
- Added a horizontal tab strip with new/select/close controls, active-tab styling, page-title labels, loading state, and a fallback glyph. Selecting and closing tabs hides/shows native WebViews; closing the final tab leaves the profile window open with an empty new tab.
- Per-tab Back, Forward, Reload, Stop and address navigation target stable tab IDs. Native callbacks carry tab generation and stale callbacks are ignored. Browser operations are serialized; registry locks are not held over native WebView operations. One resize listener is registered per profile window.
- Navigation and tab-state events include the owning profile ID. During a two-profile native session, cross-profile events reached frontend listeners and were ignored by the profile-ID guard. The cause was confirmed in the locked Tauri 2.12.5 and `@tauri-apps/api` event code: frontend `listen()` defaults its target to `Any`, while `emit_to(EventTarget::webview(chrome_label), ...)` targets the correct chrome. Both browser listeners now specify their own `Webview` target label (`chrome-alpha` or `chrome-beta`); the profile-ID guard remains as defense in depth. This API-level cause and source change are verified by code inspection, but the isolation fix still needs native two-profile retesting.
- Automated tests: 16 Rust tests passed, including generated ID/label collision checks, per-tab navigation, profile ownership/isolation, active/inactive/final closure, generation invalidation, creation retry/stale completion, and existing URL validation.
- `npm run build`, `cargo test` (16 tests), `cargo fmt --check`, `cargo build`, and `git diff --check` passed. Native desktop launch was visually observed: Alpha chrome rendered, a real Example Domain page rendered in a second native tab, a new empty tab hid the previous content, switching back restored the website, and closing an inactive tab preserved the active page. Existing local storage probe pages rendered in native Alpha and Beta tabs: Alpha showed `ALPHA-LOCAL-7C2A`, Beta showed `BETA-LOCAL-91DF`, and a second Alpha tab showed Alpha's marker. Cross-profile event mismatch was reproduced in the frontend console and ignored after the profile-ID guard was added. These are direct manual observations, not automated integration tests.
- Earlier manual observations covered native Alpha rendering, Alpha tab switching and inactive-tab closure, and local storage markers in Alpha and Beta. New final-verification checks could not be performed: the Windows desktop-control service returned `coordinate input geometry is unavailable` when attempting to activate Beta from the launcher. The test launcher was closed normally. Not yet verified: Beta's required three-tab switching/closure sequence, rapid create/close stress in both profiles, independent Back/Forward/Reload/Stop across tabs, profile close/reopen with multiple tabs, post-fix native cross-profile event isolation, and 1/3/5/10-tab resource measurements. No automated native WebView integration suite exists.
- Changed files for this milestone: `src-tauri/src/main.rs`, `src-tauri/src/tabs.rs`, `src/browser.ts`, `src/browser.css`, and this document. Work is uncommitted and unpushed.
- Next step: user-assisted manual verification using the checklist in the completion report. Do not start Mission 002B.2 until review and approval.

### Manual verification checklist — Mission 002B.1b

1. Open Alpha and create five tabs with `+` or Ctrl+T; navigate each to a distinct harmless page.
2. Switch through every tab and confirm only the selected page is visible and each tab restores its own title, URL and page state.
3. On at least two tabs, navigate between two URLs and exercise Back, Forward, Reload and Stop independently.
4. Close an inactive tab, then close the active tab; confirm a neighboring tab is selected. Close the final tab and confirm the profile window remains open on a blank new tab.
5. Resize, minimize and restore the profile window; confirm the content viewport remains below the chrome.
6. Repeat in Beta. Use a harmless local storage marker page if available to confirm same-profile tabs share state while Alpha and Beta remain isolated. Do not use real account credentials.
7. If practical, record process memory at 1, 3, 5 and 10 live tabs and note switching responsiveness. Avoid opening 10 tabs if system pressure becomes noticeable.
8. Close and reopen each profile, verify its native window and profile directory remain intact, and report any WebView2 errors. Tabs themselves are currently process-local and are not restored after restart.

### Mission 002B.1a — Registry model

- Added a Rust `BrowserRegistry` with profile lifecycle, ordered stable tab IDs, active-tab selection, per-tab WebView labels, metadata, and independent navigation state.
- Added model operations for initial profile tabs, adding/selecting/closing tabs, neighboring-tab selection, creation lifecycle, profile close/removal, ownership validation, and stale-tab callback rejection.
- Wired the existing Alpha/Beta navigation store to each profile's initial registry tab. Existing native windows and single website WebViews remain unchanged; additional native tabs and UI are not implemented in this milestone.
- Alpha and Beta retain their existing WebView2 data-directory mapping. The registry is process-local and does not persist tabs across application restarts.
- Automated Rust tests: 12 passed. `npm run build`, `cargo fmt --check`, `cargo build`, and `git diff --check` passed. Native UI verification for additional tabs is not applicable because native multi-tab creation is explicitly out of scope.
- Changed files: `src-tauri/src/main.rs`, `src-tauri/src/tabs.rs`, and this progress record.
- Next task, only after approval: Mission 002B.1b — native child WebView lifecycle and tab UI integration.

## Architecture

Tauri 2 + Wry + WebView2 Runtime. Each profile owns one native browser window/chrome WebView and a set of native website child WebViews. Every tab in a profile uses that profile's existing persistent WebView2 User Data Folder; Alpha and Beta use different folders. Website content is a separate child WebView without app capabilities. See [architecture](docs/ARCHITECTURE.md).

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
- Multi-tab UI and native surfaces are implemented locally; full manual stress verification is pending. Tabs are process-local and are not restored after an application restart. Native favicon extraction and structured WebView2 network failure reasons are not available in the current integration, so the tab strip uses a fallback glyph and page failures rely on WebView2's error document.
- Downloads, permission prompts, bookmarks, persistent browser history, crash recovery and Command Center integration remain out of scope.
- `npm run lint` is not configured. Resource measurements remain indicative rather than controlled. Automated regression coverage for Alpha/Beta storage isolation remains absent. See [verification](docs/VERIFICATION.md).

## Recovery and source control

- Previous Git baseline: `6612b0175cc01fe2b80d6cef162ad967cee642ce` (`chore: establish LEVI Browser baseline`).
- GitHub owner: `Pragadeesh-asp`.
- GitHub repository: [https://github.com/Pragadeesh-asp/levi-browser](https://github.com/Pragadeesh-asp/levi-browser); previously verified private, owned by `Pragadeesh-asp`, default branch `main`.
- Remote: `origin` → `https://github.com/Pragadeesh-asp/levi-browser.git`.
- Branch: `main` tracks `origin/main`.
- Mission 002A was committed and pushed as `a19432fda30344f3da7a62400ddf36a8bddc17bc`. Mission 002B.1a was committed and pushed as `5c6d0cffb3855f92954b86a513527b3a311dc3d0`. Mission 002B.1b is currently uncommitted and unpushed.
- Retained Alpha/Beta test UDFs are outside this repository under `%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab\alpha` and `...\beta`; they were not moved, deleted or reset. The app-local `EBWebView` directory remains untouched.
- Next task: complete the user-assisted Mission 002B.1b manual matrix and review this uncommitted milestone. Do not start Mission 002B.2 without approval.

## Checkpoint rules

Follow [DEVELOPMENT_RULES.md](DEVELOPMENT_RULES.md). Stage and review a milestone's exact files, run relevant verification, update this document, commit the verified milestone, and push only after it is complete and verified. Keep secrets, local databases, generated output, and all WebView2 profile/session data out of Git.

## LEVI visual foundation update (2026-11-16)

- Applied the shared Electric Black (`#080B12`), Electric Navy Blue (`#1646A0`) and Electric Crimson Red (`#DC2638`) semantic design tokens to browser chrome and added a shared typography scale.
- Increased browser chrome readability for the address bar, tab labels and controls. These styles do not affect website content rendered in native WebViews.
- Existing native multi-tab and Alpha/Beta storage architecture was preserved. No browser assistant or page-content collection was added; adding a panel over native website WebViews needs a separate bounds/lifecycle design to avoid covering or intercepting page content.
- `npm run build` passes. Native visual interaction, layout at normal Windows scaling, motion/focus behavior and the outstanding Mission 002B.1b lifecycle/stress checks remain unverified in this update. No commit or push was made.
