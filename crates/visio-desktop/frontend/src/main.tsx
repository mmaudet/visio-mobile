import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { ConfirmProvider } from './components/ui/ConfirmProvider'
// Legacy global styles first — the new design system tokens below must win
// for any :root / [data-theme] variables that overlap.
import './App.css'
import './styles/tokens.css'
import './styles/primitives.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ConfirmProvider>
      <App />
    </ConfirmProvider>
  </StrictMode>
)
