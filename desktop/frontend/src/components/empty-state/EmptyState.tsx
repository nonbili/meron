import { AtSign } from 'lucide-react'

export function EmptyState({ title, text }: { title: string; text: string }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-8 text-center animate-fade-in select-none">
      <div className="max-w-xs flex flex-col items-center">
        <div className="relative flex h-16 w-16 items-center justify-center rounded-2xl bg-raised border border-accent/10 text-accent shadow-sm">
          <div className="absolute inset-0 rounded-2xl bg-accent/5 blur-lg" />
          <AtSign size={24} strokeWidth={1.8} className="relative z-10" />
        </div>
        <h3 className="mt-5 font-bold text-sm text-primary tracking-tight">{title}</h3>
        <p className="mt-2 text-xs text-secondary leading-relaxed font-normal px-2">{text}</p>
      </div>
    </div>
  )
}
