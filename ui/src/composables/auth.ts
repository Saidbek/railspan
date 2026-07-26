import { computed, ref } from 'vue'

const TOKEN_KEY = 'railspan_ui_token'
const token = ref(sessionStorage.getItem(TOKEN_KEY) || '')

export function getToken(): string {
  return token.value
}

export function setToken(t: string): void {
  token.value = t
  if (t) sessionStorage.setItem(TOKEN_KEY, t)
  else sessionStorage.removeItem(TOKEN_KEY)
}

export function promptToken(msg?: string): string {
  const t = window.prompt(msg || 'UI token (RAILSPAN_UI_TOKEN / API key)', getToken())
  if (t === null) return getToken()
  setToken(t.trim())
  return getToken()
}

export function useAuth() {
  const hasToken = computed(() => !!token.value)
  return {
    token,
    hasToken,
    setToken,
    getToken,
    promptToken,
  }
}
