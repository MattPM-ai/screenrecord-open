/**
 * ============================================================================
 * REPORT FORM COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Form component for submitting report generation requests
 * 
 * DESCRIPTION:
 * Provides a form interface for selecting organization, users, and date range
 * to generate reports from the backend API. Fetches organizations and users
 * from the account and provides dropdown selections.
 * 
 * ============================================================================
 */

'use client'

import { useState, FormEvent, useEffect } from 'react'
import { getDefaultUser, getDefaultOrganisation, type Organisation, type OrganisationUser } from '@/lib/localTypes'
import { getGeminiKeyStatus, getGeminiKey, saveGeminiKey } from '@/lib/geminiApiKey'

const GEMINI_API_KEY_STORAGE_KEY = 'gemini_api_key'

interface ReportFormProps {
  onSubmit: (data: {
    accountId: number
    users: Array<{ name: string; id: number }>
    org: string
    orgId: number
    startDate: string
    endDate: string
    geminiApiKey: string
  }) => void
}

export default function ReportForm({ onSubmit }: ReportFormProps) {
  const [accountId, setAccountId] = useState<number | null>(null)
  const [organisations, setOrganisations] = useState<Organisation[]>([])
  const [users, setUsers] = useState<OrganisationUser[]>([])
  const [selectedOrgId, setSelectedOrgId] = useState<string>('')
  const [selectedUserIds, setSelectedUserIds] = useState<number[]>([])
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [loading, setLoading] = useState(true)
  const [loadingUsers, setLoadingUsers] = useState(false)
  const [error, setError] = useState('')
  const [isOwner, setIsOwner] = useState(false)
  const [currentUserId, setCurrentUserId] = useState<number | null>(null)
  const [currentUserName, setCurrentUserName] = useState<string>('')
  const [currentUserEmail, setCurrentUserEmail] = useState<string>('')
  const [apiKey, setApiKey] = useState<string>('')
  const [showApiKeyInput, setShowApiKeyInput] = useState(false)
  const [apiKeyError, setApiKeyError] = useState<string>('')
  const [keyFromBackend, setKeyFromBackend] = useState(false)

  // Load user profile and organizations
  useEffect(() => {
    const loadData = async () => {
      try {
        setLoading(true)
        setError('')

        // Use default user for local/bundled app (no auth backend)
        const userProfile = getDefaultUser()
        const accountIdValue = userProfile.account_id ?? 0
        setAccountId(accountIdValue)
        setIsOwner(userProfile.owner === true)
        setCurrentUserId(userProfile.id)
        setCurrentUserName(userProfile.name || '')
        setCurrentUserEmail(userProfile.email)

        // Use default organization for local/bundled app
        const defaultOrg = getDefaultOrganisation(accountIdValue)
        setOrganisations([defaultOrg])
        setSelectedOrgId('0')
        
        // Pre-select default user
        setSelectedUserIds([0])
      } catch (err) {
        console.error('Failed to load form data:', err)
        // Set defaults for local version
        setAccountId(0)
        const defaultOrg = getDefaultOrganisation(0)
        setOrganisations([defaultOrg])
        setSelectedOrgId('0')
        setSelectedUserIds([0])
        setError('') // Don't show error for local version
      } finally {
        setLoading(false)
      }
    }

    loadData()

    // Load API key: prefer shared backend (sync with desktop app), then localStorage
    const loadKey = async () => {
      try {
        const { set } = await getGeminiKeyStatus()
        if (set) {
          setKeyFromBackend(true)
          setApiKey('')
          setShowApiKeyInput(false)
          return
        }
      } catch {
        // Backend not available, fall back to localStorage
      }
      const storedApiKey = localStorage.getItem(GEMINI_API_KEY_STORAGE_KEY)
      if (storedApiKey) {
        setApiKey(storedApiKey)
        setShowApiKeyInput(false)
      } else {
        setShowApiKeyInput(true)
      }
    }
    loadKey()
  }, [])

  // For local version, use default user instead of loading from API
  useEffect(() => {
    if (!selectedOrgId || selectedOrgId === '') {
      setUsers([])
      setSelectedUserIds([])
      return
    }

    // Use default user for local version
    const defaultUser: OrganisationUser = {
      id: 0,
      email: 'local@screenjournal.local',
      name: 'Local User',
      owner: true,
      created_at: new Date().toISOString(),
    }
    setUsers([defaultUser])
    setSelectedUserIds([0])
    setLoadingUsers(false)
  }, [selectedOrgId])

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()

    // Use defaults for local version
    const accountIdValue = accountId ?? 0
    const orgIdValue = 0
    const orgName = 'Local Organization'
    const selectedUsers: Array<{ name: string; id: number }> = [{
      name: 'Local User',
      id: 0
    }]

    if (!startDate || !endDate) {
      alert('Please fill in both date fields')
      return
    }

    // Use shared key from backend (same as desktop app), or from form/localStorage
    let geminiApiKey = ''
    if (keyFromBackend) {
      geminiApiKey = '' // Backend will read from file
    } else {
      geminiApiKey = apiKey || localStorage.getItem(GEMINI_API_KEY_STORAGE_KEY) || ''
    }
    
    if (!keyFromBackend && !geminiApiKey) {
      alert('Please enter your Gemini API key below. Reports require a Gemini API key to generate AI-powered insights.')
      return
    }

    onSubmit({
      accountId: accountIdValue,
      users: selectedUsers,
      org: orgName,
      orgId: orgIdValue,
      startDate,
      endDate,
      geminiApiKey,
    })
  }

  /**
   * Handles saving API key (writes to shared backend so desktop app sees it too)
   */
  const handleSaveApiKey = async () => {
    if (!apiKey.trim()) {
      setApiKeyError('API key is required')
      return
    }
    
    if (apiKey.trim().length < 20) {
      setApiKeyError('Invalid API key format. Please check your Gemini API key.')
      return
    }
    
    try {
      await saveGeminiKey(apiKey.trim())
      localStorage.setItem(GEMINI_API_KEY_STORAGE_KEY, apiKey.trim())
      setKeyFromBackend(false)
      setShowApiKeyInput(false)
      setApiKeyError('')
    } catch (err) {
      setApiKeyError(err instanceof Error ? err.message : 'Failed to save key')
    }
  }

  if (loading) {
    return (
      <div className="w-full max-w-2xl bg-white rounded-lg shadow-sm border border-gray-200 p-10">
        <h1 className="text-3xl font-semibold text-gray-900 mb-2">Generate a report</h1>
        <p className="text-sm text-gray-600 mb-8">Loading form data...</p>
        <div className="flex items-center justify-center py-8">
          <div className="w-8 h-8 border-4 border-gray-200 border-t-blue-600 rounded-full animate-spin"></div>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="w-full max-w-2xl bg-white rounded-lg shadow-sm border border-gray-200 p-10">
        <h1 className="text-3xl font-semibold text-gray-900 mb-2">Generate a report</h1>
        <div className="bg-red-50 border border-red-200 rounded-md p-4 mb-4">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="w-full max-w-2xl bg-white rounded-lg shadow-sm border border-gray-200 p-10">
      <h1 className="text-3xl font-semibold text-gray-900 mb-2">Generate a report</h1>
      <p className="text-sm text-gray-600 mb-8">
        Select the date range to generate a comprehensive activity report.
      </p>

      {showApiKeyInput && (
        <div className="mb-6 p-4 border border-gray-200 rounded-md bg-blue-50">
          <div className="flex flex-col gap-2">
            <label htmlFor="api-key" className="text-sm font-medium text-gray-700">
              Gemini API Key
            </label>
            <div className="flex gap-2">
              <input
                id="api-key"
                type="password"
                value={apiKey}
                onChange={(e) => {
                  setApiKey(e.target.value)
                  setApiKeyError('')
                }}
                placeholder="Enter your Gemini API key..."
                className="flex-1 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-sm"
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    handleSaveApiKey()
                  }
                }}
              />
              <button
                type="button"
                onClick={handleSaveApiKey}
                className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors text-sm font-medium"
              >
                Save
              </button>
              {apiKey && (
                <button
                  type="button"
                  onClick={() => {
                    setShowApiKeyInput(false)
                    setApiKeyError('')
                  }}
                  className="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 transition-colors text-sm"
                >
                  Cancel
                </button>
              )}
            </div>
            {apiKeyError && (
              <p className="text-sm text-red-600">{apiKeyError}</p>
            )}
            <p className="text-xs text-gray-600">
              Your API key is stored and shared with the desktop app (Settings). Used for AI-powered report insights.
            </p>
          </div>
        </div>
      )}

      {!showApiKeyInput && (apiKey || keyFromBackend) && (
        <div className="mb-6 p-3 border border-gray-200 rounded-md bg-gray-50 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <svg className="w-4 h-4 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <span className="text-sm text-gray-700">API key configured{keyFromBackend ? ' (shared with desktop app)' : ''}</span>
          </div>
          <button
            type="button"
            onClick={() => setShowApiKeyInput(true)}
            className="text-sm text-blue-600 hover:text-blue-700"
          >
            Change
          </button>
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-6">
        <div>
          <label htmlFor="startDate" className="block text-sm font-medium text-gray-700 mb-2">Start Date:</label>
          <input
            type="date"
            id="startDate"
            name="startDate"
            required
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className="w-full px-4 py-3 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>

        <div>
          <label htmlFor="endDate" className="block text-sm font-medium text-gray-700 mb-2">End Date:</label>
          <input
            type="date"
            id="endDate"
            name="endDate"
            required
            value={endDate}
            onChange={(e) => setEndDate(e.target.value)}
            className="w-full px-4 py-3 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>

        <div>
          <button 
            type="submit" 
            disabled={loading || !startDate || !endDate || (!keyFromBackend && !apiKey)}
            className="w-full px-4 py-3 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors font-medium disabled:bg-gray-400 disabled:cursor-not-allowed"
          >
            Generate Report
          </button>
        </div>
      </form>
    </div>
  )
}

