import { invoke } from './bridge'
import { t } from './i18n'
import { showToast } from '../states/ui'
import { compose$ } from '../states/composeState'
import { findLocalThread } from '../states/mail'
import { ensureThreadData } from '../states/threadData'
import { snapshotThreadView } from '../states/threadView'
import type { Message } from '../types'
export { threadWindowSession } from './windowSession'

export { externalThreadDrafts$ } from './windowSession'

export async function openThreadWindow(threadId: string, reference?: Message, scope?: string) {
  try {
    const tab = compose$.tabs.peek().find((tab) => tab.id === compose$.activeTab.peek())
    const key = scope ?? (tab?.kind === 'thread' && tab.threadId === threadId ? `${tab.id}:${threadId}` : threadId)
    const messages = ensureThreadData(threadId).messages.peek()
    const thread = reference ?? findLocalThread(threadId) ?? messages[0]
    await invoke('threadWindow.open', {
      thread_id: threadId,
      thread,
      subject: thread?.subject,
      view: snapshotThreadView(
        key,
        messages.map((message) => message.id),
      ),
    })
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenNotificationThread'), 'error')
  }
}
