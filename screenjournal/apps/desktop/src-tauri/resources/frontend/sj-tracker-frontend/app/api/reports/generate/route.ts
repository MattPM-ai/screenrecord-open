/**
 * ============================================================================
 * REPORT GENERATION API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Initiates report generation on the backend API
 * 
 * DESCRIPTION:
 * This API route forwards report generation requests to the backend API
 * and returns the task ID for polling.
 * 
 * DEPENDENCIES:
 * - External: Backend API at http://localhost:8085
 * 
 * INPUTS:
 * - POST body: { accountId: number, users: Array<{name: string, id: number}>, org: string, orgId: number, startDate: string, endDate: string }
 * 
 * OUTPUTS:
 * - JSON: { taskId: string, status: string } - Task ID for polling
 * 
 * ============================================================================
 */

import { NextRequest, NextResponse } from 'next/server'

const BACKEND_URL = 'http://localhost:8085'

export async function POST(request: NextRequest) {
  try {
    // No authentication required for open-source local version
    const body = await request.json()
    const { accountId, users, org, orgId, startDate, endDate, geminiApiKey } = body

    // Validate and normalize accountId (handle both string and number)
    let normalizedAccountId: number
    if (accountId === null || accountId === undefined) {
      return NextResponse.json(
        { error: 'accountId is required and must be a number' },
        { status: 400 }
      )
    }
    normalizedAccountId = typeof accountId === 'string' ? Number(accountId) : accountId
    if (isNaN(normalizedAccountId) || normalizedAccountId < 0) {
      return NextResponse.json(
        { error: 'accountId is required and must be a valid non-negative number' },
        { status: 400 }
      )
    }

    if (!users || !Array.isArray(users) || users.length === 0) {
      return NextResponse.json(
        { error: 'users is required and must be a non-empty array' },
        { status: 400 }
      )
    }

    // Validate and normalize each user in the array
    const normalizedUsers = []
    for (let i = 0; i < users.length; i++) {
      const user = users[i]
      if (!user || typeof user !== 'object') {
        return NextResponse.json(
          { error: `users[${i}] must be an object` },
          { status: 400 }
        )
      }
      if (!user.name || typeof user.name !== 'string' || !user.name.trim()) {
        return NextResponse.json(
          { error: `users[${i}].name is required and must be a non-empty string` },
          { status: 400 }
        )
      }
      // Validate and normalize user ID (handle both string and number)
      let normalizedUserId: number
      if (user.id === null || user.id === undefined) {
        return NextResponse.json(
          { error: `users[${i}].id is required and must be a number` },
          { status: 400 }
        )
      }
      normalizedUserId = typeof user.id === 'string' ? Number(user.id) : user.id
      if (isNaN(normalizedUserId) || normalizedUserId < 0) {
        return NextResponse.json(
          { error: `users[${i}].id is required and must be a valid non-negative number` },
          { status: 400 }
        )
      }
      normalizedUsers.push({
        name: user.name.trim(),
        id: normalizedUserId
      })
    }

    if (!org || typeof org !== 'string' || !org.trim()) {
      return NextResponse.json(
        { error: 'org is required and must be a non-empty string' },
        { status: 400 }
      )
    }

    // Validate and normalize orgId (handle both string and number)
    let normalizedOrgId: number
    if (orgId === null || orgId === undefined) {
      return NextResponse.json(
        { error: 'orgId is required and must be a number' },
        { status: 400 }
      )
    }
    normalizedOrgId = typeof orgId === 'string' ? Number(orgId) : orgId
    if (isNaN(normalizedOrgId) || normalizedOrgId < 0) {
      return NextResponse.json(
        { error: 'orgId is required and must be a valid non-negative number' },
        { status: 400 }
      )
    }

    if (!startDate || typeof startDate !== 'string') {
      return NextResponse.json(
        { error: 'startDate is required and must be a string' },
        { status: 400 }
      )
    }

    if (!endDate || typeof endDate !== 'string') {
      return NextResponse.json(
        { error: 'endDate is required and must be a string' },
        { status: 400 }
      )
    }

    // Forward request to backend API
    const response = await fetch(`${BACKEND_URL}/api/reports/generate`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        // No Authorization header needed for local version
      },
      body: JSON.stringify({
        accountId: normalizedAccountId,
        users: normalizedUsers,
        org: org.trim(),
        orgId: normalizedOrgId,
        startDate,
        endDate,
        geminiApiKey: geminiApiKey?.trim() || '',
      }),
      // No Authorization header needed for local version
    })

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error')
      console.error('Backend API error:', errorText)
      return NextResponse.json(
        { error: 'Failed to generate report', details: errorText },
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

