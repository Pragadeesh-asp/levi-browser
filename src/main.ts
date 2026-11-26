import { invoke } from '@tauri-apps/api/core'
import { createProfileDialog, type BrowserProfile } from './profile-management'
import './style.css'
import './levi-theme.css'
import './levi-carbon.css'

type ProfileLaunch = { profileId: string; windowLabel: string; dataDirectory: string; reusedWindow: boolean }
const app = document.querySelector<HTMLDivElement>('#app')!
app.innerHTML = `<div class="shell"><aside class="rail"><div class="brand-mark">L<span>•</span></div><div class="rail-label">BROWSER</div><div class="rail-spacer"></div><div class="runtime-dot" title="Windows WebView2"></div></aside><main class="workspace"><header class="topbar"><div class="brand"><span class="eyebrow">LEVI ECOSYSTEM / PRIVATE BROWSING SPACES</span><h1>LEVI <b>BROWSER</b></h1></div><div class="build-pill"><i></i> WINDOWS WEBVIEW2 · NATIVE</div></header><section class="hero"><span class="eyebrow">PRIVATE BROWSING SPACES</span><h2>One browser.<br/><em>Your profiles.</em></h2><p>Each profile has its own persistent WebView2 storage and opens in a separate native window.</p></section><div class="profile-heading"><div><h2>Your profiles</h2><p id="profile-count">Loading saved profiles…</p></div><button class="primary add-profile">＋ Add profile</button></div><section class="profiles" id="profile-grid" aria-label="Saved browser profiles"></section><section class="proof-note"><div class="shield">◇</div><div><strong>Separate profile storage</strong><span>Profiles use independent persistent WebView2 user-data folders. Existing Alpha and Beta folders remain in place.</span></div><span class="proof-status">LOCAL DATA</span></section><footer><span>LEVI BROWSER</span><span>Profile settings stored locally</span></footer></main></div>`

const status = document.createElement('div')
status.className = 'toast'
status.setAttribute('role', 'status')
app.append(status)
function announce(message: string) { status.textContent = message; status.classList.add('visible'); window.setTimeout(() => status.classList.remove('visible'), 4500) }

async function openProfile(profileId: string) {
  try {
    const result = await invoke<ProfileLaunch>('open_profile', { profileId, currentProfileId: null })
    announce(result.reusedWindow ? `${profileId} window focused.` : `${profileId} opened in a separate window.`)
  } catch (cause) { announce(`Could not open profile: ${String(cause)}`) }
}

const dialog = createProfileDialog(null, id => void openProfile(id))
app.querySelector('.add-profile')!.addEventListener('click', () => dialog.open())

async function renderProfiles() {
  const grid = app.querySelector<HTMLElement>('#profile-grid')!
  const count = app.querySelector<HTMLElement>('#profile-count')!
  grid.replaceChildren()
  try {
    const profiles = await invoke<BrowserProfile[]>('list_profiles', { currentProfileId: null })
    count.textContent = `${profiles.length} ${profiles.length === 1 ? 'profile' : 'profiles'} saved on this device`
    for (const profile of profiles) {
      const path = await invoke<string>('profile_directory', { profileId: profile.id })
      const card = document.createElement('article')
      card.className = 'profile-card'
      card.style.setProperty('--profile-accent', profile.accent)
      const avatar = document.createElement('div'); avatar.className = 'profile-orbit'; avatar.textContent = profile.avatar
      const copy = document.createElement('div'); copy.className = 'profile-copy'
      const title = document.createElement('h3'); title.textContent = profile.name
      const detail = document.createElement('p'); detail.textContent = 'Separate native window · Independent persistent storage'
      const directory = document.createElement('code'); directory.textContent = path; directory.title = path
      copy.append(title, detail, directory)
      const actions = document.createElement('div'); actions.className = 'profile-actions'
      const open = document.createElement('button'); open.className = 'primary'; open.textContent = 'Open profile ↗'; open.addEventListener('click', () => void openProfile(profile.id))
      const manage = document.createElement('button'); manage.className = 'subtle'; manage.textContent = 'Manage'; manage.addEventListener('click', () => dialog.open(profile))
      const close = document.createElement('button'); close.className = 'subtle'; close.textContent = 'Close window'; close.addEventListener('click', async () => { try { await invoke('close_profile', { profileId: profile.id }); announce(`${profile.name} window closed. Profile data remains saved.`) } catch (cause) { announce(String(cause)) } })
      actions.append(open, manage, close); card.append(avatar, copy, actions); grid.append(card)
    }
  } catch (cause) { count.textContent = 'Could not load saved profiles'; announce(`Profile catalog error: ${String(cause)}`) }
}

window.addEventListener('levi:profiles-changed', () => void renderProfiles())
void renderProfiles()
