/**
 * ============================================================================
 * AUDIO TRANSCRIPTS API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Fetches audio transcript data from the backend API
 * 
 * DESCRIPTION:
 * This API route forwards audio transcript requests to the backend API
 * and returns audio transcripts for a specific user, grouped by audio URL.
 * 
 * DEPENDENCIES:
 * - External: Backend API at http://localhost:8085
 * 
 * INPUTS:
 * - GET query params: userId, accountId, orgId (optional), date (optional, YYYY-MM-DD)
 * 
 * OUTPUTS:
 * - JSON: AudioTranscriptResponse with transcripts array
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
    const orgId = searchParams.get('orgId')
    const date = searchParams.get('date')

    // Validate required parameters
    if (!userId) {
      return NextResponse.json(
        { error: 'userId is required' },
        { status: 400 }
      )
    }

    if (!accountId) {
      return NextResponse.json(
        { error: 'accountId is required' },
        { status: 400 }
      )
    }

    // Build query string for backend
    const queryParams = new URLSearchParams({
      userId,
      accountId,
    })
    
    // orgId is optional
    if (orgId) {
      queryParams.append('orgId', orgId)
    }
    
    // date is optional
    if (date) {
      // Validate date format if provided
      const dateRegex = /^\d{4}-\d{2}-\d{2}$/
      if (!dateRegex.test(date)) {
        return NextResponse.json(
          { error: 'date must be in YYYY-MM-DD format' },
          { status: 400 }
        )
      }
      queryParams.append('date', date)
    }

    // Forward request to backend API
    const response = await fetch(`${BACKEND_URL}/api/audio-transcripts?${queryParams.toString()}`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('Backend API error:', errorText)
      return NextResponse.json(
        { error: 'Failed to fetch audio transcripts', details: errorText },
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
