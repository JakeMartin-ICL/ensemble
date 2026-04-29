import { useEffect, useState } from 'react'
import './VersionFooter.css'

interface VersionInfo {
  app: string
  shortSha: string
  commitSha: string
  branch: string
  builtAt: string
  vercelEnv: string | null
  vercelUrl: string | null
}

const buildInfo = __BUILD_INFO__

function sameVersion(latest: VersionInfo | null) {
  return !latest || latest.commitSha === buildInfo.commitSha
}

export default function VersionFooter() {
  const [latest, setLatest] = useState<VersionInfo | null>(null)

  useEffect(() => {
    const controller = new AbortController()
    void fetch(`/version.json?t=${String(Date.now())}`, {
      cache: 'no-store',
      signal: controller.signal,
    })
      .then((response) => {
        if (!response.ok) throw new Error(`Version check failed: ${String(response.status)}`)
        return response.json() as Promise<VersionInfo>
      })
      .then(setLatest)
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === 'AbortError') return
        console.warn('Unable to check latest app version', error)
      })

    return () => { controller.abort() }
  }, [])

  const updateAvailable = !sameVersion(latest)
  const builtAt = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(buildInfo.builtAt))

  return (
    <footer className="versionFooter" aria-label="App version">
      <span>v {buildInfo.shortSha}</span>
      <span>{builtAt}</span>
      {updateAvailable && (
        <button className="versionFooterUpdate" type="button" onClick={() => { window.location.reload() }}>
          Update available
        </button>
      )}
    </footer>
  )
}
