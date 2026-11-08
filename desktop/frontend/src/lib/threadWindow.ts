import { invoke } from './bridge'
import { t } from './i18n'
import { showToast } from '../states/ui'
import { closeMessageTab } from '../states/compose'
import { compose$ } from '../states/composeState'
import { findLocalThread } from '../states/mail'
import { ensureThreadData } from '../states/threadData'
import { snapshotThreadView } from '../states/threadView'
import type { Message } from '../types'
export { threadWindowSession } from './windowSession'

export { externalThreadDrafts$ } from './windowSession'

async function requestThreadWindow(
  threadId: string,
  reference?: Message,
  scope?: string,
  moveTab = false,
): Promise<string> {
  const tab = compose$.tabs.peek().find((tab) => tab.id === compose$.activeTab.peek())
  const key = scope ?? (tab?.kind === 'thread' && tab.threadId === threadId ? `${tab.id}:${threadId}` : threadId)
  const messages = ensureThreadData(threadId).messages.peek()
  const thread = reference ?? findLocalThread(threadId) ?? messages[0]
  return invoke<string>('threadWindow.open', {
    thread_id: threadId,
    move_tab: moveTab,
    thread,
    subject: thread?.subject,
    view: snapshotThreadView(
      key,
      messages.map((message) => message.id),
    ),
  })
}

export async function openThreadWindow(threadId: string, reference?: Message, scope?: string) {
  try {
    await requestThreadWindow(threadId, reference, scope)
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenNotificationThread'), 'error')
  }
}

// Keep the source until the destination has mounted its conversation. A native
// window can open successfully even when account or message loading fails.
const movingTabs = new Set<string>()
export async function moveThreadTabToWindow(tabId: string) {
  const tab = compose$.tabs.peek().find((tab) => tab.id === tabId)
  if (tab?.kind !== 'thread' || movingTabs.has(tabId)) return
  movingTabs.add(tabId)
  try {
    const session = await requestThreadWindow(tab.threadId, undefined, `${tab.id}:${tab.threadId}`, true)
    while (!(await invoke<boolean>('threadWindow.isReady', { session }))) {
      if (compose$.tabs.peek().find((item) => item.id === tabId) !== tab) return
      await new Promise((resolve) => setTimeout(resolve, 250))
    }
    // A closed/reopened tab with the same id is a new source, not this move.
    if (compose$.tabs.peek().find((item) => item.id === tabId) === tab) await closeMessageTab(tabId)
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenNotificationThread'), 'error')
  } finally {
    movingTabs.delete(tabId)
  }
}
