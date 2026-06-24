import { useMemo, useState } from 'react'
import { CheckCheck, Copy } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'

import { parseAddressList, type AddressItem } from './messageHelpers'

export function AddressPill({ name, original }: AddressItem) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(false)

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation()
    navigator.clipboard.writeText(original)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="group relative inline-flex max-w-[180px] min-w-0 items-center gap-1 rounded-md border border-accent/10 bg-accent/5 px-2 py-1 text-[10px] font-normal text-primary outline-none transition-all duration-150 hover:border-accent/25 hover:bg-accent/10 active:scale-[0.98] dark:bg-accent/15 dark:hover:bg-accent/25 cursor-pointer select-none"
      aria-label={t('chat.copyFullAddress')}
      title={original}
    >
      <span className="min-w-0 truncate font-semibold">{name}</span>

      <span className="opacity-0 group-hover:opacity-75 transition-opacity duration-150 shrink-0 text-accent">
        {copied ? <CheckCheck size={10} className="text-emerald-500" /> : <Copy size={10} />}
      </span>
    </button>
  )
}

export function AddressRow({ label, rawList }: { label: string; rawList: string }) {
  const { t } = useTranslation()
  const items = useMemo(() => parseAddressList(rawList), [rawList])
  const [expanded, setExpanded] = useState(false)

  if (items.length === 0) return null

  const threshold = 6
  const showMore = items.length > threshold
  const visibleItems = expanded || !showMore ? items : items.slice(0, threshold)

  return (
    <div className="grid grid-cols-[38px_minmax(0,1fr)] items-start gap-2 text-[10.5px]">
      <span className="mt-1.5 shrink-0 text-[9px] font-semibold uppercase tracking-wider text-secondary/70">
        {label}:
      </span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
        {visibleItems.map((item, idx) => (
          <AddressPill key={idx} {...item} />
        ))}
        {showMore && (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="inline-flex min-w-0 items-center justify-center rounded-md bg-accent/10 px-2 py-1 text-[9px] font-bold text-accent outline-none transition-colors duration-150 hover:bg-accent/20 dark:bg-accent/25 dark:hover:bg-accent/35 cursor-pointer select-none"
          >
            {expanded ? t('common.showLess') : t('common.moreCount', { count: items.length - threshold })}
          </button>
        )}
      </div>
    </div>
  )
}
