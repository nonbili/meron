import { beforeEach } from 'bun:test'
import { compose$ } from './composeState'
import { cancelQuickReplyDraftSave } from './quickReply'
import { accounts$ } from './accounts'
import { ui$ } from './ui'
import { mail$ } from './mail'

/** Resets compose and mail state before each quick-reply draft test, with a
 *  bridge stub that records every command it is sent. */
export function useQuickReplyDraftFixture(): { command: string; payload: unknown }[] {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    // A released send claim re-arms the autosave debounce; it must not fire into
    // the next test's state.
    cancelQuickReplyDraftSave()
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyDraftThreadId.set('')
    compose$.quickReplyFrom.set('')
    mail$.messages.set([])
    mail$.threads.set([])
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
    ui$.selectedThread.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([
      {
        id: 'acc-1',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
      },
    ])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
            if (command === 'mail.folderList') return { folders: [] }
            // Discarding a reply draft leaves the parent conversation in Inbox.
            if (command === 'mail.threadList') return { threads: mail$.threads.get() }
            return {}
          },
        },
      },
    }
  })
  return calls
}
