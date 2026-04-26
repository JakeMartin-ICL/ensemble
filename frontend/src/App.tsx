import { BrowserRouter, Routes, Route } from 'react-router-dom'
import Home from './pages/Home'
import AuthCallback from './pages/AuthCallback'
import CarSetup from './pages/car/Setup'
import CarSession from './pages/car/Session'
// Party mode pages will be added here

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/auth/callback" element={<AuthCallback />} />
        <Route path="/car/setup" element={<CarSetup />} />
        <Route path="/car/session/:id" element={<CarSession />} />
        {/* Party mode routes — not yet implemented */}
      </Routes>
    </BrowserRouter>
  )
}
