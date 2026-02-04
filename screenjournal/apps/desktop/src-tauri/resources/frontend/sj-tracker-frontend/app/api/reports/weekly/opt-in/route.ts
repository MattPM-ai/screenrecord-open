/**
 * ============================================================================
 * WEEKLY REPORTS OPT-IN API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Forwards opt-in requests to the backend API
 * 
 * DESCRIPTION:
 * This API route forwards weekly reports email opt-in requests to the backend API.
 * It validates the request and forwards it with authentication.
 * 
 * DEPENDENCIES:
 * - External: Backend API at NEXT_PUBLIC_BACKEND_URL
 * 
 * INPUTS:
 * - POST body: { accountId, orgId, orgName, email, users, nextTriggerTime? }
 * 
 * OUTPUTS:
 * - JSON: { accountId, message, orgId }
 * 
 * ============================================================================
 */

import { NextRequest, NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function POST(request: NextRequest) {
  try {
    // No authentication required for open-source local version

    const body = await request.json()
    const { accountId, orgId, orgName, email, users, nextTriggerTime } = body

    // Validate required fields
    if (!accountId || typeof accountId !== 'number') {
      return NextResponse.json(
        { error: 'accountId is required and must be a number' },
        { status: 400 }
      )
    }

    if (!orgId || typeof orgId !== 'number') {
      return NextResponse.json(
        { error: 'orgId is required and must be a number' },
        { status: 400 }
      )
    }

    if (!orgName || typeof orgName !== 'string' || !orgName.trim()) {
      return NextResponse.json(
        { error: 'orgName is required and must be a non-empty string' },
        { status: 400 }
      )
    }

    if (!email || typeof email !== 'string' || !email.trim()) {
      return NextResponse.json(
        { error: 'email is required and must be a valid email address' },
        { status: 400 }
      )
    }

    if (!users || !Array.isArray(users) || users.length === 0) {
      return NextResponse.json(
        { error: 'users is required and must be a non-empty array' },
        { status: 400 }
      )
    }

    // Validate users array
    for (let i = 0; i < users.length; i++) {
      const user = users[i]
      if (!user || typeof user !== 'object') {
        return NextResponse.json(
          { error: `users[${i}] must be an object` },
          { status: 400 }
        )
      }
      if (!user.name || typeof user.name !== 'string') {
        return NextResponse.json(
          { error: `users[${i}].name is required and must be a string` },
          { status: 400 }
        )
      }
      if (!user.id || typeof user.id !== 'number') {
        return NextResponse.json(
          { error: `users[${i}].id is required and must be a number` },
          { status: 400 }
        )
      }
    }

    // Validate nextTriggerTime if provided
    if (nextTriggerTime !== undefined && nextTriggerTime !== null) {
      if (typeof nextTriggerTime !== 'string' || nextTriggerTime.trim() === '') {
        return NextResponse.json(
          { error: 'nextTriggerTime must be a valid ISO 8601 datetime string' },
          { status: 400 }
        )
      }
      // Validate ISO 8601 format
      const iso8601Regex = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z?$/
      if (!iso8601Regex.test(nextTriggerTime)) {
        return NextResponse.json(
          { error: 'nextTriggerTime must be in ISO 8601 format (e.g., "2025-12-12T17:35:00Z")' },
          { status: 400 }
        )
      }
    }

    // Forward request to backend API
    const response = await fetch(`${BACKEND_URL}/api/reports/weekly/opt-in`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        // No Authorization header needed for local version
      },
      body: JSON.stringify({
        accountId,
        orgId,
        orgName: orgName.trim(),
        email: email.trim(),
        users,
        ...(nextTriggerTime ? { nextTriggerTime: nextTriggerTime.trim() } : {})
      }),
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('Backend API error:', errorText)
      return NextResponse.json(
        { error: 'Failed to opt in to weekly reports', details: errorText },
        { status: response.status }
      )
    }

    const data = await response.json()
    return NextResponse.json(data)
  } catch (error) {
    console.error('API route error:', error)
    
    if (error instanceof SyntaxError) {
      return NextResponse.json(
        { error: 'Invalid JSON in request body' },
        { status: 400 }
      )
    }

    if (error instanceof TypeError && error.message.includes('fetch')) {
      return NextResponse.json(
        { error: 'Failed to connect to backend service' },
        { status: 503 }
      )
    }

    return NextResponse.json(
      { error: 'Internal server error', details: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 }
    )
  }
}

