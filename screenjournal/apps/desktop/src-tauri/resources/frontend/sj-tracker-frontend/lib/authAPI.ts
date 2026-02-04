/**
 * ============================================================================
 * AUTHENTICATION API CLIENT
 * ============================================================================
 * 
 * PURPOSE: Handle all authentication operations with the backend
 * SCOPE: Token management, authenticated requests, token refresh
 * DEPENDENCIES: Backend API
 * 
 * ============================================================================
 */

// For local bundled app, auth backend is not needed - API calls will be skipped
const API_BASE_URL = ''

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
 * Set secure cookies for authentication tokens
 */
const setAuthCookies = (accessToken: string, refreshToken: string) => {
  // Only use secure flag in production (HTTPS), not on localhost (HTTP)
  const isSecure = typeof window !== 'undefined' && window.location.protocol === 'https:'
  const secureFlag = isSecure ? '; secure' : ''
  
  // Set access token cookie (7 days)
  const accessExpiry = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000)
  document.cookie = `accessToken=${accessToken}; expires=${accessExpiry.toUTCString()}; path=/${secureFlag}; sameSite=strict`
  
  // Set refresh token cookie (7 days)
  const refreshExpiry = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000)
  document.cookie = `refreshToken=${refreshToken}; expires=${refreshExpiry.toUTCString()}; path=/${secureFlag}; sameSite=strict`
  
  dispatchAuthStateChange()
}

/**
 * Clear authentication cookies
 */
const clearAuthCookies = () => {
  document.cookie = 'accessToken=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/;'
  document.cookie = 'refreshToken=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/;'
  dispatchAuthStateChange()
}

/**
 * Get cookie value by name
 */
const getCookie = (name: string): string | null => {
  if (typeof window === 'undefined') return null
  const value = `; ${document.cookie}`
  const parts = value.split(`; ${name}=`)
  return parts.length === 2 ? parts.pop()?.split(';').shift() || null : null
}

/**
 * Get current access token
 */
export const getAccessToken = (): string | null => {
  return getCookie('accessToken')
}

/**
 * Refresh authentication token
 */
const refreshToken = async (): Promise<string | null> => {
  // If a refresh is already in progress, wait for it to complete
  if (refreshPromise) {
    return await refreshPromise
  }

  // Create new refresh promise
  refreshPromise = (async () => {
    try {
      const refreshTokenValue = getCookie('refreshToken')
      if (!refreshTokenValue) {
        return null
      }

      const response = await fetch(`${API_BASE_URL}/auth/refresh`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ refreshToken: refreshTokenValue }),
      })

      if (!response.ok) {
        throw new Error('Token refresh failed')
      }

      const data = await response.json()
      setAuthCookies(data.data.accessToken, data.data.refreshToken)
      return data.data.accessToken
    } catch (error) {
      console.error('Token refresh failed:', error)
      clearAuthCookies()
      return null
    } finally {
      // Clear the refresh promise when done
      refreshPromise = null
    }
  })()

  return await refreshPromise
}

/**
 * Make API request (no authentication required for open-source local version)
 * 
 * INPUTS:
 * - url: string - The API endpoint URL
 * - options: RequestInit - Fetch options
 * 
 * OUTPUTS:
 * - Response - The fetch response
 */
export const authenticatedFetch = async (url: string, options: RequestInit = {}): Promise<Response> => {
  // Simple fetch without authentication for open-source local version
  return fetch(url, {
    ...options,
    headers: {
      ...options.headers,
      'Content-Type': 'application/json',
    },
  })
}

/**
 * Check if user is authenticated (always true for open-source local version)
 */
export const isAuthenticatedSync = (): boolean => {
  return true // No authentication required for open-source local version
}

/**
 * Check if user is authenticated (always true for open-source local version)
 */
export const isAuthenticated = async (): Promise<boolean> => {
  return true // No authentication required for open-source local version
}

/**
 * Check if user is authenticated (always true for open-source local version)
 */
export const checkAuthentication = async (): Promise<boolean> => {
  return true // No authentication required for open-source local version
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
  const response = await fetch(`${API_BASE_URL}/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const errorData = await response.json().catch(() => ({}))
    throw new Error(errorData.message || 'Login failed')
  }

  const authData: AuthResponse = await response.json()
  
  // Set cookies
  setAuthCookies(authData.data.accessToken, authData.data.refreshToken)
  
  return authData
}

/**
 * User registration
 */
export const register = async (data: RegisterRequest): Promise<AuthResponse> => {
  const response = await fetch(`${API_BASE_URL}/auth/register`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      ...data,
      tracker_enabled: true,
    }),
  })

  if (!response.ok) {
    const errorData: ErrorResponse = await response.json().catch(() => ({ message: 'Registration failed' }))
    throw new Error(errorData.message || 'Registration failed')
  }

  const authData: AuthResponse = await response.json()
  
  // Note: Non-business users are now created as pending_user
  // Only set auth cookies if the response contains tokens
  // (this handles both old and new response formats)
  if (authData.data.accessToken && authData.data.refreshToken) {
    setAuthCookies(authData.data.accessToken, authData.data.refreshToken)
  }
  
  return authData
}

/**
 * Business registration
 */
export const registerBusiness = async (data: RegisterRequest): Promise<AuthResponse> => {
  const response = await fetch(`${API_BASE_URL}/auth/register/business`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      ...data,
      tracker_enabled: true,
    }),
  })

  if (!response.ok) {
    const errorData: ErrorResponse = await response.json().catch(() => ({ message: 'Business registration failed' }))
    throw new Error(errorData.message || 'Business registration failed')
  }

  const authData: AuthResponse = await response.json()
  
  // Set cookies for business users (they get immediate access)
  // Add defensive check to ensure tokens exist before setting cookies
  if (authData.data?.accessToken && authData.data?.refreshToken) {
    setAuthCookies(authData.data.accessToken, authData.data.refreshToken)
  } else {
    throw new Error('Registration succeeded but no authentication tokens were provided. Please contact support.')
  }
  
  return authData
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
  // For open-source local version, return a default user if profile endpoint is not available
  // If API_BASE_URL is not set, return default user immediately
  if (!API_BASE_URL || API_BASE_URL === 'undefined') {
    return {
      id: 0,
      email: 'local@screenjournal.local',
      name: 'Local User',
      account_id: 0, // Default account ID for local version
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      owner: true,
    }
  }
  
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/users/profile`)
    
    if (!response.ok) {
      // If profile endpoint fails, return default user for local version
      return {
        id: 0,
        email: 'local@screenjournal.local',
        name: 'Local User',
        account_id: 0, // Default account ID for local version
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        owner: true,
      }
    }

    const profileData: ProfileResponse = await response.json()
    return profileData.data
  } catch (error) {
    // Return default user if profile endpoint is not available
    return {
      id: 0,
      email: 'local@screenjournal.local',
      name: 'Local User',
      account_id: 0, // Default account ID for local version
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      owner: true,
    }
  }
}

/**
 * Logout
 */
export const logout = () => {
  clearAuthCookies()
}

/**
 * Get account information for admin users
 */
export interface Account {
  id: number
  name: string
  type: string
  industry: string
  created_at: string
  updated_at: string
}

export interface AccountResponse {
  success: boolean
  data: Account
}

export const getAccountInfo = async (accountId: number): Promise<Account> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}`)
    if (!response.ok) {
      throw new Error(`Failed to fetch account info: ${response.status}`)
    }
    const data: AccountResponse = await response.json()
    return data.data
  } catch (error) {
    console.error('getAccountInfo error:', error)
    throw new Error('Failed to fetch account information')
  }
}

/**
 * Join code management
 */
export interface JoinCode {
  code: string
  account_id: string
}

export interface JoinCodesResponse {
  success: boolean
  message: string
  data: JoinCode[]
}

export interface CreateJoinCodeRequest {
  expires_in_days: number
}

export interface CreateJoinCodeResponse {
  success: boolean
  message: string
  data: JoinCode
}

export const getJoinCodes = async (accountId: number): Promise<JoinCode[]> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/join-codes`)
    if (!response.ok) {
      throw new Error(`Failed to fetch join codes: ${response.status}`)
    }
    const data: JoinCodesResponse = await response.json()
    return data.data
  } catch (error) {
    console.error('getJoinCodes error:', error)
    throw new Error('Failed to fetch join codes')
  }
}

export const createJoinCode = async (accountId: number, expiresInDays: number): Promise<JoinCode> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/join-codes`, {
      method: 'POST',
      body: JSON.stringify({ expires_in_days: expiresInDays }),
    })
    if (!response.ok) {
      throw new Error(`Failed to create join code: ${response.status}`)
    }
    const data: CreateJoinCodeResponse = await response.json()
    return data.data
  } catch (error) {
    console.error('createJoinCode error:', error)
    throw new Error('Failed to create join code')
  }
}

export const refreshJoinCode = async (accountId: number): Promise<JoinCode> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/join-codes`, {
      method: 'PUT',
    })
    if (!response.ok) {
      throw new Error(`Failed to refresh join code: ${response.status}`)
    }
    const data: CreateJoinCodeResponse = await response.json()
    return data.data
  } catch (error) {
    console.error('refreshJoinCode error:', error)
    throw new Error('Failed to refresh join code')
  }
}

export const deleteAllJoinCodes = async (accountId: number): Promise<void> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/join-codes`, {
      method: 'DELETE',
    })
    if (!response.ok) {
      throw new Error(`Failed to delete join codes: ${response.status}`)
    }
  } catch (error) {
    console.error('deleteAllJoinCodes error:', error)
    throw new Error('Failed to delete join codes')
  }
}

/**
 * User invitation
 */
export const inviteUser = async (accountId: number, email: string, referralCode: string): Promise<void> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/invite`, {
      method: 'POST',
      body: JSON.stringify({ email, referralCode }),
    })
    if (!response.ok) {
      throw new Error(`Failed to send invite: ${response.status}`)
    }
  } catch (error) {
    console.error('inviteUser error:', error)
    throw new Error('Failed to send invite')
  }
}

/**
 * Pending users management
 */
export interface PendingUser {
  id: string
  created_at: string
  updated_at: string
  email: string
  account_id: string
}

export interface PendingUsersResponse {
  success: boolean
  message: string
  data: PendingUser[]
}

export interface PendingUserResponse {
  success: boolean
  message: string
  data: PendingUser
}

export interface ApproveUserResponse {
  success: boolean
  message: string
  data: {
    approvedUser: User
    message: string
  }
}

export const getPendingUsers = async (accountId: number): Promise<PendingUser[]> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/pending-users`)
    if (!response.ok) {
      if (response.status === 403 || response.status === 401) {
        console.warn('Access to pending users is restricted')
        return []
      }
      throw new Error(`Failed to fetch pending users: ${response.status}`)
    }
    const data: PendingUsersResponse = await response.json()
    return data.data
  } catch (error) {
    console.warn('getPendingUsers error (likely permission restricted):', error)
    return []
  }
}

export const approvePendingUser = async (accountId: number, pendingUserId: string): Promise<ApproveUserResponse | null> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/pending-users/${pendingUserId}/approve`, {
      method: 'POST',
    })
    if (!response.ok) {
      if (response.status === 403 || response.status === 401) {
        console.warn('Access to approve pending user is restricted')
        return null
      }
      throw new Error(`Failed to approve pending user: ${response.status}`)
    }
    const data: ApproveUserResponse = await response.json()
    return data
  } catch (error) {
    console.warn('approvePendingUser error (likely permission restricted):', error)
    return null
  }
}

export const deletePendingUser = async (accountId: number, pendingUserId: string): Promise<boolean> => {
  try {
    const response = await authenticatedFetch(`${API_BASE_URL}/accounts/${accountId}/pending-users/${pendingUserId}`, {
      method: 'DELETE',
    })
    if (!response.ok) {
      if (response.status === 403 || response.status === 401) {
        console.warn('Access to delete pending user is restricted')
        return false
      }
      throw new Error(`Failed to delete pending user: ${response.status}`)
    }
    return true
  } catch (error) {
    console.warn('deletePendingUser error (likely permission restricted):', error)
    return false
  }
}

/**
 * Generate referral link
 */
export const generateReferralLink = async (accountId: number): Promise<string> => {
  try {
    const joinCodes = await getJoinCodes(accountId)
    if (joinCodes.length === 0) {
      // Create a new join code if none exists
      const newCode = await createJoinCode(accountId, 30)
      return `${typeof window !== 'undefined' ? window.location.origin : ''}/register?code=${newCode.code}`
    }
    return `${typeof window !== 'undefined' ? window.location.origin : ''}/register?code=${joinCodes[0].code}`
  } catch (error) {
    console.error('generateReferralLink error:', error)
    throw new Error('Failed to generate referral link')
  }
}

