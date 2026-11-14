import './profile.css'

const profileId = location.pathname.endsWith('profile-alpha.html')
  ? 'alpha'
  : location.pathname.endsWith('profile-beta.html')
    ? 'beta'
    : null
if (profileId !== 'alpha' && profileId !== 'beta') {
  document.body.textContent = 'Invalid isolated test profile.'
  throw new Error('Profile test page must be opened with profile=alpha or profile=beta')
}

const markerKey = 'levi-isolation-proof'
const cookieName = 'levi_isolation_proof'
const initialValue = profileId === 'alpha' ? 'ALPHA-LOCAL-7C2A' : 'BETA-LOCAL-91DF'
if (localStorage.getItem(markerKey) === null) localStorage.setItem(markerKey, initialValue)
if (!document.cookie.split('; ').some((cookie) => cookie.startsWith(`${cookieName}=`))) {
  document.cookie = `${cookieName}=${initialValue}; path=/; SameSite=Lax`
}

document.querySelector<HTMLDivElement>('#profile-probe')!.innerHTML = `
  <main class="probe-shell">
    <header><div class="brand-mark">L<span>•</span></div><span class="eyebrow">LOCAL PROFILE INSPECTOR</span><div class="live"><i></i> SESSION ACTIVE</div></header>
    <section class="probe-hero"><span class="eyebrow">WEBVIEW2 USER DATA FOLDER</span><h1>Profile <em>${profileId.toUpperCase()}</em></h1><p>This window is bound to its own persistent Tauri webview data directory. No remote site or account was opened.</p></section>
    <section class="storage-grid">
      <article><span class="eyebrow">LOCAL STORAGE</span><h2>Session marker</h2><code id="local-marker"></code><small>Key: ${markerKey}</small></article>
      <article><span class="eyebrow">COOKIE JAR</span><h2>Test cookie</h2><code id="cookie-marker"></code><small>Key: ${cookieName}</small></article>
    </section>
    <section class="proof-callout"><span>✓</span><div><strong>Independent session storage</strong><p>Change or clear data here during the lab and the other profile’s values remain separate.</p></div></section>
    <footer>TEST PROFILE ${profileId.toUpperCase()} · LOCAL ORIGIN · NO CREDENTIALS</footer>
  </main>
`

function showValues() {
  document.querySelector<HTMLElement>('#local-marker')!.textContent = localStorage.getItem(markerKey) ?? '(not set)'
  const value = document.cookie.split('; ').find((cookie) => cookie.startsWith(`${cookieName}=`))?.split('=').slice(1).join('=')
  document.querySelector<HTMLElement>('#cookie-marker')!.textContent = value || '(not set)'
}

showValues()
