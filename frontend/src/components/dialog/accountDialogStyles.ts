// The dialog renders in two skins: the standalone first-run "setup" screen and
// the in-app "add account" modal. These resolve the per-skin class strings once
// so the form sections don't each re-derive them.
export type DialogClasses = {
  panelClass: string
  scrollClass: string
  inputClass: string | undefined
  fieldLabelClass: string | undefined
  serverGridClass: string
}

export function dialogClasses(isSetup: boolean): DialogClasses {
  return {
    panelClass: isSetup
      ? 'm-auto w-full max-w-[640px] rounded-3xl border border-border/80 bg-chats p-10 text-primary shadow-[0_1px_2px_rgba(15,23,42,0.04),0_24px_64px_rgba(15,23,42,0.08)] animate-slide-up flex flex-col gap-7 max-[640px]:rounded-2xl max-[640px]:p-6 max-[640px]:gap-6'
      : 'bg-chats border border-border text-primary max-w-md w-full rounded-3xl p-6 shadow-2xl animate-slide-up flex flex-col gap-4.5',
    scrollClass: isSetup
      ? 'flex flex-col gap-5 overflow-y-auto px-0.5 py-0.5'
      : 'flex flex-col gap-3.5 max-h-[300px] overflow-y-auto px-1 py-0.5',
    inputClass: isSetup
      ? 'w-full rounded-xl border border-border bg-chats px-4 py-3.5 text-[15px] text-primary outline-none transition-all placeholder:text-secondary focus:border-accent focus:ring-3 focus:ring-accent/10'
      : undefined,
    fieldLabelClass: isSetup ? 'text-[12.5px] font-semibold text-secondary' : undefined,
    serverGridClass: isSetup
      ? 'grid grid-cols-[minmax(0,1fr)_120px] gap-3 max-[520px]:grid-cols-1'
      : 'grid grid-cols-[1fr_80px] gap-2',
  }
}
