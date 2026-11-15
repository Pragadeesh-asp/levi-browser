import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import './browser.css'

type NavigationState = {
  currentUrl: string | null
  entries: string[]
  index: number | null
  isLoading: boolean
  error: string | null
}

const profileId = document.documentElement.dataset.profile!
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
    <div class="address-form" id="address-form">
      <span class="secure-mark" aria-hidden="true">◈</span>
      <input id="address" type="text" autocomplete="off" spellcheck="false" placeholder="Search or enter a website address" aria-label="Search or enter a website address" />
      <button class="go-button" type="button" aria-label="Go">↵</button>
    </div>
    <label class="search-setting" title="Search engine">
      <span>SEARCH</span>
      <select id="search-engine" aria-label="Search engine">
        <option value="https://duckduckgo.com/?q={query}">DuckDuckGo</option>
        <option value="https://www.google.com/search?q={query}">Google</option>
        <option value="https://www.bing.com/search?q={query}">Bing</option>
      </select>
    </label>
    <span class="profile-chip"><i></i>${profileId.toUpperCase()} SPACE</span>
  </header>
  <main class="browser-empty" id="empty-state">
    <div class="empty-orbit">L<span>•</span></div>
    <p class="overline">LEVI BROWSER / ${profileId.toUpperCase()} ISOLATED SPACE</p>
    <h1>Your next<br/><em>destination.</em></h1>
    <p class="empty-copy">Enter a website address or a search above. This space keeps its own WebView2 storage.</p>
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

searchEngine.value = localStorage.getItem(searchKey) ?? defaultSearch
searchEngine.addEventListener('change', () => localStorage.setItem(searchKey, searchEngine.value))

function showMessage(text: string) {
  message.textContent = text
  message.classList.add('visible')
  window.setTimeout(() => message.classList.remove('visible'), 4500)
}

function updateState(state: NavigationState) {
  if (state.currentUrl && state.currentUrl !== 'about:blank') address.value = state.currentUrl
  emptyState.hidden = Boolean(state.currentUrl)
  loadingStrip.classList.toggle('active', state.isLoading)
  root.querySelector<HTMLButtonElement>('#back')!.disabled = state.index === null || state.index <= 0
  root.querySelector<HTMLButtonElement>('#forward')!.disabled = state.index === null || state.index + 1 >= state.entries.length
  root.querySelector<HTMLButtonElement>('#reload')!.disabled = !state.currentUrl
  root.querySelector<HTMLButtonElement>('#stop')!.hidden = !state.isLoading
  if (state.error) showMessage(state.error)
}

async function refreshState() {
  try {
    updateState(await invoke<NavigationState>('browser_state', { profileId }))
  } catch (error) {
    showMessage(`Could not read navigation state: ${String(error)}`)
  }
}

async function navigate(value: string) {
  console.info('[LEVI_DIAG] event=navigation_invoke_started profile=%s', profileId)
  showMessage('Navigation requested…')
  try {
    const target = await invoke<string>('browser_navigate', {
      profileId,
      input: value,
      searchTemplate: searchEngine.value,
    })
    console.info('[LEVI_DIAG] event=navigation_invoke_resolved profile=%s', profileId)
    address.value = target
    loadingStrip.classList.add('active')
    showMessage(`Opening ${target}`)
  } catch (error) {
    console.error('[LEVI_DIAG] event=navigation_invoke_rejected profile=%s error=%s', profileId, String(error))
    showMessage(String(error))
  }
}

address.addEventListener('keydown', (event) => {
  if (event.key === 'Enter') {
    event.preventDefault()
    void navigate(address.value)
  }
})
root.querySelector<HTMLButtonElement>('.go-button')!.addEventListener('click', () => void navigate(address.value))
root.querySelector<HTMLButtonElement>('#back')!.addEventListener('click', () => void invoke('browser_back', { profileId }).catch((error) => showMessage(String(error))))
root.querySelector<HTMLButtonElement>('#forward')!.addEventListener('click', () => void invoke('browser_forward', { profileId }).catch((error) => showMessage(String(error))))
root.querySelector<HTMLButtonElement>('#reload')!.addEventListener('click', () => void invoke('browser_reload', { profileId }).catch((error) => showMessage(String(error))))
root.querySelector<HTMLButtonElement>('#stop')!.addEventListener('click', () => void invoke('browser_stop', { profileId }).catch((error) => showMessage(String(error))))
root.querySelector<HTMLButtonElement>('#home')!.addEventListener('click', () => void navigate('https://duckduckgo.com/'))

window.addEventListener('keydown', (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'l') {
    event.preventDefault()
    address.focus()
    address.select()
  }
})

void listen<NavigationState>('browser:navigation-state', (event) => {
  const state = event.payload
  console.info(
    '[LEVI_DIAG] event=address_state_event_received profile=%s hasUrl=%s loading=%s errorPresent=%s',
    profileId,
    Boolean(state.currentUrl),
    state.isLoading,
    Boolean(state.error),
  )
  updateState(state)
}).then(() => {
  console.info('[LEVI_DIAG] event=address_state_listener_ready profile=%s', profileId)
  return refreshState()
}).catch((error: unknown) => {
  console.error('[LEVI_DIAG] event=address_state_listener_failed profile=%s error=%s', profileId, String(error))
  showMessage(`Could not listen for navigation state: ${String(error)}`)
})
