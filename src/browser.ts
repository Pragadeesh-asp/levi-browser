import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createProfileDialog, type BrowserProfile } from './profile-management'
import './browser.css'
import './levi-theme.css'
import './levi-carbon.css'
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
  <nav class="tab-strip" aria-label="Browser tabs">
    <button class="profile-chip" id="profile-menu-button" type="button" aria-haspopup="true" aria-expanded="false" aria-controls="profile-menu" title="Switch Space"><i></i><span id="profile-chip-label">Profile</span><span class="chip-caret" aria-hidden="true">▾</span></button>
    <div id="tabs" class="tabs"></div>
    <button id="new-tab" class="new-tab" title="New tab (Ctrl+T)" aria-label="New tab">+</button>
  </nav>
  <header class="browser-toolbar">
    <div class="nav-controls" aria-label="Navigation controls">
      <button class="icon-button" id="back" aria-label="Back" title="Back" disabled>‹</button>
      <button class="icon-button" id="forward" aria-label="Forward" title="Forward" disabled>›</button>
      <button class="icon-button" id="reload" aria-label="Reload" title="Reload" disabled>↻</button>
      <button class="icon-button stop-button" id="stop" aria-label="Stop loading" title="Stop loading" hidden>×</button>
      <button class="icon-button" id="home" aria-label="Home" title="Home">⌂</button>
    </div>
    <div class="address-form" id="address-form">
      <span class="secure-mark" aria-hidden="true">⌕</span>
      <input id="address" type="text" autocomplete="off" spellcheck="false" placeholder="Search or enter a website address" aria-label="Search or enter a website address" />
      <label class="search-setting" title="Search engine for typed searches">
        <select id="search-engine" aria-label="Search engine">
          <option value="https://duckduckgo.com/?q={query}">DuckDuckGo</option>
          <option value="https://www.google.com/search?q={query}">Google</option>
          <option value="https://www.bing.com/search?q={query}">Bing</option>
        </select>
      </label>
      <button class="bookmark-button" id="bookmark" type="button" aria-label="Bookmark this page" aria-pressed="false" title="Bookmark this page" disabled>☆</button>
      <button class="go-button" type="button" aria-label="Go" title="Go (Enter)">→</button>
    </div>
    <div class="toolbar-tools">
      <button class="icon-button" id="tab-search-button" type="button" aria-label="Search open tabs" title="Search open tabs in every Space (Ctrl+Shift+A)">⧉</button>
      <button class="icon-button" id="library-button" type="button" aria-label="History and bookmarks" title="History and bookmarks (Ctrl+H)">☰</button>
    </div>
  </header>
  <section class="overlay-panel library-panel" id="library-panel" hidden aria-label="History and bookmarks">
    <header class="overlay-head">
      <div class="library-tabs" role="tablist">
        <button type="button" role="tab" id="library-tab-history" aria-selected="true">History</button>
        <button type="button" role="tab" id="library-tab-bookmarks" aria-selected="false">Bookmarks</button>
      </div>
      <input id="library-filter" type="search" autocomplete="off" spellcheck="false" placeholder="Filter by title or address" aria-label="Filter" />
      <button type="button" class="subtle-button" id="library-clear">Clear history</button>
      <button type="button" class="icon-button" id="library-close" aria-label="Close">×</button>
    </header>
    <ul class="overlay-list" id="library-list"></ul>
  </section>
  <section class="overlay-panel tab-search" id="tab-search" hidden aria-label="Search open tabs">
    <header class="overlay-head"><span class="overlay-mark" aria-hidden="true">⌕</span><input id="tab-search-input" type="search" autocomplete="off" spellcheck="false" placeholder="Find an open tab in any Space" aria-label="Find an open tab" aria-controls="tab-search-list" /><kbd>Esc</kbd></header>
    <ul class="overlay-list" id="tab-search-list" role="listbox"></ul>
  </section>
  <section class="profile-menu" id="profile-menu" hidden aria-label="Browser profile menu"><div class="profile-menu-current"><span class="eyebrow">CURRENT SPACE</span><strong id="profile-current-name"></strong></div><div class="profile-menu-list" id="profile-menu-list"></div><div class="profile-menu-actions"><button id="profile-add" type="button">＋ Add Space</button><button id="profile-manage" type="button">Manage Spaces</button></div></section>
  <main class="browser-empty" id="empty-state">
    <div class="empty-orbit">L<span>•</span></div>
    <p class="overline">LEVI BROWSER · ${profileId.toUpperCase()} SPACE</p>
    <h1>Where to?</h1>
    <p class="empty-copy">Type an address or a search above. Every tab in this Space keeps its own sign-ins and history, separate from your other Spaces.</p>
    <button type="button" class="restore-session" id="restore-session" hidden></button>
    <div class="empty-shortcut"><kbd>Ctrl</kbd><span>+</span><kbd>L</kbd><span>Focus the address bar</span></div>
    <div class="empty-shortcut"><kbd>Ctrl</kbd><span>+</span><kbd>Shift</kbd><span>+</span><kbd>A</kbd><span>Find any open tab</span></div>
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
let renderedTabIds: Set<string> | null = null
const restoreButton = root.querySelector<HTMLButtonElement>('#restore-session')!
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)')

/* ---------- Panels drawn over the page ----------
 * Pages are native views above this toolbar, so while any panel is open the
 * active page is hidden and shown again when the last one closes. */
const openOverlays = new Set<string>()
function setOverlay(name: string, open: boolean) {
  const before = openOverlays.size
  if (open) openOverlays.add(name); else openOverlays.delete(name)
  if ((before === 0) !== (openOverlays.size === 0)) {
    void invoke('browser_set_overlay', { profileId, open: openOverlays.size > 0 }).catch(error => showMessage(String(error)))
  }
}

function closeTabAnimated(tabId: string) {
  const run = () => void invoke('browser_close_tab', { profileId, tabId }).catch((error) => showMessage(String(error)))
  const item = tabsElement.querySelector<HTMLElement>(`[data-tab-id="${CSS.escape(tabId)}"]`)
  if (!item || reducedMotion.matches) { run(); return }
  item.classList.add('tab-leaving')
  window.setTimeout(run, 150)
}

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
  if (state.currentUrl) restoreButton.hidden = true
  void refreshBookmark(state.currentUrl)
}

/* ---------- Bookmark star ---------- */
const bookmarkButton = root.querySelector<HTMLButtonElement>('#bookmark')!
let bookmarkUrl: string | null = null
function paintBookmark(on: boolean) {
  bookmarkButton.textContent = on ? '★' : '☆'
  bookmarkButton.classList.toggle('is-on', on)
  bookmarkButton.setAttribute('aria-pressed', String(on))
  bookmarkButton.title = on ? 'Remove bookmark' : 'Bookmark this page'
}
async function refreshBookmark(url: string | null) {
  const web = url && /^https?:\/\//.test(url) ? url : null
  bookmarkUrl = web
  bookmarkButton.disabled = !web
  if (!web) { paintBookmark(false); return }
  try {
    const on = await invoke<boolean>('browser_is_bookmarked', { profileId, url: web })
    if (bookmarkUrl === web) paintBookmark(on)
  } catch { paintBookmark(false) }
}
bookmarkButton.addEventListener('click', async () => {
  if (!bookmarkUrl) return
  const tab = tabsState?.tabs.find(item => item.id === tabsState?.activeTabId)
  try {
    const on = await invoke<boolean>('browser_toggle_bookmark', { profileId, url: bookmarkUrl, title: tab?.title ?? '' })
    paintBookmark(on)
    bookmarkButton.classList.remove('pop'); void bookmarkButton.offsetWidth; bookmarkButton.classList.add('pop')
    showMessage(on ? 'Bookmarked in this Space.' : 'Bookmark removed.')
  } catch (error) { showMessage(String(error)) }
})

function renderTabs(state: TabsState) {
  if (state.profileId !== profileId) {
    console.warn('[LEVI_DIAG] ignored cross-profile tab state event')
    return
  }
  const activeChanged = tabsState?.activeTabId !== state.activeTabId
  tabsState = state
  tabsElement.replaceChildren()
  let activeItem: HTMLDivElement | null = null
  const firstRender = renderedTabIds === null
  const known = renderedTabIds ?? new Set<string>()
  renderedTabIds = new Set(state.tabs.map(tab => tab.id))
  // Switching tabs shows that tab's page again; keep it hidden while a panel is open.
  if (activeChanged && openOverlays.size) void invoke('browser_set_overlay', { profileId, open: true }).catch(() => undefined)
  for (const tab of state.tabs) {
    const item = document.createElement('div')
    const loading = tab.loading || tab.lifecycle === 'creating'
    item.className = `tab-item${tab.id === state.activeTabId ? ' active' : ''}${loading ? ' is-loading' : ''}${!firstRender && !known.has(tab.id) ? ' tab-enter' : ''}`
    if (tab.id === state.activeTabId) activeItem = item
    item.dataset.tabId = tab.id
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
      closeTabAnimated(tab.id)
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
  if (profileMenu.hidden === !open) return
  profileMenu.hidden = !open
  profileMenuButton.setAttribute('aria-expanded', String(open))
  setOverlay('profile-menu', open)
}

async function refreshProfiles() {
  try {
    const profiles = await invoke<BrowserProfile[]>('list_profiles', { currentProfileId: profileId })
    const current = profiles.find(profile => profile.id === profileId)
    if (!current) { showMessage('This profile is no longer present in the local catalog.'); return }
    // Space-colored chrome: the Space's accent runs along the toolbar, the active tab and the chip.
    if (/^#[0-9a-f]{3,8}$/i.test(current.accent)) document.documentElement.style.setProperty('--space-accent', current.accent)
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

/* ---------- Library: history and bookmarks for this Space ---------- */
type HistoryEntry = { url: string; title: string; visitedAt: number }
type Bookmark = { url: string; title: string; addedAt: number }
const libraryPanel = root.querySelector<HTMLElement>('#library-panel')!
const libraryList = root.querySelector<HTMLUListElement>('#library-list')!
const libraryFilter = root.querySelector<HTMLInputElement>('#library-filter')!
const libraryClear = root.querySelector<HTMLButtonElement>('#library-clear')!
let libraryView: 'history' | 'bookmarks' = 'history'
let libraryTimer = 0

function when(seconds: number) {
  const date = new Date(seconds * 1000)
  const today = new Date()
  const sameDay = date.toDateString() === today.toDateString()
  return sameDay ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : date.toLocaleDateString([], { month: 'short', day: 'numeric' })
}
function hostOf(url: string) { try { return new URL(url).hostname } catch { return url } }

function pageRow(url: string, title: string, meta: string, onOpen: () => void, extra?: HTMLElement) {
  const li = document.createElement('li')
  const open = document.createElement('button')
  open.type = 'button'; open.className = 'overlay-row'
  const strong = document.createElement('strong'); strong.textContent = title || hostOf(url)
  const small = document.createElement('small'); small.textContent = url
  const time = document.createElement('span'); time.className = 'overlay-meta'; time.textContent = meta
  open.append(strong, small, time)
  open.addEventListener('click', onOpen)
  li.append(open)
  if (extra) li.append(extra)
  return li
}

async function loadLibrary() {
  const query = libraryFilter.value.trim().toLowerCase()
  libraryClear.hidden = libraryView !== 'history'
  for (const [id, view] of [['#library-tab-history', 'history'], ['#library-tab-bookmarks', 'bookmarks']] as const) {
    root.querySelector<HTMLButtonElement>(id)!.setAttribute('aria-selected', String(libraryView === view))
  }
  try {
    const rows: HTMLLIElement[] = []
    const openPage = (url: string) => () => { setLibrary(false); void navigate(url) }
    if (libraryView === 'history') {
      for (const entry of await invoke<HistoryEntry[]>('browser_history', { profileId, query })) rows.push(pageRow(entry.url, entry.title, when(entry.visitedAt), openPage(entry.url)))
    } else {
      const marks = (await invoke<Bookmark[]>('browser_bookmarks', { profileId })).filter(mark => !query || mark.url.toLowerCase().includes(query) || mark.title.toLowerCase().includes(query))
      for (const mark of marks) {
        const remove = document.createElement('button')
        remove.type = 'button'; remove.className = 'overlay-remove'; remove.textContent = '×'; remove.setAttribute('aria-label', `Remove bookmark ${mark.title || mark.url}`)
        remove.addEventListener('click', async () => {
          try { await invoke('browser_toggle_bookmark', { profileId, url: mark.url, title: mark.title }); void loadLibrary(); void refreshBookmark(bookmarkUrl) } catch (error) { showMessage(String(error)) }
        })
        rows.push(pageRow(mark.url, mark.title, when(mark.addedAt), openPage(mark.url), remove))
      }
    }
    if (!rows.length) {
      const empty = document.createElement('li'); empty.className = 'overlay-empty'
      empty.textContent = query ? `Nothing matches “${libraryFilter.value.trim()}”.` : libraryView === 'history' ? 'No pages visited in this Space yet.' : 'No bookmarks yet. Use ☆ in the address bar.'
      rows.push(empty)
    }
    libraryList.replaceChildren(...rows)
  } catch (error) { showMessage(`Could not load the library: ${String(error)}`) }
}

function setLibrary(open: boolean, view?: typeof libraryView) {
  if (view) libraryView = view
  if (open) { setTabSearch(false); setProfileMenu(false) }
  if (libraryPanel.hidden === !open) { if (open) void loadLibrary(); return }
  libraryPanel.hidden = !open
  setOverlay('library', open)
  if (open) { libraryFilter.value = ''; void loadLibrary(); requestAnimationFrame(() => libraryFilter.focus()) }
}
root.querySelector<HTMLButtonElement>('#library-button')!.addEventListener('click', () => setLibrary(libraryPanel.hidden !== false))
root.querySelector<HTMLButtonElement>('#library-close')!.addEventListener('click', () => setLibrary(false))
root.querySelector<HTMLButtonElement>('#library-tab-history')!.addEventListener('click', () => { libraryView = 'history'; void loadLibrary() })
root.querySelector<HTMLButtonElement>('#library-tab-bookmarks')!.addEventListener('click', () => { libraryView = 'bookmarks'; void loadLibrary() })
libraryFilter.addEventListener('input', () => { window.clearTimeout(libraryTimer); libraryTimer = window.setTimeout(() => void loadLibrary(), 120) })
let clearArmed = 0
libraryClear.addEventListener('click', async () => {
  // Two-step clear instead of a system dialog: the first click arms it for a few seconds.
  if (!clearArmed) {
    libraryClear.textContent = 'Click again to clear'; libraryClear.classList.add('armed')
    clearArmed = window.setTimeout(() => { clearArmed = 0; libraryClear.textContent = 'Clear history'; libraryClear.classList.remove('armed') }, 3000)
    return
  }
  window.clearTimeout(clearArmed); clearArmed = 0
  libraryClear.textContent = 'Clear history'; libraryClear.classList.remove('armed')
  try { await invoke('browser_clear_history', { profileId }); showMessage('History cleared for this Space.'); void loadLibrary() } catch (error) { showMessage(String(error)) }
})

/* ---------- Tab search across every open Space ---------- */
type OpenTab = { profileId: string; profileName: string; accent: string; tabId: string; title: string; url: string; active: boolean }
const tabSearch = root.querySelector<HTMLElement>('#tab-search')!
const tabSearchInput = root.querySelector<HTMLInputElement>('#tab-search-input')!
const tabSearchList = root.querySelector<HTMLUListElement>('#tab-search-list')!
let allTabs: OpenTab[] = []
let tabMatches: OpenTab[] = []
let tabIndex = 0

function paintTabSearch() {
  const q = tabSearchInput.value.trim().toLowerCase()
  tabMatches = allTabs.filter(tab => !q || `${tab.title} ${tab.url} ${tab.profileName}`.toLowerCase().includes(q))
  tabIndex = Math.min(tabIndex, Math.max(0, tabMatches.length - 1))
  const rows = tabMatches.map((tab, i) => {
    const li = document.createElement('li')
    const row = document.createElement('button')
    row.type = 'button'; row.className = `overlay-row${i === tabIndex ? ' is-active' : ''}`
    row.setAttribute('role', 'option'); row.setAttribute('aria-selected', String(i === tabIndex))
    const space = document.createElement('span'); space.className = 'overlay-space'; space.textContent = tab.profileName
    if (/^#[0-9a-f]{3,8}$/i.test(tab.accent)) space.style.setProperty('--row-accent', tab.accent)
    const strong = document.createElement('strong'); strong.textContent = tab.title || (tab.url ? hostOf(tab.url) : 'New tab')
    const small = document.createElement('small'); small.textContent = tab.url || 'Empty tab'
    row.append(space, strong, small)
    row.addEventListener('mouseenter', () => { tabIndex = i; for (const [n, el] of [...tabSearchList.querySelectorAll('.overlay-row')].entries()) el.classList.toggle('is-active', n === i) })
    row.addEventListener('click', () => void goToTab(tab))
    li.append(row)
    return li
  })
  if (!rows.length) {
    const empty = document.createElement('li'); empty.className = 'overlay-empty'
    empty.textContent = q ? `No open tab matches “${tabSearchInput.value.trim()}”.` : 'No open tabs.'
    rows.push(empty)
  }
  tabSearchList.replaceChildren(...rows)
  tabSearchList.querySelector('.is-active')?.scrollIntoView({ block: 'nearest' })
}

async function goToTab(tab: OpenTab) {
  setTabSearch(false)
  try {
    if (tab.profileId === profileId) await invoke('browser_select_tab', { profileId, tabId: tab.tabId })
    else await invoke('browser_focus_tab', { profileId, targetProfileId: tab.profileId, tabId: tab.tabId })
  } catch (error) { showMessage(String(error)) }
}

function setTabSearch(open: boolean) {
  if (open) { setLibrary(false); setProfileMenu(false) }
  if (tabSearch.hidden === !open) return
  tabSearch.hidden = !open
  setOverlay('tab-search', open)
  if (!open) return
  tabSearchInput.value = ''; tabIndex = 0; allTabs = []; paintTabSearch()
  requestAnimationFrame(() => tabSearchInput.focus())
  void invoke<OpenTab[]>('browser_all_tabs', { profileId }).then(found => {
    // This Space first, then the others; the tab you're on goes last within its Space.
    allTabs = found.sort((a, b) => Number(b.profileId === profileId) - Number(a.profileId === profileId) || Number(a.active) - Number(b.active))
    paintTabSearch()
  }).catch(error => showMessage(`Could not list open tabs: ${String(error)}`))
}
root.querySelector<HTMLButtonElement>('#tab-search-button')!.addEventListener('click', () => setTabSearch(tabSearch.hidden !== false))
tabSearchInput.addEventListener('input', () => { tabIndex = 0; paintTabSearch() })
tabSearchInput.addEventListener('keydown', event => {
  if (event.key === 'ArrowDown') { event.preventDefault(); tabIndex = Math.min(tabMatches.length - 1, tabIndex + 1); paintTabSearch() }
  else if (event.key === 'ArrowUp') { event.preventDefault(); tabIndex = Math.max(0, tabIndex - 1); paintTabSearch() }
  else if (event.key === 'Enter') { event.preventDefault(); const tab = tabMatches[tabIndex]; if (tab) void goToTab(tab) }
})

/* ---------- Restore the last session ---------- */
async function offerRestore() {
  const tabs = tabsState?.tabs ?? []
  if (tabs.length !== 1 || tabs[0].currentUrl) return
  try {
    const pages = await invoke<string[]>('browser_last_session', { profileId })
    if (!pages.length || tabsState?.tabs.some(tab => tab.currentUrl)) return
    restoreButton.textContent = `↺  Restore ${pages.length} tab${pages.length === 1 ? '' : 's'} from last time`
    restoreButton.title = pages.map(hostOf).join(', ')
    restoreButton.hidden = false
    restoreButton.onclick = async () => {
      restoreButton.hidden = true
      try { for (const url of pages) await openLaunchUrl(url) } catch (error) { showMessage(`Could not restore every tab: ${String(error)}`) }
    }
  } catch { /* nothing saved yet */ }
}

window.addEventListener('keydown', (event) => {
  const mod = event.ctrlKey || event.metaKey
  const key = event.key.toLowerCase()
  if (event.key === 'Escape' && (!libraryPanel.hidden || !tabSearch.hidden)) { event.preventDefault(); setLibrary(false); setTabSearch(false); return }
  if (mod && event.shiftKey && key === 'a') { event.preventDefault(); setTabSearch(tabSearch.hidden !== false); return }
  if (mod && !event.shiftKey && key === 'h') { event.preventDefault(); setLibrary(libraryPanel.hidden !== false, 'history'); return }
  if (mod && event.shiftKey && key === 'o') { event.preventDefault(); setLibrary(true, 'bookmarks'); return }
  if (mod && !event.shiftKey && key === 'd') { event.preventDefault(); bookmarkButton.click(); return }
  if (mod && key === 'l') { event.preventDefault(); setLibrary(false); setTabSearch(false); address.focus(); address.select() }
  if (mod && key === 't') { event.preventDefault(); void invoke('browser_new_tab', { profileId }).catch((error) => showMessage(String(error))) }
  if (mod && key === 'w') { event.preventDefault(); const tabId = activeTabId(); if (tabId) closeTabAnimated(tabId) }
})

// Pages sent by other LEVI apps (`--space <id> --open <url>`) wait in Rust until this
// chrome takes them. The empty first tab is reused; otherwise each page gets a new tab.
let drainingLaunches = false
let drainAgain = false
async function openLaunchUrl(url: string) {
  const active = tabsState?.tabs.find((tab) => tab.id === tabsState?.activeTabId)
  const tabId = active && active.lifecycle === 'empty' && !active.currentUrl ? active.id : await invoke<string>('browser_new_tab', { profileId })
  address.value = await invoke<string>('browser_navigate', { profileId, tabId, input: url, searchTemplate: searchEngine.value })
}
async function drainLaunchQueue() {
  if (drainingLaunches) { drainAgain = true; return }
  drainingLaunches = true
  try {
    do {
      drainAgain = false
      for (const url of await invoke<string[]>('browser_take_launch_urls', { profileId })) await openLaunchUrl(url)
    } while (drainAgain)
  } catch (error) { showMessage(`Could not open the requested page: ${String(error)}`) }
  finally { drainingLaunches = false }
}

void Promise.all([
  listen('browser:profiles-changed', () => void refreshProfiles()),
  listen('browser:open-request', () => void drainLaunchQueue(), { target: { kind: 'Webview', label: `chrome-${profileId}` } }),
  listen<NavigationEvent>('browser:navigation-state', (event) => {
    if (event.payload.profileId !== profileId) {
      console.warn('[LEVI_DIAG] ignored cross-profile navigation state event')
      return
    }
    updateNavigation(event.payload.state)
  }, { target: { kind: 'Webview', label: `chrome-${profileId}` } }),
  listen<TabsState>('browser:tabs-state', (event) => { renderTabs(event.payload); if (event.payload.activeTabId !== activeTabId()) { /* state refresh follows in command response */ } }, { target: { kind: 'Webview', label: `chrome-${profileId}` } }),
  // Tab search in another Space asked this Space to bring one of its tabs forward.
  listen<string>('browser:select-tab', (event) => {
    void invoke('browser_select_tab', { profileId, tabId: event.payload }).catch((error) => showMessage(String(error)))
  }, { target: { kind: 'Webview', label: `chrome-${profileId}` } }),
]).then(async () => { await refresh(); await drainLaunchQueue(); await offerRestore() }).catch((error: unknown) => showMessage(`Could not listen for browser updates: ${String(error)}`))
