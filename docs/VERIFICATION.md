# LEVI BROWSER Foundation Verification

Verification updated: 2026-11-15

## Environment

- Windows 11 desktop environment.
- Rust/Cargo target: Windows MSVC (same machine used for the native LEVI Command Center).
- Node `v24.21.0`; npm `11.19.0`.
- Microsoft Edge and WebView2 Runtime `157.0.4341.46`.
- Proof data is isolated under LEVI BROWSER's own Tauri app-local-data directory. It does not use or change LEVI COMMAND CENTER browser profiles.

## Results

### Passed — native Windows proof

- `npm run build` passed with Vite 8.3.8.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed; the crate currently defines **0 automated tests**, so this only verifies the Rust test harness/build.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` passed.
- `cargo build --manifest-path src-tauri/Cargo.toml` passed.
- Launched the actual Tauri desktop application in development mode using the fixed Vite URL `http://127.0.0.1:4173`; Vite strict-port mode reported that exact URL and did not silently select another port.
- The native isolation shell rendered correctly. Profile windows were created as native windows and their local probe pages rendered visibly.
- With both sessions open during the earlier live run, Alpha displayed `ALPHA-LOCAL-7C2A` in both localStorage and its test cookie; Beta displayed `BETA-LOCAL-91DF` in both. The values were distinct, demonstrating no observed cross-profile leakage for these stores on the local test origin.
- Closed and reopened Alpha within the app; its marker and cookie persisted.
- Closed the app and restarted it. Reopened Alpha and Beta separately. Alpha again displayed `ALPHA-LOCAL-7C2A` in localStorage and its cookie; Beta again displayed `BETA-LOCAL-91DF` in both. This verifies persistence across a complete app restart for both profiles.
- Screenshots were captured through the desktop UI automation session for the shell and profile inspector. The visible evidence showed the profile heading, localStorage marker, cookie value, and local URL.
- The installed WebView2 Runtime version was reported as `157.0.4341.46`.

### Resource snapshot — indicative only

Read-only process-tree working-set snapshots on this machine:

| State observed | Processes in LEVI process tree | Total working set |
| --- | ---: | ---: |
| Shell only | 7 | 318.4 MiB |
| Shell plus one profile | 13 | 613.7–662.9 MiB across separate samples |

The samples varied materially between runs, are short-lived working-set observations, and are not a controlled benchmark. A simultaneous-two-profile resource comparison was not reliably captured. These figures must not be treated as expected per-profile costs or a production capacity estimate. CPU, startup latency, background-idle behavior, and representative remote-site workloads remain unmeasured.

### Checks not available or not run

- `npm run lint` is not configured in this standalone proof-of-concept `package.json`; no lint command was run.
- There is no Git repository in the standalone proof-of-concept directory, so `git diff --check` is not applicable there. The LEVI COMMAND CENTER repository was inspected separately and remained clean.
- No release-mode packaged application was launched; native verification used the real Tauri development application. Vite production asset build and native debug build both passed.
- Automated isolation regression tests are absent. Current proof is a manual, visible native test with distinct synthetic markers.

## Evidence and retained data

The local test UDFs were retained; no cleanup was performed:

- `%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab\alpha`
- `%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab\beta`
- `%LOCALAPPDATA%\com.levi.browser.foundation\browser-shell`

No personal profiles, credentials, authentication data, Command Center records, or remote website data were used. No test UDF was deleted.

## Scope not covered by this proof

No general browsing/address bar, remote navigation, account sign-in, multi-tab controls, download manager, history/bookmarks, permissions UI, project workspaces, crash recovery, Command Center bridge, or deletion workflow is implemented. Do not describe these as working features.

The UI reports that it created a session and requested navigation; the command result alone is not proof that a document finished loading. In this proof the local pages were separately observed rendering in native windows. Production launch telemetry should distinguish creation, navigation start, page load, and load failure.
