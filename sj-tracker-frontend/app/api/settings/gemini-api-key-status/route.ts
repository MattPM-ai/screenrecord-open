/**
 * GET /api/settings/gemini-api-key-status
 * Proxies to report backend. Returns { set: boolean } for shared Gemini key (same as desktop app).
 */

import { NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function GET() {
  try {
    const response = await fetch(`${BACKEND_URL}/api/settings/gemini-api-key-status`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
    })
    if (!response.ok) {
      const text = await response.text().catch(() => '')
      return NextResponse.json({ set: false }, { status: 200 })
    }
    const data = await response.json()
    return NextResponse.json(data)
  } catch {
    return NextResponse.json({ set: false }, { status: 200 })
  }
}
