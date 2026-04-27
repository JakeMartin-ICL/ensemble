const _apiUrl = import.meta.env.VITE_API_URL as string | undefined
if (!_apiUrl) throw new Error('VITE_API_URL is not set')
const BASE_URL: string = _apiUrl

async function errorMessage(res: Response): Promise<string> {
  const fallback = `API error: ${res.status.toString()}`
  const contentType = res.headers.get('content-type') ?? ''

  if (contentType.includes('application/json')) {
    const body = await res.json() as { error?: unknown }
    return typeof body.error === 'string' ? body.error : fallback
  }

  const body = await res.text()
  return body ? `${fallback}: ${body}` : fallback
}

export async function post<T>(
  path: string,
  body: unknown,
  headers?: Record<string, string>,
): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...headers },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(await errorMessage(res))
  return res.json() as Promise<T>
}

export async function get<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, { headers })
  if (!res.ok) throw new Error(await errorMessage(res))
  return res.json() as Promise<T>
}
