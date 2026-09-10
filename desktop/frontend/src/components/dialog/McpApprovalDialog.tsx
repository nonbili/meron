import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'
import { useEscapeKey } from '../../lib/useEscapeKey'

type Approval = {
  id: string
  client: string
  account: string
  tool: string
  expires_at: string
  preview: {
    kind: 'send' | 'delete'
    to?: string
    cc?: string
    bcc?: string
    subject?: string
    body?: string
    in_reply_to?: string
    references?: string
    selection?: { folder: string; messages: { uid: number; subject: string; from: string; date: number }[] }
  }
}

export function McpApprovalDialog() {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const [pending, setPending] = useState<Approval[]>([])
  const [busy, setBusy] = useState(false)
  // A failed operation is no longer pending, so its report cannot live on the
  // request itself: the next poll would drop the prompt and the reason with it.
  const [failure, setFailure] = useState<{ client: string; tool: string; message: string } | null>(null)
  const revision = useRef(0)
  const changing = useRef(false)
  const refreshPending = useRef<() => Promise<void>>(undefined)
  const rejectButton = useRef<HTMLButtonElement>(null)
  const closeButton = useRef<HTMLButtonElement>(null)
  const dialog = useRef<HTMLElement>(null)
  const current = failure ? undefined : pending[0]

  useEffect(() => {
    let active = true
    const refresh = async () => {
      if (changing.current) return
      const version = revision.current
      try {
        const result = await invoke<Approval[]>('mcp.pending')
        if (active && version === revision.current && Array.isArray(result)) {
          setPending(result.sort((a, b) => a.expires_at.localeCompare(b.expires_at) || a.id.localeCompare(b.id)))
        }
      } catch {
        // An unavailable backend cannot authorize anything. Clear stale prompts.
        if (active && version === revision.current) setPending([])
      }
    }
    refreshPending.current = refresh
    void refresh()
    // The backend announces every change to the pending set, so an idle Meron
    // asks it nothing. Polling remains only where events are unavailable.
    const eventsOn = (window as any).runtime?.EventsOn
    const unsubscribe = typeof eventsOn === 'function' ? eventsOn('mcp.approvals', () => void refresh()) : undefined
    const timer = unsubscribe ? undefined : setInterval(() => void refresh(), 1500)
    return () => {
      active = false
      if (timer !== undefined) clearInterval(timer)
      if (typeof unsubscribe === 'function') unsubscribe()
    }
  }, [])

  // A request that nobody answers expires on its own; drop the prompt when it
  // does rather than leaving a dead dialog on screen.
  useEffect(() => {
    if (!current) return
    const remaining = new Date(current.expires_at).getTime() - Date.now()
    const timer = setTimeout(() => void refreshPending.current?.(), Math.max(remaining, 0) + 250)
    return () => clearTimeout(timer)
  }, [current?.id, current?.expires_at])

  useEffect(() => {
    if (!current && !failure) return
    const previous = document.activeElement as HTMLElement | null
    ;(failure ? closeButton : rejectButton).current?.focus()
    return () => {
      if (previous?.isConnected) previous.focus()
    }
  }, [current?.id, Boolean(failure)])

  async function resolve(approve: boolean) {
    if (!current || changing.current) return
    const request = current
    changing.current = true
    revision.current++
    setBusy(true)
    try {
      const result = await invoke<{ status: string; error?: string }>('mcp.resolve', { id: request.id, approve })
      if (result.status === 'failed') {
        setFailure({ client: request.client, tool: request.tool, message: result.error || t('mcp.operationFailed') })
      }
      setPending((items) => items.filter((item) => item.id !== request.id))
    } catch (err) {
      setFailure({ client: request.client, tool: request.tool, message: String(err) })
      setPending((items) => items.filter((item) => item.id !== request.id))
    } finally {
      changing.current = false
      setBusy(false)
    }
  }
  useEscapeKey(() => (failure ? setFailure(null) : void resolve(false)), Boolean(current) || Boolean(failure))
  if (failure) {
    return (
      <div className="fixed inset-0 z-[130] flex items-center justify-center bg-black/50 p-4">
        <section
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="mcp-failure-title"
          className="flex w-full max-w-md flex-col gap-3 rounded-2xl border border-border bg-chats p-5 text-primary shadow-2xl"
        >
          <h2 id="mcp-failure-title" className="font-bold text-rose-500">
            {t('mcp.failed')}
          </h2>
          <p className="break-words text-sm select-text">{failure.message}</p>
          <p className="break-all text-xs text-secondary">
            {failure.client} · {failure.tool}
          </p>
          <div className="flex justify-end">
            <button
              ref={closeButton}
              onClick={() => setFailure(null)}
              className="rounded-lg bg-hover px-4 py-2 text-sm"
            >
              {t('buttons.close')}
            </button>
          </div>
        </section>
      </div>
    )
  }
  if (!current) return null
  const deleting = current.preview.kind === 'delete'
  const selection = current.preview.selection

  return (
    <div className="fixed inset-0 z-[130] flex items-center justify-center bg-black/50 p-4">
      <section
        ref={dialog}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="mcp-approval-title"
        className="flex max-h-[85vh] w-full max-w-2xl flex-col gap-3 rounded-2xl border border-border bg-chats p-5 text-primary shadow-2xl"
        onKeyDown={(event) => {
          if (event.key !== 'Tab') return
          const items = Array.from(
            dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled), [tabindex="0"]') || [],
          )
          const first = items[0],
            last = items[items.length - 1]
          if (event.shiftKey && document.activeElement === first) {
            event.preventDefault()
            last?.focus()
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault()
            first?.focus()
          }
        }}
      >
        <h2 id="mcp-approval-title" className="font-bold">
          {deleting ? t('mcp.approveDeleteTitle') : t('mcp.approveSendTitle')}
        </h2>
        <p className="text-sm">
          {t('mcp.requestingClient')}: <strong>{current.client}</strong>
        </p>
        <p className="break-all text-sm">
          {t('settings.account.account')}: {accounts.find((a) => a.id === current.account)?.email || current.account}
        </p>
        <p className="text-xs text-secondary">
          {t('accounts.certificate.expires')}: {new Date(current.expires_at).toLocaleTimeString()}
        </p>
        <div tabIndex={0} className="min-h-0 overflow-y-auto rounded-lg border border-border p-3 text-sm select-text">
          {deleting ? (
            <>
              <p className="mb-3 font-semibold text-rose-500">{t('mcp.irreversible')}</p>
              <p>
                {t('mcp.folder')}: {selection?.folder}
              </p>
              <p>
                {t('mcp.messageCount')}: {selection?.messages.length ?? 0}
              </p>
              <ul className="mt-3 space-y-3">
                {selection?.messages.map((message) => (
                  <li key={message.uid}>
                    <p className="break-words font-semibold">{message.subject || t('threads.noSubject')}</p>
                    <p className="break-all text-secondary">
                      {message.from} · {new Date(message.date * 1000).toLocaleString()} · UID {message.uid}
                    </p>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <>
              <p className="break-all">To: {current.preview.to}</p>
              {current.preview.cc && <p className="break-all">Cc: {current.preview.cc}</p>}
              {current.preview.bcc && <p className="break-all">Bcc: {current.preview.bcc}</p>}
              <p className="my-3 break-words font-semibold">{current.preview.subject || t('threads.noSubject')}</p>
              <pre className="whitespace-pre-wrap break-words font-sans">{current.preview.body}</pre>
              {current.preview.in_reply_to && (
                <p className="mt-3 break-all text-xs text-secondary">In-Reply-To: {current.preview.in_reply_to}</p>
              )}
            </>
          )}
        </div>
        <div className="flex justify-end gap-2">
          <button
            ref={rejectButton}
            disabled={busy}
            onClick={() => void resolve(false)}
            className="rounded-lg bg-hover px-4 py-2 text-sm disabled:opacity-50"
          >
            {t('mcp.reject')}
          </button>
          <button
            disabled={busy}
            onClick={() => void resolve(true)}
            className="rounded-lg bg-accent px-4 py-2 text-sm text-white disabled:opacity-50"
          >
            {busy ? t('settings.storage.working') : deleting ? t('mcp.approveDelete') : t('mcp.approveSend')}
          </button>
        </div>
      </section>
    </div>
  )
}
