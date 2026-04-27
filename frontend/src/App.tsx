import { BrowserRouter, Routes, Route } from 'react-router-dom'
import Home from './pages/Home'
import AuthCallback from './pages/AuthCallback'
import WeaveHome from './pages/weave/Setup'
import WeaveSession from './pages/weave/Session'
import PartyHome from './pages/party/Setup'
import PartySession from './pages/party/Session'

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/auth/callback" element={<AuthCallback />} />
        <Route path="/weave" element={<WeaveHome />} />
        <Route path="/weave/session" element={<WeaveSession />} />
        <Route path="/party" element={<PartyHome />} />
        <Route path="/party/session" element={<PartySession />} />
      </Routes>
    </BrowserRouter>
  )
}
