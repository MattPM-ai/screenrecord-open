/**
 * Shared Gemini API key client.
 * Uses the report backend's settings API so desktop app Settings and report frontend stay in sync (same file).
 */

const API_BASE = ''

export async function getGeminiKeyStatus(): Promise<{ set: boolean }> {
  const res = await fetch(`${API_BASE}/api/settings/gemini-api-key-status`)
  const data = await res.json().catch(() => ({ set: false }))
  return { set: !!data?.set }
}

export async function getGeminiKey(): Promise<string | null> {
  const res = await fetch(`${API_BASE}/api/settings/gemini-api-key`)
  if (!res.ok) return null
  const data = await res.json().catch(() => null)
  return data?.key && typeof data.key === 'string' ? data.key.trim() : null
}

export async function saveGeminiKey(key: string): Promise<void> {
  const res = await fetch(`${API_BASE}/api/settings/gemini-api-key`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ key: key.trim() }),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err?.error || 'Failed to save Gemini API key')
  }
}
