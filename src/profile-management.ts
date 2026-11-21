import { invoke } from '@tauri-apps/api/core'

export interface BrowserProfile { id: string; name: string; avatar: string; accent: string; createdAt: string }

export function createProfileDialog(currentProfileId: string | null, onCreated: (id: string) => void) {
  const dialog = document.createElement('dialog')
  dialog.className = 'profile-dialog'
  dialog.innerHTML = `<form method="dialog" class="profile-dialog-card"><header><div><span class="eyebrow">BROWSER PROFILES</span><h2 id="profile-dialog-title">Add profile</h2></div><button class="profile-dialog-close" value="cancel" aria-label="Close">×</button></header><label>Profile name<input name="name" maxlength="40" required autocomplete="off"/></label><div class="profile-dialog-row"><label>Avatar / initial<input name="avatar" maxlength="3" value="L" required/></label><label>Accent<input name="accent" type="color" value="#C7F000"/></label></div><p class="profile-dialog-error" role="alert"></p><footer><button type="button" class="subtle" data-cancel>Cancel</button><button type="submit" class="primary" data-save>Create profile</button></footer></form>`
  document.body.append(dialog)
  const form = dialog.querySelector<HTMLFormElement>('form')!
  const name = form.elements.namedItem('name') as HTMLInputElement
  const avatar = form.elements.namedItem('avatar') as HTMLInputElement
  const accent = form.elements.namedItem('accent') as HTMLInputElement
  const error = dialog.querySelector<HTMLElement>('.profile-dialog-error')!
  let editingId: string | null = null

  dialog.querySelector('[data-cancel]')!.addEventListener('click', () => dialog.close())
  dialog.querySelector('.profile-dialog-close')!.addEventListener('click', () => dialog.close())
  dialog.addEventListener('click', event => { if (event.target === dialog) dialog.close() })
  form.addEventListener('submit', async event => {
    event.preventDefault()
    const submit = dialog.querySelector<HTMLButtonElement>('[data-save]')!
    submit.disabled = true; error.textContent = ''
    try {
      if (editingId) {
        await invoke('update_profile', { currentProfileId, profileId: editingId, name: name.value, avatar: avatar.value, accent: accent.value })
      } else {
        const profile = await invoke<BrowserProfile>('create_profile', { currentProfileId, name: name.value, avatar: avatar.value, accent: accent.value })
        onCreated(profile.id)
      }
      dialog.close()
      window.dispatchEvent(new CustomEvent('levi:profiles-changed'))
    } catch (cause) { error.textContent = String(cause) }
    finally { submit.disabled = false }
  })

  return {
    open(profile?: BrowserProfile) {
      editingId = profile?.id ?? null
      name.value = profile?.name ?? ''
      avatar.value = profile?.avatar ?? 'L'
      accent.value = profile?.accent ?? '#C7F000'
      error.textContent = ''
      dialog.querySelector('#profile-dialog-title')!.textContent = profile ? 'Manage profile' : 'Add profile'
      dialog.querySelector<HTMLButtonElement>('[data-save]')!.textContent = profile ? 'Save changes' : 'Create profile'
      dialog.showModal()
      name.focus()
    },
  }
}
