import { invoke } from './bridge'

// Board images reuse the account avatar/wallpaper bridge calls: the Go handlers
// only sanitize the id into a media subdirectory (no account lookup), so a board
// id works the same as an account id and needs no backend changes.

export async function writeBoardAvatarFile(boardId: string, file: { name: string; mime: string; data: string }) {
  const res = await invoke<{ url: string }>('account.writeAvatarFile', {
    id: boardId,
    filename: file.name,
    mime: file.mime,
    data: file.data,
  })
  return res.url
}

export async function writeBoardWallpaperFile(boardId: string, file: { name: string; mime: string; data: string }) {
  const res = await invoke<{ url: string }>('account.writeChatWallpaperFile', {
    id: boardId,
    filename: file.name,
    mime: file.mime,
    data: file.data,
  })
  return res.url
}
