import { Component, type ErrorInfo, type ReactNode } from 'react'

interface Props {
  children: ReactNode
  // Optional custom fallback. Receives the caught error and a reset callback that
  // clears the boundary so the subtree can try to re-render.
  fallback?: (error: Error, reset: () => void) => ReactNode
  // Short label identifying the wrapped region, used in the default fallback and logs.
  label?: string
}

interface State {
  error: Error | null
}

// Catches render/lifecycle errors in its subtree so one crashing region doesn't blank
// the whole window. Kept dependency-free (no i18n/state hooks) since those may be the
// very thing that broke. React only invokes error boundaries for errors thrown during
// rendering — event-handler errors still need their own try/catch.
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    const where = this.props.label ? ` in ${this.props.label}` : ''
    console.error(`ErrorBoundary caught an error${where}:`, error, info.componentStack)
  }

  private reset = () => this.setState({ error: null })

  render() {
    const { error } = this.state
    if (!error) return this.props.children

    if (this.props.fallback) return this.props.fallback(error, this.reset)

    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-3 bg-app p-6 text-center text-primary">
        <p className="text-sm font-medium">Something went wrong{this.props.label ? ` in ${this.props.label}` : ''}.</p>
        <p className="max-w-md text-xs text-secondary">{error.message}</p>
        <button
          type="button"
          onClick={() => window.location.reload()}
          className="rounded-full border border-border/40 bg-active px-4 py-1.5 text-xs font-medium text-secondary hover:bg-active cursor-pointer"
        >
          Reload
        </button>
      </div>
    )
  }
}
