const _apiUrl = import.meta.env.VITE_API_URL as string | undefined
if (!_apiUrl) throw new Error('VITE_API_URL is not set')
const BASE_URL: string = _apiUrl

export async function post<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`API error: ${res.status.toString()}`)
  return res.json() as Promise<T>
}

export async function get<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, { headers })
  if (!res.ok) throw new Error(`API error: ${res.status.toString()}`)
  return res.json() as Promise<T>
}
