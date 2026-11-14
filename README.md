# LEVI BROWSER — Foundation

Standalone Tauri 2 + WebView2 profile-isolation proof. This workspace is separate from LEVI COMMAND CENTER.

## Run on Windows

Requirements: Node/npm, Rust stable MSVC toolchain, Visual Studio C++ Build Tools, and WebView2 Runtime.

```powershell
npm install
npm run tauri -- dev --config src-tauri/tauri.dev.conf.json
```

The app opens a local test shell. Open Profile Alpha and Profile Beta separately. Each native window uses a different persistent UDF and shows profile-specific `localStorage` and cookie values. Close a profile window from the shell and reopen it to confirm its data persists. To verify restart persistence, close both profile windows, close LEVI BROWSER, relaunch it, then reopen both.

The test UDFs are stored beneath `%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab`. The application has no cleanup/delete control. Remove test data only after closing the app and after reviewing exact contents; no personal browser profile is used.

This is an isolation proof, not a functional browser. See [architecture](docs/ARCHITECTURE.md) and [verification status](docs/VERIFICATION.md).
