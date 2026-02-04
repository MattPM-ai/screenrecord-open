/**
 * ============================================================================
 * LOCAL AUTH (for bundled app without auth backend)
 * ============================================================================
 * 
 * Minimal auth utilities for local/bundled mode where full auth backend
 * is not available. Returns null for tokens since authentication is not needed.
 * 
 * ============================================================================
 */

/**
 * Get access token (returns null in local/bundled mode)
 */
export const getAccessToken = (): string | null => {
  // In local/bundled mode, no auth backend is available
  // Return null to indicate no token is available
  return null
}

