/** Whether a typed server address is plain `http://` to another machine:
 * a password sent there crosses the network unencrypted. This computer
 * (localhost, 127.0.0.0/8, ::1) is not a network hop. */
export function sendsPlainText(server: string) {
  const text = server.trim()
  if (!/^http:\/\//i.test(text)) return false
  let host: string
  try {
    host = new URL(text).hostname.toLowerCase()
  } catch {
    return true
  }
  return !(host === 'localhost' || host.endsWith('.localhost') || host === '[::1]' || /^127\.\d+\.\d+\.\d+$/.test(host))
}
