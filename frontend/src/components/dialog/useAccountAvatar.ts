import { type ChangeEvent, useState } from 'react'
import { readFileData } from '../../lib/readFileData'
import { setAccountAvatar, writeAccountAvatarFile } from '../../states/accounts'
import { showToast } from '../../states/ui'

// Avatar upload/URL persistence shared by the settings header avatar (click to
// change) and the dedicated Avatar card. `onChanged` fires with the new URL so
// the caller can sync any local form state.
export function useAccountAvatar(accountId: string, onChanged?: (url: string) => void) {
  const [avatarBusy, setAvatarBusy] = useState(false)

  const persistAvatarUrl = async (url: string) => {
    setAvatarBusy(true)
    try {
      if (await setAccountAvatar(accountId, url)) {
        onChanged?.(url.trim())
        return true
      }
      return false
    } finally {
      setAvatarBusy(false)
    }
  }

  const persistAvatarFile = async (file: File) => {
    if (!file) return false
    setAvatarBusy(true)
    try {
      const data = await readFileData(file)
      const url = await writeAccountAvatarFile(accountId, { name: file.name, mime: file.type, data })
      if (await setAccountAvatar(accountId, url)) {
        onChanged?.(url)
        return true
      }
      return false
    } catch (error) {
      showToast(error instanceof Error ? error.message : 'Failed to upload avatar', 'error')
      return false
    } finally {
      setAvatarBusy(false)
    }
  }

  const onAvatarFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    await persistAvatarFile(file)
  }

  return { avatarBusy, persistAvatarUrl, persistAvatarFile, onAvatarFile }
}
