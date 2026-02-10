/**
 * ============================================================================
 * TIMELINE API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Fetches timeline data from the backend API
 * 
 * DESCRIPTION:
 * This API route forwards timeline requests to the backend API
 * and returns timeline events for a specific user and date.
 * 
 * DEPENDENCIES:
 * - External: Backend API at http://localhost:8085
 * 
 * INPUTS:
 * - GET query params: userId, accountId, date (YYYY-MM-DD)
 * 
 * OUTPUTS:
 * - JSON: TimelineResponse with events array
 * 
 * ============================================================================
 */

import { NextRequest, NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function GET(request: NextRequest) {
  try {
    // Get query parameters
    const searchParams = request.nextUrl.searchParams
    const userId = searchParams.get('userId')
    const accountId = searchParams.get('accountId')
    const date = searchParams.get('date')

    // Validate required parameters
    if (!userId) {
      return NextResponse.json(
        { error: 'userId is required' },
        { status: 400 }
      )
    }

    if (!date) {
      return NextResponse.json(
        { error: 'date is required (YYYY-MM-DD format)' },
        { status: 400 }
      )
    }

    // Validate date format
    const dateRegex = /^\d{4}-\d{2}-\d{2}$/
    if (!dateRegex.test(date)) {
      return NextResponse.json(
        { error: 'date must be in YYYY-MM-DD format' },
        { status: 400 }
      )
    }

    // Build query string for backend
    const queryParams = new URLSearchParams({
      userId,
      date,
    })
    
    // accountId is optional (defaults to 0 for local version)
    if (accountId) {
      queryParams.append('accountId', accountId)
    } else {
      queryParams.append('accountId', '0')
    }

    // Forward request to backend API
    const response = await fetch(`${BACKEND_URL}/api/timeline?${queryParams.toString()}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('Backend API error:', errorText)
      return NextResponse.json(
        { error: 'Failed to fetch timeline data', details: errorText },
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
