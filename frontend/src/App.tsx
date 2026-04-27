import { BrowserRouter, Routes, Route } from 'react-router-dom'
import Home from './pages/Home'
import AuthCallback from './pages/AuthCallback'
import WeaveHome from './pages/car/Setup'
import WeaveSession from './pages/car/Session'

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/auth/callback" element={<AuthCallback />} />
        <Route path="/car" element={<WeaveHome />} />
        <Route path="/car/session" element={<WeaveSession />} />
      </Routes>
    </BrowserRouter>
  )
}
