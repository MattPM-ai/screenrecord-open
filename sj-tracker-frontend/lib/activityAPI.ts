/**
 * ============================================================================
 * ACTIVITY PAGE API CLIENT
 * ============================================================================
 * 
 * PURPOSE: Provides API functions for the Activity/Timeline page
 * 
 * DESCRIPTION:
 * This module handles API calls for timeline data.
 * Integrates with the backend API endpoints for the open-source local version.
 * 
 * ============================================================================
 */

// ============================================================================
// API BASE CONFIGURATION
// ============================================================================

// Use Next.js API routes (which proxy to the backend)
// This matches the pattern used by other API calls in this frontend
const API_BASE_URL = ''

// ============================================================================
// COMPONENT TYPES (used by the Activity page)
// ============================================================================

export interface TimelineEntry {
  id: string
  title: string
  app: string
  type: 'WORK' | 'BREAK' | 'MEETING'
  description?: string
  startTime: string
  endTime: string
  status: 'green' | 'grey' | 'blue'
}

// ============================================================================
// API RESPONSE TYPES (from backend)
// ============================================================================

interface TimelineEvent {
  time: string
  app: string
  appTitle: string
  description: string
  productiveScore: number
  durationSeconds: number
}

interface TimelineResponse {
  userId: number
  accountId: number
  date: string
  events: TimelineEvent[]
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/**
 * Map productive score to timeline entry type and status
 */
function mapProductiveScoreToType(productiveScore: number): { type: TimelineEntry['type'], status: TimelineEntry['status'] } {
  if (productiveScore >= 7) {
    return { type: 'WORK', status: 'green' }
  } else if (productiveScore >= 4) {
    return { type: 'MEETING', status: 'blue' }
  } else {
    return { type: 'BREAK', status: 'grey' }
  }
}

/**
 * Format ISO timestamp to time string (HH:MM:SS)
 */
function formatTime(isoString: string): string {
  const date = new Date(isoString)
  return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false })
}

/**
 * Calculate end time from start time and duration
 */
function calculateEndTime(startTime: string, durationSeconds: number): string {
  const start = new Date(startTime)
  const end = new Date(start.getTime() + durationSeconds * 1000)
  return formatTime(end.toISOString())
}

// ============================================================================
// API FUNCTIONS
// ============================================================================

/**
 * Fetches timeline entries for a specific user and date
 * 
 * INPUTS:
 * - userId: number - The user ID (0 for local version)
 * - accountId: number - The account ID (0 for local version)
 * - date: Date - The date to fetch timeline for
 * 
 * OUTPUTS:
 * - Promise<TimelineEntry[]> - Array of timeline entries
 */
export async function getTimelineEntries(userId: number, accountId: number, date: Date): Promise<TimelineEntry[]> {
  try {
    const dateStr = date.toISOString().split('T')[0] // Format as YYYY-MM-DD
    // Use Next.js API route (relative URL) which proxies to backend
    const url = `/api/timeline?userId=${userId}&accountId=${accountId}&date=${dateStr}`
    console.log('[getTimelineEntries] Fetching from:', url)
    
    const response = await fetch(url, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    })

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}))
      throw new Error(errorData.error || `HTTP error! status: ${response.status}`)
    }

    const data: TimelineResponse = await response.json()
    console.log('[getTimelineEntries] API Response:', JSON.stringify(data, null, 2))
    
    // Sort events by time (ascending) - API should already be sorted, but ensure it
    const sortedEvents = [...data.events].sort((a, b) => {
      return new Date(a.time).getTime() - new Date(b.time).getTime()
    })
    
    // Convert timeline events to timeline entries
    return sortedEvents.map((event, index) => {
      const { type, status } = mapProductiveScoreToType(event.productiveScore)
      const startTime = formatTime(event.time)
      const endTime = calculateEndTime(event.time, event.durationSeconds)
      
      return {
        id: `${userId}-${dateStr}-${index}`,
        title: event.appTitle || event.app,
        app: event.app,
        type,
        description: event.description,
        startTime,
        endTime,
        status
      }
    })
  } catch (error) {
    console.error('Failed to fetch timeline entries:', error)
    return []
  }
}
