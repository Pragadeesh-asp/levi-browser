import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createProfileDialog, type BrowserProfile } from './profile-management'
import './browser.css'
import './levi-theme.css'
import './levi-effects.css'

type NavigationState = {
  currentUrl: string | null
  entries: string[]
  index: number | null
  isLoading: boolean
  error: string | null
}

type Tab = {
  id: string
  title: string | null
  faviconUri: string | null
  currentUrl: string | null
  lifecycle: 'empty' | 'creating' | 'ready' | 'closing'
  loading: boolean
  error: string | null
}

type TabsState = { profileId: string; activeTabId: string; tabs: Tab[] }
type NavigationEvent = { profileId: string; state: NavigationState }

const profileId = document.documentElement.dataset.profile || new URLSearchParams(location.search).get('profile') || ''
if (!/^[a-z0-9-]{1,80}$/.test(profileId)) throw new Error('Invalid browser profile identifier.')
const searchKey = 'levi-browser-search-template'
const defaultSearch = 'https://duckduckgo.com/?q={query}'
const root = document.querySelector<HTMLDivElement>('#browser')!

root.innerHTML = `
  <header class="browser-toolbar">
    <div class="nav-controls" aria-label="Navigation controls">
      <button class="icon-button" id="back" aria-label="Back" title="Back" disabled>‹</button>
      <button class="icon-button" id="forward" aria-label="Forward" title="Forward" disabled>›</button>
      <button class="icon-button" id="reload" aria-label="Reload" title="Reload" disabled>↻</button>
      <button class="icon-button stop-button" id="stop" aria-label="Stop loading" title="Stop loading" hidden>×</button>
      <button class="icon-button" id="home" aria-label="Home" title="Home">⌂</button>
    </div>
    <div class="address-form browser-address-beam" id="address-form">
      <span class="secure-mark" aria-hidden="true">◈</span>
      <input id="address" type="text" autocomplete="off" spellcheck="false" placeholder="Search or enter a website address" aria-label="Search or enter a website address" />
      <button class="go-button" type="button" aria-label="Go">↵</button>
    </div>
    <label class="search-setting" title="Search engine">
      <select id="search-engine" aria-label="Search engine">
        <option value="https://duckduckgo.com/?q={query}">DuckDuckGo</option>
        <option value="https://www.google.com/search?q={query}">Google</option>
        <option value="https://www.bing.com/search?q={query}">Bing</option>
      </select>
    </label>
    <button class="profile-chip" id="profile-menu-button" type="button" aria-haspopup="true" aria-expanded="false" aria-controls="profile-menu"><i></i><span id="profile-chip-label">Profile</span><span aria-hidden="true">⌄</span></button>
  </header>
  <section class="profile-menu" id="profile-menu" hidden aria-label="Browser profile menu"><div class="profile-menu-current"><span class="eyebrow">CURRENT PROFILE</span><strong id="profile-current-name"></strong></div><div class="profile-menu-list" id="profile-menu-list"></div><div class="profile-menu-actions"><button id="profile-add" type="button">＋ Add Profile</button><button id="profile-manage" type="button">Manage Profiles</button></div></section>
  <nav class="tab-strip" aria-label="Browser tabs"><div id="tabs" class="tabs"></div><button id="new-tab" class="new-tab" title="New tab" aria-label="New tab">+</button></nav>
  <main class="browser-empty" id="empty-state">
    <div class="empty-orbit">L<span>•</span></div>
    <p class="overline">LEVI BROWSER / ${profileId.toUpperCase()} ISOLATED SPACE</p>
    <h1>Your next<br/><em>destination.</em></h1>
    <p class="empty-copy">Enter a website address or a search above. Tabs in this space share its persistent WebView2 storage.</p>
    <div class="empty-shortcut"><kbd>Ctrl</kbd><span>+</span><kbd>L</kbd><span>Focus the address bar</span></div>
  </main>
  <div class="loading-strip" id="loading-strip"><span></span><span></span><span></span></div>
  <div class="browser-message" id="browser-message" role="status" aria-live="polite"></div>
`

const address = root.querySelector<HTMLInputElement>('#address')!
const searchEngine = root.querySelector<HTMLSelectElement>('#search-engine')!
const message = root.querySelector<HTMLDivElement>('#browser-message')!
const loadingStrip = root.querySelector<HTMLDivElement>('#loading-strip')!
const emptyState = root.querySelector<HTMLElement>('#empty-state')!
const tabsElement = root.querySelector<HTMLDivElement>('#tabs')!
const profileMenu = root.querySelector<HTMLElement>('#profile-menu')!
const profileMenuButton = root.querySelector<HTMLButtonElement>('#profile-menu-button')!
let tabsState: TabsState | null = null

searchEngine.value = localStorage.getItem(searchKey) ?? defaultSearch
searchEngine.addEventListener('change', () => localStorage.setItem(searchKey, searchEngine.value))

function showMessage(text: string) {
  message.textContent = text
  message.classList.add('visible')
  window.setTimeout(() => message.classList.remove('visible'), 4500)
}

function activeTabId() { return tabsState?.activeTabId ?? '' }

function updateNavigation(state: NavigationState) {
  address.value = state.currentUrl && state.currentUrl !== 'about:blank' ? state.currentUrl : ''
  emptyState.hidden = Boolean(state.currentUrl)
  loadingStrip.classList.toggle('active', state.isLoading)
  root.querySelector<HTMLButtonElement>('#back')!.disabled = state.index === null || state.index <= 0
  root.querySelector<HTMLButtonElement>('#forward')!.disabled = state.index === null || state.index + 1 >= state.entries.length
  root.querySelector<HTMLButtonElement>('#reload')!.disabled = !state.currentUrl
  root.querySelector<HTMLButtonElement>('#stop')!.hidden = !state.isLoading
  if (state.error) showMessage(state.error)
}

function renderTabs(state: TabsState) {
  if (state.profileId !== profileId) {
    console.warn('[LEVI_DIAG] ignored cross-profile tab state event')
    return
  }
  tabsState = state
  tabsElement.replaceChildren()
  let activeItem: HTMLDivElement | null = null
  for (const tab of state.tabs) {
    const item = document.createElement('div')
    item.className = `tab-item${tab.id === state.activeTabId ? ' active' : ''}`
    if (tab.id === state.activeTabId) activeItem = item
    item.title = tab.currentUrl ?? 'New tab'
    const select = document.createElement('button')
    select.className = 'tab-select'
    select.type = 'button'
    select.textContent = tab.title?.trim() || (tab.currentUrl ? new URL(tab.currentUrl).hostname : 'New tab')
    select.addEventListener('click', () => void invoke('browser_select_tab', { profileId, tabId: tab.id }).catch((error) => showMessage(String(error))))
    const status = document.createElement('span')
    status.className = `tab-status ${tab.loading || tab.lifecycle === 'creating' ? 'loading' : ''}`
    status.setAttribute('aria-hidden', 'true')
    const close = document.createElement('button')
    close.className = 'tab-close'
    close.type = 'button'
    close.setAttribute('aria-label', `Close ${select.textContent}`)
    close.textContent = '×'
    close.addEventListener('click', (event) => {
      event.stopPropagation()
      void invoke('browser_close_tab', { profileId, tabId: tab.id }).catch((error) => showMessage(String(error)))
    })
    const favicon = document.createElement('span')
    favicon.className = 'tab-favicon'
    favicon.setAttribute('aria-hidden', 'true')
    if (tab.faviconUri) {
      const image = document.createElement('img')
      image.src = tab.faviconUri
      image.alt = ''
      favicon.append(image)
    } else {
      favicon.textContent = '◈'
    }
    item.append(status, favicon, select, close)
    tabsElement.append(item)
  }
  activeItem?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}

async function refresh() {
  try {
    renderTabs(await invoke<TabsState>('browser_tabs_state', { profileId }))
    updateNavigation(await invoke<NavigationState>('browser_state', { profileId }))
  } catch (error) { showMessage(`Could not refresh browser state: ${String(error)}`) }
}

async function navigate(value: string) {
  const tabId = activeTabId()
  if (!tabId) { showMessage('The active tab is not ready yet.'); return }
  showMessage('Navigation requested…')
  try {
    const target = await invoke<string>('browser_navigate', { profileId, tabId, input: value, searchTemplate: searchEngine.value })
    address.value = target
    loadingStrip.classList.add('active')
  } catch (error) { showMessage(String(error)) }
}

address.addEventListener('keydown', (event) => {
  if (event.key === 'Enter') { event.preventDefault(); void navigate(address.value) }
})
root.querySelector<HTMLButtonElement>('.go-button')!.addEventListener('click', () => void navigate(address.value))
for (const [id, command] of [['back', 'browser_back'], ['forward', 'browser_forward'], ['reload', 'browser_reload'], ['stop', 'browser_stop']] as const) {
  root.querySelector<HTMLButtonElement>(`#${id}`)!.addEventListener('click', () => {
    const tabId = activeTabId()
    if (tabId) void invoke(command, { profileId, tabId }).catch((error) => showMessage(String(error)))
  })
}
root.querySelector<HTMLButtonElement>('#home')!.addEventListener('click', () => void navigate('https://duckduckgo.com/'))
root.querySelector<HTMLButtonElement>('#new-tab')!.addEventListener('click', async () => {
  try { await invoke('browser_new_tab', { profileId }) } catch (error) { showMessage(String(error)) }
})

function setProfileMenu(open: boolean) {
  profileMenu.hidden = !open
  profileMenuButton.setAttribute('aria-expanded', String(open))
}

async function refreshProfiles() {
  try {
    const profiles = await invoke<BrowserProfile[]>('list_profiles', { currentProfileId: profileId })
    const current = profiles.find(profile => profile.id === profileId)
    if (!current) { showMessage('This profile is no longer present in the local catalog.'); return }
    const chip = root.querySelector<HTMLElement>('#profile-chip-label')!
    chip.textContent = `${current.avatar} · ${current.name}`
    root.querySelector<HTMLElement>('#profile-current-name')!.textContent = current.name
    root.querySelector<HTMLElement>('#profile-menu-list')!.replaceChildren()
    for (const profile of profiles.filter(profile => profile.id !== profileId)) {
      const row = document.createElement('button')
      row.type = 'button'; row.className = 'profile-menu-item'; row.style.setProperty('--profile-accent', profile.accent)
      const avatar = document.createElement('span'); avatar.className = 'profile-menu-avatar'; avatar.textContent = profile.avatar
      const name = document.createElement('span'); name.textContent = profile.name
      const action = document.createElement('small'); action.textContent = 'Open in window'
      row.append(avatar, name, action)
      row.addEventListener('click', async () => {
        setProfileMenu(false)
        try { await invoke('open_profile', { profileId: profile.id, currentProfileId: profileId }) }
        catch (error) { showMessage(`Could not open ${profile.name}: ${String(error)}`) }
      })
      root.querySelector<HTMLElement>('#profile-menu-list')!.append(row)
    }
  } catch (error) { showMessage(`Could not load profiles: ${String(error)}`) }
}

const profileDialog = createProfileDialog(profileId, async newProfileId => {
  try { await invoke('open_profile', { profileId: newProfileId, currentProfileId: profileId }) }
  catch (error) { showMessage(`Profile was saved, but its window could not open: ${String(error)}`) }
})
// `hidden` can also be the string "until-found"; any hidden state means "open it".
profileMenuButton.addEventListener('click', () => setProfileMenu(profileMenu.hidden !== false))
root.querySelector<HTMLButtonElement>('#profile-add')!.addEventListener('click', () => { setProfileMenu(false); profileDialog.open() })
root.querySelector<HTMLButtonElement>('#profile-manage')!.addEventListener('click', () => {
  setProfileMenu(false)
  void invoke('show_profile_manager', { currentProfileId: profileId }).catch(error => showMessage(`Could not open profile manager: ${String(error)}`))
})
document.addEventListener('pointerdown', event => { if (!profileMenu.contains(event.target as Node) && event.target !== profileMenuButton && !profileMenuButton.contains(event.target as Node)) setProfileMenu(false) })
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && !profileMenu.hidden) {
    setProfileMenu(false)
    profileMenuButton.focus()
  }
})
window.addEventListener('levi:profiles-changed', () => void refreshProfiles())
void refreshProfiles()

window.addEventListener('keydown', (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'l') { event.preventDefault(); address.focus(); address.select() }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 't') { event.preventDefault(); void invoke('browser_new_tab', { profileId }).catch((error) => showMessage(String(error))) }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'w') { event.preventDefault(); const tabId = activeTabId(); if (tabId) void invoke('browser_close_tab', { profileId, tabId }).catch((error) => showMessage(String(error))) }
})

void Promise.all([
  listen('browser:profiles-changed', () => void refreshProfiles()),
  listen<NavigationEvent>('browser:navigation-state', (event) => {
    if (event.payload.profileId !== profileId) {
      console.warn('[LEVI_DIAG] ignored cross-profile navigation state event')
      return
    }
    updateNavigation(event.payload.state)
  }, { target: { kind: 'Webview', label: `chrome-${profileId}` } }),
  listen<TabsState>('browser:tabs-state', (event) => { renderTabs(event.payload); if (event.payload.activeTabId !== activeTabId()) { /* state refresh follows in command response */ } }, { target: { kind: 'Webview', label: `chrome-${profileId}` } }),
]).then(() => refresh()).catch((error: unknown) => showMessage(`Could not listen for browser updates: ${String(error)}`))
