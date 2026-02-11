/**
 * ============================================================================
 * AUDIO FILE API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Proxies audio file requests to the backend API
 * 
 * DESCRIPTION:
 * This API route forwards audio file requests to the backend API
 * which serves local audio files from the filesystem.
 * 
 * DEPENDENCIES:
 * - External: Backend API at http://localhost:8085
 * 
 * INPUTS:
 * - GET query params: path (absolute or relative path to audio file)
 * 
 * OUTPUTS:
 * - Audio file stream (MP4)
 * 
 * ============================================================================
 */

import { NextRequest, NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function GET(request: NextRequest) {
  try {
    // Get the path query parameter
    const searchParams = request.nextUrl.searchParams
    const filePath = searchParams.get('path')

    if (!filePath) {
      return NextResponse.json(
        { error: 'path parameter is required' },
        { status: 400 }
      )
    }

    // Forward request to backend API
    const backendUrl = `${BACKEND_URL}/api/audio-file?path=${encodeURIComponent(filePath)}`
    console.log('[audio-file route] Proxying to backend:', backendUrl)

    const response = await fetch(backendUrl, {
      method: 'GET',
      // Don't set Content-Type - let the backend set it
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('[audio-file route] Backend API error:', response.status, errorText)
      return NextResponse.json(
        { error: 'Failed to fetch audio file', details: errorText },
        { status: response.status }
      )
    }

    // Get the audio file as a blob
    const audioBlob = await response.blob()
    
    // Get content type from backend response
    const contentType = response.headers.get('content-type') || 'audio/mp4'
    const contentLength = response.headers.get('content-length')
    
    // Return the audio file with appropriate headers
    const headers = new Headers()
    headers.set('Content-Type', contentType)
    if (contentLength) {
      headers.set('Content-Length', contentLength)
    }
    headers.set('Accept-Ranges', 'bytes')
    headers.set('Cache-Control', 'public, max-age=3600') // Cache for 1 hour

    return new NextResponse(audioBlob, {
      status: 200,
      headers,
    })
  } catch (error) {
    console.error('[audio-file route] Error:', error)
    
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
