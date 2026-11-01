import { useRef, useState, type ReactNode } from 'react'
import { useValue } from '@legendapp/state/react'
import { ChevronDown } from 'lucide-react'
import { accounts$, isSendableAccount } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { ensureAccountFolders } from '../../states/mailFolders'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { FolderMenuTree } from '../menu/FolderMenuTree'
import { MenuItem } from '../menu/MenuItem'
import { AccountFolderItem, accountLabel, flyoutClass, FolderListStatus } from '../threads/ThreadMoveCopyItems'
import type { Folder } from '../../types'

export type RuleFolderTarget = { accountId: string; folderId: string }

/** How a rule's target reads: its account too when there is more than one. */
export function useRuleTargetLabel() {
  const accounts = useValue(accounts$).filter(isSendableAccount)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  return ({ accountId, folderId }: RuleFolderTarget) => {
    const folder = foldersByAccount[accountId]?.find((item) => item.id === folderId)
    const name = folder?.name ?? folderId
    const account = accounts.find((item) => item.id === accountId)
    return accounts.length > 1 && account ? `${accountLabel(account)} / ${name}` : name
  }
}

/**
 * Picks where a rule sends mail, with the thread menu's Move to and Copy to lists:
 * `options` first, then one row per account opening its folders. Rules only run
 * on synced mail accounts, so those are the accounts offered.
 */
export function RuleFolderPicker({
  label,
  value,
  options,
  onPick,
}: {
  label: string
  /** What the button shows. */
  value: string
  options: { label: string; onPick: () => void }[]
  onPick: (target: RuleFolderTarget) => void
}) {
  const accounts = useValue(accounts$).filter(isSendableAccount)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null)
  const [openAccountId, setOpenAccountId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const buttonRef = useRef<HTMLButtonElement | null>(null)

  const close = () => {
    setPosition(null)
    setOpenAccountId(null)
  }

  const open = () => {
    const rect = buttonRef.current?.getBoundingClientRect()
    setPosition({ x: rect?.left ?? 0, y: (rect?.bottom ?? 0) + 4 })
    setLoading(true)
    void Promise.all(
      accounts.map((account) =>
        ensureAccountFolders(account.id, { refreshIfBootstrapOnly: true, waitForRefresh: true }),
      ),
    ).finally(() => setLoading(false))
  }

  const pick = (accountId: string, folder: Folder) => {
    close()
    onPick({ accountId, folderId: folder.id })
  }

  const inlineAccount = accounts.length === 1 ? accounts[0] : undefined
  const menu: ReactNode = position && (
    <FloatingContextMenu
      x={position.x}
      y={position.y}
      onClose={close}
      overlay
      overlayClassName="fixed inset-0 z-[61]"
      className={`${flyoutClass} min-w-[230px]`}
      dataAttribute="data-thread-context-menu"
    >
      {options.map((option) => (
        <MenuItem
          key={option.label}
          label={option.label}
          onClick={() => {
            close()
            option.onPick()
          }}
        />
      ))}
      {accounts.length > 0 && <div className="my-1 border-t border-border" />}
      {inlineAccount ? (
        <>
          {(foldersByAccount[inlineAccount.id] ?? []).length === 0 && <FolderListStatus loading={loading} />}
          <FolderMenuTree
            folders={foldersByAccount[inlineAccount.id] ?? []}
            excludedFolderIds={[]}
            onPick={(folder) => pick(inlineAccount.id, folder)}
          />
        </>
      ) : (
        accounts.map((account) => (
          <AccountFolderItem
            key={account.id}
            account={account}
            folders={foldersByAccount[account.id] ?? []}
            excludedFolderIds={[]}
            loading={loading}
            open={openAccountId === account.id}
            setOpen={(next) =>
              setOpenAccountId((current) => (next ? account.id : current === account.id ? null : current))
            }
            onPick={(folder) => pick(account.id, folder)}
          />
        ))
      )}
    </FloatingContextMenu>
  )

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        aria-label={label}
        aria-expanded={position !== null}
        onClick={() => (position ? close() : open())}
        className="flex w-44 cursor-pointer items-center justify-between gap-2 rounded-xl bg-app py-1.5 pr-2.5 pl-3 text-left text-xs font-semibold text-primary ring-1 ring-border"
      >
        <span className="min-w-0 truncate" title={value}>
          {value}
        </span>
        <ChevronDown size={12} className="shrink-0 text-secondary" />
      </button>
      {menu}
    </>
  )
}
