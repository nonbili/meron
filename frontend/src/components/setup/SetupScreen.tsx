import { AccountDialog } from '../dialog/AccountDialog'

export function SetupScreen() {
  return (
    <main className="flex h-full w-full justify-center overflow-y-auto bg-app px-6 py-10 text-primary">
      <AccountDialog variant="setup" />
    </main>
  )
}
