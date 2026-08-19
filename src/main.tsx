import { getCurrentWindow } from '@tauri-apps/api/window'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './tailwind.css'
import './styles.scss'
import App from './App.tsx'
import { DesktopLyricsOverlay } from './components/DesktopLyricsOverlay.tsx'

const windowView = '__TAURI_INTERNALS__' in window && getCurrentWindow().label === 'desktop-lyrics'
  ? 'desktop-lyrics'
  : new URLSearchParams(window.location.search).get('window')
if (windowView) document.documentElement.dataset.window = windowView

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {windowView === 'desktop-lyrics' ? <DesktopLyricsOverlay /> : <App />}
  </StrictMode>,
)
