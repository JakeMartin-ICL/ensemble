import { execFileSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

function git(args: string[], fallback = '') {
  try {
    return execFileSync('git', args, { encoding: 'utf8' }).trim()
  } catch (error: unknown) {
    if (
      error
      && typeof error === 'object'
      && 'stdout' in error
      && typeof error.stdout === 'string'
    ) {
      const output = error.stdout.trim()
      if (output) return output
    }
    return fallback
  }
}

const commitSha = process.env.VERCEL_GIT_COMMIT_SHA || git(['rev-parse', 'HEAD'], 'unknown')
const buildInfo = {
  app: 'ensemble',
  commitSha,
  shortSha: commitSha === 'unknown' ? 'unknown' : commitSha.slice(0, 7),
  branch: process.env.VERCEL_GIT_COMMIT_REF || git(['branch', '--show-current'], 'unknown'),
  builtAt: new Date().toISOString(),
  vercelEnv: process.env.VERCEL_ENV || null,
  vercelUrl: process.env.VERCEL_URL || null,
}

mkdirSync('public', { recursive: true })
writeFileSync(join('public', 'version.json'), `${JSON.stringify(buildInfo, null, 2)}\n`)

export default defineConfig({
  server: {
    host: '127.0.0.1',
  },
  define: {
    __BUILD_INFO__: JSON.stringify(buildInfo),
  },
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      workbox: {
        globIgnores: ['**/version.json'],
        runtimeCaching: [
          {
            urlPattern: ({ url }) => url.pathname === '/version.json',
            handler: 'NetworkOnly',
          },
        ],
      },
      manifest: {
        name: 'Spotify Party',
        short_name: 'SpotifyParty',
        description: 'Better multi-listener Spotify experiences',
        theme_color: '#000000',
        background_color: '#000000',
        display: 'standalone',
        icons: [
          { src: '/icon-192.png', sizes: '192x192', type: 'image/png' },
          { src: '/icon-512.png', sizes: '512x512', type: 'image/png' },
        ],
      },
    }),
  ],
})
