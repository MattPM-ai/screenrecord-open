/**
 * GET /api/settings/gemini-api-key - returns shared Gemini key for prefill/chat
 * POST /api/settings/gemini-api-key - saves key to shared file (sync with desktop app)
 */

import { NextRequest, NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function GET() {
  try {
    const response = await fetch(`${BACKEND_URL}/api/settings/gemini-api-key`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
    })
    if (!response.ok) {
      if (response.status === 404) {
        return NextResponse.json({ error: 'Gemini API key not set' }, { status: 404 })
      }
      const text = await response.text().catch(() => '')
      return NextResponse.json({ error: text || 'Failed to get key' }, { status: response.status })
    }
    const data = await response.json()
    return NextResponse.json(data)
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to connect to backend service' },
      { status: 503 }
    )
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json()
    const key = typeof body?.key === 'string' ? body.key.trim() : ''
    if (!key) {
      return NextResponse.json({ error: 'key is required' }, { status: 400 })
    }
    const response = await fetch(`${BACKEND_URL}/api/settings/gemini-api-key`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ key }),
    })
    const data = await response.json().catch(() => ({}))
    if (!response.ok) {
      return NextResponse.json(
        { error: data.error || 'Failed to save key' },
        { status: response.status }
      )
    }
    return NextResponse.json(data)
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to connect to backend service' },
      { status: 503 }
    )
  }
}
