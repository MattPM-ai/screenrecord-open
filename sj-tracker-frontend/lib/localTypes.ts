/**
 * ============================================================================
 * LOCAL TYPES (for bundled app without auth backend)
 * ============================================================================
 * 
 * These types are used by components but don't require API calls
 * since we're running in local/bundled mode without an auth backend.
 * 
 * ============================================================================
 */

export interface User {
  id: number
  email: string
  name: string | null
  account_id?: number
  owner?: boolean
  created_at: string
  updated_at: string
}

export interface Organisation {
  id: number
  name: string
  description: string
  account_id: number
  created_at: string
  updated_at: string
}

export interface OrganisationUser {
  id: number
  email: string
  name: string
  owner: boolean
  created_at: string
}

/**
 * Get default user for local/bundled app
 */
export const getDefaultUser = (): User => ({
  id: 0,
  email: 'local@screenrecord.local',
  name: 'Local User',
  account_id: 0,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
  owner: true,
})

/**
 * Get default organization for local/bundled app
 */
export const getDefaultOrganisation = (accountId: number = 0): Organisation => ({
  id: 0,
  name: 'Local Organization',
  description: 'Default organization for local use',
  account_id: accountId,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
})

/**
 * Simple logout function (no-op for local app)
 */
export const logout = () => {
  // No-op for local bundled app
}

