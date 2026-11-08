import { invoke } from './bridge'
import { t } from './i18n'
import { showToast, ui$ } from '../states/ui'
import { closeMessageTab, finishClosingMessageTab, openReplyInFullEditor } from '../states/compose'
import { getComposeSession, transferComposeSession } from '../states/composeSessions'
import { closeCurrentConversation, kanban$ } from '../states/kanban'
import { isQuickReplyBlank } from '../states/quickReply'
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

function currentThreadId() {
  return kanban$.activeBoardId.peek() ? kanban$.paneThreadId.peek() : compose$.conversationThread.peek()
}

async function moveCurrentToWindow() {
  const threadId = currentThreadId()
  if (!threadId || movingTabs.has('')) return
  movingTabs.add('')
  try {
    const session = await requestThreadWindow(threadId, undefined, threadId, true)
    while (!(await invoke<boolean>('threadWindow.isReady', { session }))) {
      if (currentThreadId() !== threadId) return
      await new Promise((resolve) => setTimeout(resolve, 250))
    }
    if (currentThreadId() !== threadId) return
    // A reader window has no inline reply box. Keep the live reply in a full
    // editor before retiring Current, without waiting for a server save.
    if (ui$.selectedThread.peek() === threadId && !isQuickReplyBlank()) {
      const previousTab = compose$.activeTab.peek()
      openReplyInFullEditor()
      if (previousTab) compose$.activeTab.set(previousTab)
      if (!isQuickReplyBlank()) throw new Error(t('compose.toast.couldNotOpenNotificationThread'))
    }
    closeCurrentConversation()
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenNotificationThread'), 'error')
  } finally {
    movingTabs.delete('')
  }
}

async function moveComposerToWindow(tabId: string) {
  const session = getComposeSession(tabId)
  if (session.savesStopped || session.closing) return
  await transferComposeSession(tabId, async () => {
    session.savesStopped = true
    session.savingToClose = true
    session.setClosingState?.(true)
    let destination = ''
    let moved = false
    let canResume = true
    try {
      await session.saveChain
      const fetches = [...session.attachmentFetches]
      await Promise.all([...session.pendingEdits, ...fetches])
      const fetched = (await Promise.all(fetches)).flat()
      const tab = compose$.tabs.peek().find((tab) => tab.id === tabId)
      if (!tab?.compose) return
      // The native bridge holds the complete draft in memory, including binary
      // attachments. localStorage's recovery buffer intentionally omits them.
      destination = await invoke<string>('threadWindow.open', {
        compose_tab: JSON.parse(
          JSON.stringify({
            ...tab,
            compose: {
              ...tab.compose,
              // The fetch's caller may not have appended its result yet. Carry
              // the bytes directly, as the save-on-close path does.
              attachments: [
                ...new Map([...tab.compose.attachments, ...fetched].map((file) => [file.id, file])).values(),
              ],
            },
          }),
        ),
        subject: tab.compose.subject || tab.subject,
      })
      const deadline = Date.now() + 60_000
      while (!(await invoke<boolean>('threadWindow.isReady', { session: destination }))) {
        if (Date.now() >= deadline) throw new Error(t('compose.toast.couldNotOpenNotificationThread'))
        await new Promise((resolve) => setTimeout(resolve, 250))
      }
      await invoke('threadWindow.activateComposer', { session: destination })
      moved = true
      finishClosingMessageTab(tabId)
    } catch (error) {
      if (destination) {
        try {
          // An activation whose reply was lost already owns the draft. A
          // staged editor can instead be destroyed before resuming the source.
          moved = !(await invoke<boolean>('threadWindow.abortComposer', { session: destination }))
          if (moved) finishClosingMessageTab(tabId)
        } catch {
          // Ownership could not be established. Keep the frozen source buffer
          // intact rather than allow two editors to autosave the same draft.
          canResume = false
          showToast(t('compose.toast.couldNotOpenNotificationThread'), 'error')
          return
        }
      }
      if (!moved)
        showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenNotificationThread'), 'error')
    } finally {
      if (!moved && canResume) {
        session.savesStopped = false
        session.savingToClose = false
        session.setClosingState?.(false)
      }
    }
  })
}

export async function moveTabToWindow(tabId: string) {
  if (!tabId) return moveCurrentToWindow()
  const tab = compose$.tabs.peek().find((tab) => tab.id === tabId)
  if (tab?.kind === 'thread') return moveThreadTabToWindow(tabId)
  if (tab?.kind === 'compose') return moveComposerToWindow(tabId)
}
