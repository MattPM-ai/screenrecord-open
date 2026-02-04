/**
 * ============================================================================
 * WEEKLY REPORTS OPT-OUT API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Forwards opt-out requests to the backend API
 * 
 * DESCRIPTION:
 * This API route forwards weekly reports email opt-out requests to the backend API.
 * It validates the request and forwards it with authentication.
 * 
 * DEPENDENCIES:
 * - External: Backend API at NEXT_PUBLIC_BACKEND_URL
 * 
 * INPUTS:
 * - POST body: { accountId, orgId }
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
    const { accountId, orgId } = body

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

    // Forward request to backend API
    const response = await fetch(`${BACKEND_URL}/api/reports/weekly/opt-out`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        // No Authorization header needed for local version
      },
      body: JSON.stringify({
        accountId,
        orgId,
      }),
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('Backend API error:', errorText)
      return NextResponse.json(
        { error: 'Failed to opt out of weekly reports', details: errorText },
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

