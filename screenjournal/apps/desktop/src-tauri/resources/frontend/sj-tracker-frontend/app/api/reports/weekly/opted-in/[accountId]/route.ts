/**
 * ============================================================================
 * GET OPTED-IN ACCOUNTS API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Fetches opted-in weekly report accounts for a given account ID
 * 
 * DESCRIPTION:
 * This API route forwards requests to the backend API to get all organizations
 * that have opted in to weekly report emails for the given account.
 * 
 * DEPENDENCIES:
 * - External: Backend API at NEXT_PUBLIC_BACKEND_URL
 * 
 * INPUTS:
 * - GET /api/reports/weekly/opted-in/:accountId
 * 
 * OUTPUTS:
 * - JSON: { accountId, accounts: [...] }
 * 
 * ============================================================================
 */

import { NextRequest, NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function GET(
  request: NextRequest,
  { params }: { params: { accountId: string } }
) {
  try {
    // No authentication required for open-source local version

    const { accountId } = params

    // Validate accountId
    if (!accountId || isNaN(Number(accountId))) {
      return NextResponse.json(
        { error: 'Invalid accountId format' },
        { status: 400 }
      )
    }

    // Forward request to backend API
    const response = await fetch(`${BACKEND_URL}/api/reports/weekly/opted-in/${accountId}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        // No Authorization header needed for local version
      },
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('Backend API error:', errorText)
      return NextResponse.json(
        { error: 'Failed to fetch opted-in accounts', details: errorText },
        { status: response.status }
      )
    }

    const data = await response.json()
    return NextResponse.json(data)
  } catch (error) {
    console.error('API route error:', error)
    
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



