/**
 * ============================================================================
 * USERS API CLIENT
 * ============================================================================
 * 
 * PURPOSE: Handle all users API operations
 * SCOPE: User management operations
 * DEPENDENCIES: Backend API
 * 
 * ============================================================================
 */

// ============================================================================
// INTERFACES
// ============================================================================

export interface User {
  id: number
  email: string
  name: string | null
  owner?: boolean
  account_id?: number
  created_at: string
  updated_at: string
}

export interface PaginationInfo {
  page: number
  limit: number
  count: number
  total: number
  totalPages: number
}

export interface PaginatedResponse<T> {
  data: T[]
  pagination: PaginationInfo
}

// ============================================================================
// API BASE CONFIGURATION
// ============================================================================

// For local bundled app, auth backend is not needed - API calls will be skipped
const API_BASE_URL = ''

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

const handleResponse = async (response: Response) => {
  if (!response.ok) {
    const errorData = await response.json().catch(() => ({}))
    throw new Error(errorData.message || `HTTP error! status: ${response.status}`)
  }
  return response.json()
}

// Import authenticated fetch for API calls
import { authenticatedFetch } from './authAPI'

// ============================================================================
// USERS API
// ============================================================================

export const usersAPI = {
  // Get all users with pagination
  getUsersPaginated: async (accountId: number, page: number = 0, limit: number = 20): Promise<PaginatedResponse<User>> => {
    // For local bundled app, return empty list if auth backend is not configured
    if (!API_BASE_URL || API_BASE_URL === 'undefined') {
      return {
        data: [],
        pagination: {
          page,
          limit,
          count: 0,
          total: 0,
          totalPages: 0,
        },
      }
    }
    const response = await authenticatedFetch(`${API_BASE_URL}/users/${accountId}/users?page=${page}&limit=${limit}`, {
      method: 'GET',
    })
    return handleResponse(response)
  },
}

