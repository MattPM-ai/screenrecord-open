/**
 * ============================================================================
 * AUDIO TRANSCRIPTS API CLIENT
 * ============================================================================
 * 
 * PURPOSE: Provides API functions for the Audio page
 * 
 * DESCRIPTION:
 * This module handles API calls for audio transcript data.
 * Fetches transcripts from the backend API endpoint /api/audio-transcripts
 * 
 * ============================================================================
 */

// ============================================================================
// API BASE CONFIGURATION
// ============================================================================

// Use Next.js API routes (which proxy to the backend)
// This matches the pattern used by other API calls in this frontend
const API_BASE_URL = process.env.NEXT_PUBLIC_BACKEND_URL || ''

// ============================================================================
// TYPES
// ============================================================================

export interface AudioTranscriptFields {
  audio_url: string
  duration_ms: number
  result?: string
  speaker: string
  table?: number
  text: string
}

export interface AudioTranscript {
  time: string
  accountId: number
  orgId?: number
  userId: number
  org: string
  user: string
  hostname: string
  fields: AudioTranscriptFields
}

export interface AudioTranscriptGroup {
  audioUrl: string
  transcripts: AudioTranscript[]
}

export interface AudioTranscriptsResponse {
  userId: number
  accountId: number
  orgId?: number
  transcripts: AudioTranscriptGroup[]
}

// ============================================================================
// API FUNCTIONS
// ============================================================================

/**
 * Fetches audio transcripts for a specific user and account
 * Transcripts are grouped by audio URL, with transcripts sorted by time within each group
 * 
 * INPUTS:
 * - userId: number - The user ID to query transcripts for
 * - accountId?: number - Optional account ID (defaults to 0 for local version)
 * - orgId?: number - Optional organization ID filter
 * - date?: Date - Optional date to filter by (format: YYYY-MM-DD). If not provided, returns last 30 days
 * 
 * OUTPUTS:
 * - Promise<AudioTranscriptGroup[]> - Array of transcript groups, each containing an audioUrl and transcripts array
 */
export async function getAudioTranscripts(
  userId: number,
  accountId: number = 0,
  orgId?: number,
  date?: Date
): Promise<AudioTranscriptGroup[]> {
  try {
    // Build query parameters
    const params = new URLSearchParams()
    params.set('userId', userId.toString())
    params.set('accountId', accountId.toString())
    if (orgId !== undefined) {
      params.set('orgId', orgId.toString())
    }
    if (date !== undefined) {
      // Format date as YYYY-MM-DD
      const dateStr = date.toISOString().split('T')[0]
      params.set('date', dateStr)
    }

    // Use Next.js API route (relative URL) which proxies to backend
    // This matches the pattern used in activityAPI.ts
    const url = `/api/audio-transcripts?${params.toString()}`
    
    console.log('[getAudioTranscripts] ===== API REQUEST =====')
    console.log('[getAudioTranscripts] Fetching from:', url)
    console.log('[getAudioTranscripts] API_BASE_URL:', API_BASE_URL)
    console.log('[getAudioTranscripts] Request params:', {
      userId,
      accountId,
      orgId,
      date: date ? date.toISOString().split('T')[0] : undefined
    })
    
    const response = await fetch(url, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    })

    console.log('[getAudioTranscripts] Response status:', response.status, response.statusText)
    console.log('[getAudioTranscripts] Response headers:', Object.fromEntries(response.headers.entries()))

    if (!response.ok) {
      const errorText = await response.text().catch(() => '')
      let errorData
      try {
        errorData = JSON.parse(errorText)
      } catch {
        errorData = { raw: errorText }
      }
      console.error('[getAudioTranscripts] ===== ERROR RESPONSE =====')
      console.error('[getAudioTranscripts] Status:', response.status)
      console.error('[getAudioTranscripts] Error data:', errorData)
      throw new Error(errorData.error || `HTTP error! status: ${response.status}`)
    }

    const responseText = await response.text()
    console.log('[getAudioTranscripts] Raw response text length:', responseText.length)
    
    let data: AudioTranscriptsResponse
    try {
      data = JSON.parse(responseText)
    } catch (parseError) {
      console.error('[getAudioTranscripts] ===== JSON PARSE ERROR =====')
      console.error('[getAudioTranscripts] Parse error:', parseError)
      console.error('[getAudioTranscripts] Response text (first 500 chars):', responseText.substring(0, 500))
      throw new Error('Failed to parse JSON response')
    }

    console.log('[getAudioTranscripts] ===== API RESPONSE =====')
    console.log('[getAudioTranscripts] Response structure:', {
      userId: data.userId,
      accountId: data.accountId,
      orgId: data.orgId,
      transcriptGroupsCount: data.transcripts?.length || 0
    })
    
    // Log detailed information about each group
    if (data.transcripts) {
      data.transcripts.forEach((group, index) => {
        console.log(`[getAudioTranscripts] ===== Group ${index} =====`)
        console.log(`[getAudioTranscripts] Group ${index} audioUrl:`, group.audioUrl)
        console.log(`[getAudioTranscripts] Group ${index} audioUrl type:`, typeof group.audioUrl)
        console.log(`[getAudioTranscripts] Group ${index} audioUrl length:`, group.audioUrl?.length || 0)
        console.log(`[getAudioTranscripts] Group ${index} transcript count:`, group.transcripts?.length || 0)
        
        // Check if transcripts have audio_url in their fields
        if (group.transcripts && group.transcripts.length > 0) {
          const transcriptsWithAudioUrl = group.transcripts.filter(t => t.fields?.audio_url)
          const transcriptsWithoutAudioUrl = group.transcripts.filter(t => !t.fields?.audio_url)
          
          console.log(`[getAudioTranscripts] Group ${index} transcripts WITH audio_url field:`, transcriptsWithAudioUrl.length)
          console.log(`[getAudioTranscripts] Group ${index} transcripts WITHOUT audio_url field:`, transcriptsWithoutAudioUrl.length)
          
          if (transcriptsWithAudioUrl.length > 0) {
            console.log(`[getAudioTranscripts] Group ${index} sample audio_url values:`, 
              transcriptsWithAudioUrl.slice(0, 3).map(t => ({
                audio_url: t.fields?.audio_url,
                time: t.time
              }))
            )
          }
          
          // Log first transcript's fields for inspection
          console.log(`[getAudioTranscripts] Group ${index} first transcript fields:`, group.transcripts[0].fields)
        }
      })
    }
    
    console.log('[getAudioTranscripts] Full API Response JSON:', JSON.stringify(data, null, 2))
    
    return data.transcripts || []
  } catch (error) {
    console.error('Failed to fetch audio transcripts:', error)
    throw error
  }
}
