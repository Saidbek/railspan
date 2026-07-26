import { getToken, promptToken } from '@/composables/auth'

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

function authHeaders(extra: HeadersInit = {}): Headers {
  const h = new Headers(extra)
  const t = getToken()
  if (t) h.set('Authorization', `Bearer ${t}`)
  return h
}

export async function api<T = unknown>(path: string, opts: RequestInit = {}): Promise<T> {
  const headers = authHeaders(opts.headers || {})
  let res = await fetch(path, { ...opts, headers })

  if (res.status === 401) {
    promptToken('Unauthorized — enter UI token')
    const retryHeaders = authHeaders(opts.headers || {})
    res = await fetch(path, { ...opts, headers: retryHeaders })
  }

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText)
    throw new ApiError(res.status, text || res.statusText)
  }

  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}
