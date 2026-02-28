import React from 'react'
import ReactDOM from 'react-dom/client'
import { HeroUIProvider } from '@heroui/react'
import App from './App.tsx'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <HeroUIProvider>
      <main className="dark text-foreground bg-background min-h-screen">
        <App />
      </main>
    </HeroUIProvider>
  </React.StrictMode>,
)
