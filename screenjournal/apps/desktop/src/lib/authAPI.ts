/**
 * ============================================================================
 * AUTHENTICATION API CLIENT
 * ============================================================================
 * 
 * PURPOSE: Handle all authentication operations with the backend
 * SCOPE: Token management, authenticated requests, token refresh
 * DEPENDENCIES: Backend API
 * 
 * Based on sj-tracker-frontend implementation, adapted for Tauri desktop app
 * Uses localStorage instead of cookies for token storage
 * 
 * ============================================================================
 */

const API_BASE_URL = 'http://localhost:8080/api'

// Storage keys
const ACCESS_TOKEN_KEY = 'auth_access_token'
const REFRESH_TOKEN_KEY = 'auth_refresh_token'

// Refresh lock to prevent simultaneous refresh operations
let refreshPromise: Promise<string | null> | null = null

/**
 * Dispatch custom event to notify components of authentication state change
 */
const dispatchAuthStateChange = () => {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('authStateChange'))
  }
}

/**
 * Set authentication tokens in localStorage
 * Also updates collector with the new token (fire-and-forget)
 */
const setAuthTokens = (accessToken: string, refreshToken: string) => {
  try {
    localStorage.setItem(ACCESS_TOKEN_KEY, accessToken)
    localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken)
    dispatchAuthStateChange()
    
    // Update collector with the new token (fire-and-forget, don't block)
    import('@/lib/collectorClient')
      .then(({ updateCollectorAppJwtToken }) => updateCollectorAppJwtToken(accessToken))
      .then(() => console.log('[AUTH] Updated collector with new app JWT token'))
      .catch((error) => console.warn('[AUTH] Failed to update collector token:', error))
  } catch (error) {
    console.error('Failed to store auth tokens:', error)
    throw new Error('Failed to store authentication tokens')
  }
}

/**
 * Clear authentication tokens from localStorage
 * Also clears token from collector (fire-and-forget)
 */
const clearAuthTokens = () => {
  try {
    localStorage.removeItem(ACCESS_TOKEN_KEY)
    localStorage.removeItem(REFRESH_TOKEN_KEY)
    dispatchAuthStateChange()
    
    // Clear collector token (fire-and-forget, don't block)
    import('@/lib/collectorClient')
      .then(({ updateCollectorAppJwtToken }) => updateCollectorAppJwtToken(null))
      .then(() => console.log('[AUTH] Cleared collector app JWT token'))
      .catch((error) => console.warn('[AUTH] Failed to clear collector token:', error))
  } catch (error) {
    console.error('Failed to clear auth tokens:', error)
  }
}

/**
 * Get current access token from localStorage
 */
export const getAccessToken = (): string | null => {
  if (typeof window === 'undefined') return null
  try {
    return localStorage.getItem(ACCESS_TOKEN_KEY)
  } catch (error) {
    console.error('Failed to get access token:', error)
    return null
  }
}

/**
 * Get current refresh token from localStorage
 */
const getRefreshToken = (): string | null => {
  if (typeof window === 'undefined') return null
  try {
    return localStorage.getItem(REFRESH_TOKEN_KEY)
  } catch (error) {
    console.error('Failed to get refresh token:', error)
    return null
  }
}

/**
 * Refresh authentication token
 * 
 * Handles various failure scenarios:
 * - No refresh token available
 * - Expired/invalid refresh token (401/403) - clears tokens, user needs to re-login
 * - Network errors - logged but tokens preserved for retry
 * - Server errors (5xx) - logged but tokens preserved for retry
 */
const refreshToken = async (): Promise<string | null> => {
  // If a refresh is already in progress, wait for it to complete
  if (refreshPromise) {
    return await refreshPromise
  }

  // Create new refresh promise
  refreshPromise = (async () => {
    try {
      const refreshTokenValue = getRefreshToken()
      if (!refreshTokenValue) {
        console.warn('[AUTH] No refresh token available')
        return null
      }

      console.log('[AUTH] Attempting token refresh...')
      
      const response = await fetch(`${API_BASE_URL}/auth/refresh`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ refreshToken: refreshTokenValue }),
      })

      if (!response.ok) {
        // Try to extract error details from response
        let errorMessage = 'Unknown error'
        let errorData: any = null
        try {
          errorData = await response.json()
          errorMessage = errorData?.message || errorData?.error || `HTTP ${response.status}`
        } catch {
          errorMessage = `HTTP ${response.status} ${response.statusText}`
        }

        console.error(`[AUTH] Token refresh failed - Status: ${response.status}, Message: ${errorMessage}`)

        // Handle specific status codes
        if (response.status === 401 || response.status === 403) {
          // Refresh token is expired or invalid - user needs to re-login
          console.warn('[AUTH] Refresh token expired or invalid - clearing tokens, re-authentication required')
          clearAuthTokens()
          return null
        }

        if (response.status >= 500) {
          // Server error - don't clear tokens, might be temporary
          console.warn('[AUTH] Server error during refresh - tokens preserved for retry')
          return null
        }

        // Other client errors (400, 404, etc.) - likely invalid request, clear tokens
        console.warn('[AUTH] Client error during refresh - clearing tokens')
        clearAuthTokens()
        return null
      }

      const data = await response.json()
      console.log('[AUTH] Token refresh successful')
      setAuthTokens(data.data.accessToken, data.data.refreshToken)
      return data.data.accessToken
    } catch (error) {
      // Network error or other fetch failure
      if (error instanceof TypeError && error.message.includes('fetch')) {
        console.error('[AUTH] Network error during token refresh - check connection:', error.message)
        // Don't clear tokens on network error - might be temporary
        return null
      }
      
      console.error('[AUTH] Unexpected error during token refresh:', error)
      // For unexpected errors, clear tokens to be safe
      clearAuthTokens()
      return null
    } finally {
      // Clear the refresh promise when done
      refreshPromise = null
    }
  })()

  return await refreshPromise
}

/**
 * Make authenticated request with automatic token refresh
 * 
 * INPUTS:
 * - url: string - The API endpoint URL
 * - options: RequestInit - Fetch options
 * 
 * OUTPUTS:
 * - Response - The fetch response
 * 
 * ERROR HANDLING:
 * - Automatically refreshes token on 401
 * - Retries request with new token
 * - Throws error if refresh fails
 */
export const authenticatedFetch = async (url: string, options: RequestInit = {}): Promise<Response> => {
  let token = getAccessToken()
  
  // If no access token but we have a refresh token, try to refresh first
  if (!token) {
    const refreshTokenValue = getRefreshToken()
    if (refreshTokenValue) {
      try {
        const newToken = await refreshToken()
        if (newToken) {
          token = newToken
        } else {
          throw new Error('No access token available')
        }
      } catch (error) {
        console.error('Token refresh failed:', error)
        throw new Error('No access token available')
      }
    } else {
      throw new Error('No access token available')
    }
  }

  // Add token to headers
  const response = await fetch(url, {
    ...options,
    headers: {
      ...options.headers,
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
  })

  // Handle token expiration
  if (response.status === 401) {
    const newToken = await refreshToken()
    if (newToken) {
      // Retry original request with new token
      return fetch(url, {
        ...options,
        headers: {
          ...options.headers,
          'Authorization': `Bearer ${newToken}`,
          'Content-Type': 'application/json',
        },
      })
    } else {
      throw new Error('Authentication failed - refresh token expired')
    }
  }

  return response
}

/**
 * Check if user is authenticated synchronously
 */
export const isAuthenticatedSync = (): boolean => {
  const accessToken = getAccessToken()
  if (!accessToken) {
    return false
  }
  
  // Check if JWT token is expired
  try {
    const tokenParts = accessToken.split('.')
    const payloadPart = tokenParts[1]
    if (!payloadPart) {
      return false
    }
    const payload = JSON.parse(atob(payloadPart))
    const isExpired = payload.exp * 1000 <= Date.now()
    return !isExpired
  } catch (error) {
    return false
  }
}

/**
 * Check if user is authenticated with smart refresh logic
 */
export const isAuthenticated = async (): Promise<boolean> => {
  const accessToken = getAccessToken()
  const refreshTokenValue = getRefreshToken()
  
  // If we have no access token but have a refresh token, try to refresh
  if (!accessToken && refreshTokenValue) {
    try {
      const newToken = await refreshToken()
      return newToken !== null
    } catch (error) {
      return false
    }
  }
  
  // If we have no access token and no refresh token, user is not authenticated
  if (!accessToken) {
    return false
  }
  
  // Check if token is expired locally first
  try {
    const tokenParts = accessToken.split('.')
    const payloadPart = tokenParts[1]
    if (!payloadPart) {
      return false
    }
    const payload = JSON.parse(atob(payloadPart))
    const isExpired = payload.exp * 1000 <= Date.now()
    
    if (!isExpired) {
      return true
    }
    
    // Token is expired, try to refresh
    if (refreshTokenValue) {
      try {
        const newToken = await refreshToken()
        return newToken !== null
      } catch (error) {
        return false
      }
    }
    
    return false
  } catch (error) {
    return false
  }
}

/**
 * Check if user is authenticated and refresh if needed
 * This is the main function that should be used for authentication checks
 */
export const checkAuthentication = async (): Promise<boolean> => {
  // First check synchronously for immediate response
  const syncResult = isAuthenticatedSync()
  if (syncResult) {
    return true
  }
  
  // If we get here, either no token, token is expired, or token is malformed
  // Try to refresh the token
  return await isAuthenticated()
}

/**
 * User login
 */
export interface LoginRequest {
  email: string
  password: string
}

export interface RegisterRequest {
  name: string
  email: string
  password: string
  join_code?: string
}

export interface AuthResponse {
  success: boolean
  message: string
  data: {
    user: any
    accessToken: string
    refreshToken: string
  }
}

export interface ErrorResponse {
  success: boolean
  message: string
}

export const login = async (data: LoginRequest): Promise<AuthResponse> => {
  console.log(`[AUTH] Attempting login for: ${data.email}`)
  
  let response: Response
  try {
    response = await fetch(`${API_BASE_URL}/auth/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(data),
    })
  } catch (error) {
    // Network error - fetch itself failed
    console.error('[AUTH] Login network error:', error instanceof Error ? error.message : error)
    throw new Error('Network error - unable to reach authentication server. Check your internet connection.')
  }

  if (!response.ok) {
    // Try to extract error details from response
    let errorMessage = 'Login failed'
    let errorData: any = null
    
    try {
      errorData = await response.json()
      errorMessage = errorData?.message || errorData?.error || errorMessage
    } catch {
      // Response body wasn't valid JSON
      errorMessage = `HTTP ${response.status} ${response.statusText}`
    }

    // Log detailed error info for debugging
    console.error(`[AUTH] Login failed - Status: ${response.status}, Message: ${errorMessage}`)
    
    if (errorData) {
      console.error('[AUTH] Login error details:', JSON.stringify(errorData, null, 2))
    }

    // Provide user-friendly error messages based on status code
    if (response.status === 401) {
      throw new Error('Invalid email or password')
    } else if (response.status === 403) {
      throw new Error('Account access denied. Please contact support.')
    } else if (response.status === 404) {
      throw new Error('Account not found. Please check your email or register.')
    } else if (response.status === 429) {
      throw new Error('Too many login attempts. Please wait a moment and try again.')
    } else if (response.status >= 500) {
      console.error(`[AUTH] Server error ${response.status} - this is a backend issue`)
      throw new Error(`Server error (${response.status}). Please try again later or contact support if the issue persists.`)
    }

    throw new Error(errorMessage)
  }

  let authData: AuthResponse
  try {
    authData = await response.json()
  } catch (error) {
    console.error('[AUTH] Failed to parse login response:', error)
    throw new Error('Invalid response from server. Please try again.')
  }

  // Validate response structure
  if (!authData?.data?.accessToken || !authData?.data?.refreshToken) {
    console.error('[AUTH] Login response missing tokens:', JSON.stringify(authData, null, 2))
    throw new Error('Invalid login response - missing authentication tokens')
  }
  
  console.log('[AUTH] Login successful')
  
  // Set tokens in localStorage
  setAuthTokens(authData.data.accessToken, authData.data.refreshToken)
  
  return authData
}

/**
 * Logout
 */
export const logout = () => {
  clearAuthTokens()
}

/**
 * Get user profile
 */
export interface User {
  id: number
  email: string
  name: string | null
  owner?: boolean
  account_id?: number
  created_at: string
  updated_at: string
}

export interface ProfileResponse {
  success: boolean
  data: User
}

export const getProfile = async (): Promise<User> => {
  const response = await authenticatedFetch(`${API_BASE_URL}/users/profile`)
  
  if (!response.ok) {
    const errorData = await response.json().catch(() => ({}))
    throw new Error(errorData.message || 'Failed to fetch profile')
  }

  const profileData: ProfileResponse = await response.json()
  return profileData.data
}

/**
 * Organisation types
 */
export interface Organisation {
  id: string
  name: string
  description: string
  account_id: string
  created_at: string
  updated_at: string
}

export interface OrganisationsResponse {
  success: boolean
  data: Organisation[]
  pagination: {
    page: number
    limit: number
    count: number
    total: number
  }
}

/**
 * Get user's organisations
 * Returns organisations that the user has scope for
 */
export const getOrganisations = async (): Promise<Organisation[]> => {
  const response = await authenticatedFetch(`${API_BASE_URL}/users/organisations`)
  
  if (!response.ok) {
    const errorData = await response.json().catch(() => ({}))
    throw new Error(errorData.message || 'Failed to fetch organisations')
  }

  const orgsData: OrganisationsResponse = await response.json()
  return orgsData.data
}

