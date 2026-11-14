# LEVI BROWSER — Architecture decision and foundation

Status: Windows 11 proof of concept. This is an independent product workspace and does not alter LEVI COMMAND CENTER or its browser registrations/database.

## Decision

Use Tauri 2 + Wry + the installed Evergreen WebView2 Runtime, with one explicit persistent User Data Folder (UDF) per LEVI profile. Each profile tab/window is a distinct Tauri WebView2 instance whose `data_directory` points at that profile's app-owned directory:

```text
%LOCALAPPDATA%\com.levi.browser.foundation\profile-isolation-lab\<profile-id>
```

Tabs in the same profile share that directory and therefore the session. Different profile IDs use distinct directories and separate WebView2 environment/process collections. LEVI never reads, copies, or edits another browser's profile. Production profile operations must use generated opaque IDs, validate paths remain below LEVI's root, and refuse symlink/junction escapes.

## Alternatives

| Option | Isolation and lifecycle | Cost / fit |
| --- | --- | --- |
| A. Tauri WebView2, separate UDF per LEVI profile | Tauri/Wry exposes `data_directory` per WebView. WebViews given different UDFs create independent WebView2 environment/process groups. Session state persists in each UDF. Multiple tabs for one profile intentionally use its same UDF. | Best MVP fit; simple public Tauri API, genuine cookie/local-storage separation, higher memory/disk/process cost as profiles grow. This is the selected proof architecture. |
| B. Multiple WebView2 controllers using named profiles under one UDF | WebView2's multi-profile API provides profile-specific data under a shared UDF. Same-profile controls share state; separate named profiles separate cookies and other data. | Potentially more efficient. Current Tauri/Wry API accepts a UDF but does not expose `CoreWebView2ControllerOptions.ProfileName`; using it requires a maintained native plugin/COM layer and version negotiation. Defer until measured profile counts justify it. |
| C. Dedicated Chromium framework (for example a Chromium embedding framework) | Broad browser-engine and lifecycle control, depending on framework. | Larger runtime/deployment footprint and security/update burden. No present requirement that justifies replacing WebView2. Reassess only if WebView2 cannot satisfy verified requirements. |

## Verified platform facts

- WebView2 supports a separate UDF per environment and multiple profiles under one UDF. Separate UDFs create separate process collections; WebView2 controls sharing the same UDF can share a browser process. [WebView2 multiple profiles](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/multi-profile-support), [process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)
- WebView2 profile data includes cookies, preferences and cached resources; each named profile gets a dedicated profile folder. WebView2 documents separate UDFs as an older isolation approach with greater memory, CPU and disk use than named profiles. [Multi-profile support](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/multi-profile-support)
- Resource use grows with active WebView2 instances/processes and page complexity. It must be measured on representative LEVI pages rather than inferred from an “embedded” label. [WebView2 performance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
- Tauri 2's `WebviewWindowBuilder` and Wry expose a per-webview `data_directory`; Tauri also exposes navigation, download, permission-request, title-change and page-load hooks. The current public API does not expose the WebView2 named `ProfileName` controller option. [Tauri WebviewWindowBuilder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)
- Runtime profile isolation does not by itself implement a complete browser. Browser history, bookmarks, permission UI/policy, downloads UI, tab lifecycle, crash recovery and workspace mapping remain LEVI responsibilities. Tauri/Wry can surface navigation/download/permission events, but the prototype has not implemented those product flows.
- Microsoft lists Windows 11 as supported, recommends Evergreen for most apps, and describes the WebView2 Runtime as preinstalled on Windows 11. The test machine reports Edge and WebView2 Runtime `157.0.4341.46`. [WebView2 introduction](https://learn.microsoft.com/en-us/microsoft-edge/webview2), [runtime release notes](https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/runtime)

## Isolation proof

The foundation app creates two native Tauri windows. `Profile Alpha` and `Profile Beta` point to different UDFs and open a bundled local-only test page. The page stores a unique marker in `localStorage` and a profile-specific cookie, then displays both values. Closing a test window preserves its UDF; reopening it must show the same values. The lab has no real account login, external URL navigation, credential access, or profile deletion function.

The Windows proof passed for this prototype: distinct localStorage and cookie markers were visible in separate Alpha/Beta windows, and both values persisted after closing each profile and restarting the app. This validates the narrow storage-isolation claim for the current local test pages; it does not make the app a production profile manager.

Profile-window creation is dispatched onto Tauri's Windows main thread (`AppHandle::run_on_main_thread`). Calling the builder directly from an async command worker previously produced a blank native window. The command reports the requested URL and window creation; it does not itself prove navigation completion. The local document was separately verified in the native UI.

## Product implications and limitations

- Each simultaneous profile UDF starts/uses its own WebView2 process collection. Memory and CPU will scale with active sites and profiles; the MVP should lazily open sessions and close idle WebViews without deleting data.
- A browser “tab” will map to a WebView in an application-managed tab record. Tabs for one profile share its UDF; they must not navigate into a different profile's UDF as fallback.
- Separate UDFs are not filesystem security boundaries against the same Windows user or other processes running with that user's access. LEVI's isolation means browser storage/session separation within the app, not a separate Windows identity or immunity from website/provider restrictions.
- Same-profile windows deliberately share cookies/storage and may contend on lifecycle/shutdown. Browser process crash and active-download close behavior need explicit UX before release.
- Browser permissions require an explicit policy surface and tests; do not silently approve camera, microphone, geolocation, notifications, or other sensitive capabilities. Downloads need a safe destination/confirmation policy and record; never auto-open downloaded files.
- External webpages must receive no Tauri command capability. The shell is the only capability-enabled webview. Add strict navigation allow/deny rules for internal URLs and avoid arbitrary host-object exposure.
- History, bookmarks, downloads and workspaces are LEVI-owned databases/features, not included in this proof. No LEVI COMMAND CENTER bridge is included.

## Next phases

1. Harden profile identity/root validation and define crash/close handling.
2. Add address bar/navigation/back/forward/reload and supervised new-window handling.
3. Add true profile create/rename/open/close/archive management while preserving each UDF.
4. Add tabs/windows, download handling, history/bookmarks and permission UI.
5. Evaluate WebView2 named profiles through a small maintained native layer only if resource measurements justify its complexity.
6. Design a local authenticated/authorized Command Center bridge as a separate reviewed milestone.
