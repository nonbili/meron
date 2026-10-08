import { StrictMode, useEffect, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import './lib/i18n'
import App from './App'
import { ErrorBoundary } from './components/ErrorBoundary'
import { windowChromeReady } from './lib/windowChrome'
import { invoke } from './lib/bridge'
import { usesNativeScrollbars } from './lib/nativeScrollbars'
import './index.css'

document.documentElement.classList.toggle('native-scrollbars', usesNativeScrollbars(navigator.userAgent))

function StartupWindow({ children }: { children: ReactNode }) {
  useEffect(() => {
    let revealed = false
    let frame = 0
    const reveal = () => {
      if (revealed) return
      revealed = true
      void invoke('window.ready').catch(console.error)
    }
    // The second frame runs after React's first commit has been painted. Some
    // hidden webviews suspend animation frames, so a timer also lets those
    // platforms show the already committed UI.
    frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(reveal)
    })
    const timer = setTimeout(reveal, 200)
    return () => {
      cancelAnimationFrame(frame)
      clearTimeout(timer)
    }
  }, [])
  return children
}

// Rendering before the title bar is known would draw the system-title-bar
// layout first and then move everything when Meron's own title bar arrives.
void windowChromeReady().then(() => {
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <StartupWindow>
        <ErrorBoundary>
          <App />
        </ErrorBoundary>
      </StartupWindow>
    </StrictMode>,
  )
})
