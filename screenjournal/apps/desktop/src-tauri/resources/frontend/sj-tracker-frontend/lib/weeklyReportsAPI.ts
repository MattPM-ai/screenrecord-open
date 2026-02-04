/**
 * ============================================================================
 * WEEKLY REPORTS EMAIL API CLIENT
 * ============================================================================
 * 
 * PURPOSE: Handle weekly reports email opt-in/opt-out operations
 * SCOPE: Email subscription management for weekly reports
 * DEPENDENCIES: Backend API
 * 
 * ============================================================================
 */

// Note: Using regular fetch since we're not using auth backend in bundled mode

export interface WeeklyReportOptInRequest {
  accountId: number
  orgId: number
  orgName: string
  email: string
  users: Array<{ name: string; id: number }>
  nextTriggerTime?: string // Optional - ISO 8601 datetime for testing
}

export interface WeeklyReportOptInResponse {
  accountId: number
  message: string
  orgId: number
}

export interface WeeklyReportOptOutRequest {
  accountId: number
  orgId: number
}

export interface WeeklyReportOptOutResponse {
  accountId: number
  message: string
  orgId: number
}

export interface ErrorResponse {
  error: string
}

export interface OptedInAccount {
  accountId: number
  orgId: number
  orgName: string
  email: string
  users: Array<{ name: string; id: number }>
  optedInAt: string // ISO 8601 datetime
  nextTriggerTime?: string // Optional - ISO 8601 datetime for testing
}

export interface OptedInAccountsResponse {
  accountId: number
  accounts: OptedInAccount[]
}

/**
 * Opt in to weekly reports email
 * 
 * INPUTS:
 * - data: WeeklyReportOptInRequest - Opt-in request data
 * 
 * OUTPUTS:
 * - WeeklyReportOptInResponse - Success response
 * 
 * ERRORS:
 * - Throws Error with message from backend on failure
 */
export const optInWeeklyReports = async (data: WeeklyReportOptInRequest): Promise<WeeklyReportOptInResponse> => {
  try {
    const response = await fetch('/api/reports/weekly/opt-in', {
      method: 'POST',
      body: JSON.stringify(data),
    })

    if (!response.ok) {
      const errorData: ErrorResponse = await response.json().catch(() => ({ error: 'Failed to opt in to weekly reports' }))
      throw new Error(errorData.error || 'Failed to opt in to weekly reports')
    }

    const result: WeeklyReportOptInResponse = await response.json()
    return result
  } catch (error) {
    console.error('optInWeeklyReports error:', error)
    throw error
  }
}

/**
 * Opt out of weekly reports email
 * 
 * INPUTS:
 * - data: WeeklyReportOptOutRequest - Opt-out request data
 * 
 * OUTPUTS:
 * - WeeklyReportOptOutResponse - Success response
 * 
 * ERRORS:
 * - Throws Error with message from backend on failure
 */
export const optOutWeeklyReports = async (data: WeeklyReportOptOutRequest): Promise<WeeklyReportOptOutResponse> => {
  try {
    const response = await fetch('/api/reports/weekly/opt-out', {
      method: 'POST',
      body: JSON.stringify(data),
    })

    if (!response.ok) {
      const errorData: ErrorResponse = await response.json().catch(() => ({ error: 'Failed to opt out of weekly reports' }))
      throw new Error(errorData.error || 'Failed to opt out of weekly reports')
    }

    const result: WeeklyReportOptOutResponse = await response.json()
    return result
  } catch (error) {
    console.error('optOutWeeklyReports error:', error)
    throw error
  }
}

export interface OptedInAccount {
  accountId: number
  orgId: number
  orgName: string
  email: string
  users: Array<{ name: string; id: number }>
  optedInAt: string // ISO 8601 datetime
  nextTriggerTime?: string // Optional - ISO 8601 datetime for testing
}

export interface OptedInAccountsResponse {
  accountId: number
  accounts: OptedInAccount[]
}

/**
 * Get opted-in accounts for weekly reports
 * 
 * INPUTS:
 * - accountId: number - The account ID to query
 * 
 * OUTPUTS:
 * - OptedInAccountsResponse - Response with list of opted-in accounts
 * 
 * ERRORS:
 * - Throws Error with message from backend on failure
 */
export const getOptedInAccounts = async (accountId: number): Promise<OptedInAccountsResponse> => {
  try {
    const response = await fetch(`/api/reports/weekly/opted-in/${accountId}`, {
      method: 'GET',
    })

    if (!response.ok) {
      const errorData: ErrorResponse = await response.json().catch(() => ({ error: 'Failed to fetch opted-in accounts' }))
      throw new Error(errorData.error || 'Failed to fetch opted-in accounts')
    }

    const result: OptedInAccountsResponse = await response.json()
    return result
  } catch (error) {
    console.error('getOptedInAccounts error:', error)
    throw error
  }
}

