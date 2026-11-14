import { invoke } from '@tauri-apps/api/core'
import './style.css'

type ProfileLaunch = {
  profileId: string
  windowLabel: string
  dataDirectory: string
  requestedUrl: string
  reusedWindow: boolean
}

const app = document.querySelector<HTMLDivElement>('#app')!
const status = document.createElement('div')
status.className = 'status-line'

app.innerHTML = `
  <div class="shell">
    <aside class="rail">
      <div class="brand-mark">L<span>•</span></div>
      <div class="rail-label">LAB</div>
      <div class="rail-spacer"></div>
      <div class="runtime-dot" title="Windows WebView2"></div>
    </aside>
    <main class="workspace">
      <header class="topbar">
        <div class="brand"><span class="eyebrow">LEVI ECOSYSTEM / BROWSER FOUNDATION</span><h1>LEVI <b>BROWSER</b></h1></div>
        <div class="build-pill"><i></i> WINDOWS WEBVIEW2 · ISOLATION LAB</div>
      </header>
      <section class="hero">
        <span class="eyebrow">MISSION 001 / STORAGE BOUNDARY PROOF</span>
        <h2>One Browser.<br/><em>Multiple Worlds.</em></h2>
        <p>Two independent WebView2 sessions, backed by separate persistent User Data Folders. Use the isolated local probe pages to inspect each session’s storage boundary.</p>
      </section>
      <section class="profiles" aria-label="Isolated test profiles">
        <article class="profile-card alpha">
          <div class="profile-orbit">A</div>
          <div class="profile-copy"><span class="eyebrow">ISOLATION SPACE 01</span><h3>Profile Alpha</h3><p>Its own WebView2 data directory, local storage and cookie jar.</p><code id="alpha-path">Loading storage path…</code></div>
          <div class="profile-actions"><button class="primary" data-open="alpha">Open isolated session <span>↗</span></button><button class="subtle" data-close="alpha">Close session</button></div>
        </article>
        <article class="profile-card beta">
          <div class="profile-orbit">B</div>
          <div class="profile-copy"><span class="eyebrow">ISOLATION SPACE 02</span><h3>Profile Beta</h3><p>Separate from Alpha, including when both sessions are open together.</p><code id="beta-path">Loading storage path…</code></div>
          <div class="profile-actions"><button class="primary" data-open="beta">Open isolated session <span>↗</span></button><button class="subtle" data-close="beta">Close session</button></div>
        </article>
      </section>
      <section class="proof-note"><div class="shield">◇</div><div><strong>Local-only isolation test</strong><span>Each test window stores a different marker in localStorage and a test cookie. No network navigation, personal profile, or real login is used. Closing a session preserves its test data.</span></div><span class="proof-status">PERSISTENT UDF</span></section>
      <footer><span>LEVI BROWSER FOUNDATION · 0.1.0</span><span>Profiles remain on this device</span></footer>
    </main>
  </div>
`

const statusLine = document.createElement('div')
statusLine.className = 'toast'
statusLine.setAttribute('role', 'status')
app.append(statusLine)

function announce(message: string) {
  statusLine.textContent = message
  statusLine.classList.add('visible')
}

async function loadPaths() {
  for (const profileId of ['alpha', 'beta']) {
    const path = await invoke<string>('profile_directory', { profileId })
    document.querySelector<HTMLElement>(`#${profileId}-path`)!.textContent = path
  }
}

app.querySelectorAll<HTMLButtonElement>('[data-open]').forEach((button) => {
  button.addEventListener('click', async () => {
    const profileId = button.dataset.open!
    button.disabled = true
    try {
      const result = await invoke<ProfileLaunch>('open_profile', { profileId })
      announce(result.reusedWindow
        ? `Existing ${profileId.toUpperCase()} session focused · ${result.requestedUrl}`
        : `${profileId.toUpperCase()} session created; navigation requested to ${result.requestedUrl}`)
    } catch (error) {
      announce(`Could not open ${profileId.toUpperCase()}: ${String(error)}`)
    } finally {
      button.disabled = false
    }
  })
})

app.querySelectorAll<HTMLButtonElement>('[data-close]').forEach((button) => {
  button.addEventListener('click', async () => {
    const profileId = button.dataset.close!
    try {
      await invoke('close_profile', { profileId })
      announce(`${profileId.toUpperCase()} test window closed. Its isolated data is preserved.`)
    } catch (error) {
      announce(`Could not close ${profileId.toUpperCase()}: ${String(error)}`)
    }
  })
})

void loadPaths().catch((error) => announce(`Could not resolve profile storage paths: ${String(error)}`))
