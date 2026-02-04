/**
 * ============================================================================
 * WEEKLY REPORT FORM COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Form component for submitting weekly report generation requests
 * 
 * DESCRIPTION:
 * Provides a form interface for selecting organization and week (Monday-Sunday)
 * to generate weekly reports from the backend API. Automatically loads all users
 * from the selected organization.
 * 
 * ============================================================================
 */

'use client'

import { useState, FormEvent, useEffect } from 'react'
import { getDefaultUser, getDefaultOrganisation, type Organisation, type OrganisationUser } from '@/lib/localTypes'

const GEMINI_API_KEY_STORAGE_KEY = 'gemini_api_key'

interface WeeklyReportFormProps {
  onSubmit: (data: {
    accountId: number
    users: Array<{ name: string; id: number }>
    org: string
    orgId: number
    weekStartDate: string
    geminiApiKey: string
  }) => void
}

/**
 * Gets the Monday of the week containing the given date
 * 
 * INPUTS:
 * - date: Date - Any date in the week
 * 
 * OUTPUTS:
 * - Date - Monday of that week
 */
function getMondayOfWeek(date: Date): Date {
  const d = new Date(date) // Create a copy to avoid mutation
  const day = d.getDay()
  const diff = d.getDate() - day + (day === 0 ? -6 : 1) // Adjust when day is Sunday
  return new Date(d.setDate(diff))
}

/**
 * Gets all Mondays for weeks in the current year
 * 
 * OUTPUTS:
 * - Date[] - Array of Monday dates for each week
 */
function getWeeksInCurrentYear(): Date[] {
  const year = new Date().getFullYear()
  const weeks: Date[] = []
  
  // Start from January 1st
  let currentDate = new Date(year, 0, 1)
  
  // Find the first Monday of the year (or the Monday of the week containing Jan 1)
  const firstMonday = getMondayOfWeek(new Date(currentDate))
  
  // Generate all weeks until we're past December 31st
  let weekStart = new Date(firstMonday)
  while (weekStart.getFullYear() === year || weekStart.getFullYear() === year - 1) {
    if (weekStart.getFullYear() === year) {
      weeks.push(new Date(weekStart))
    }
    
    // Move to next Monday
    weekStart = new Date(weekStart)
    weekStart.setDate(weekStart.getDate() + 7)
    
    // Safety check to prevent infinite loop
    if (weeks.length > 60) break
  }
  
  return weeks
}

/**
 * Formats a week range as "Mon DD - Sun DD, MMM YYYY"
 * 
 * INPUTS:
 * - monday: Date - Monday of the week
 * 
 * OUTPUTS:
 * - string - Formatted week range
 */
function formatWeekRange(monday: Date): string {
  const sunday = new Date(monday)
  sunday.setDate(sunday.getDate() + 6)
  
  const mondayStr = monday.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })
  const sundayStr = sunday.toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })
  
  return `${mondayStr} - ${sundayStr}`
}

export default function WeeklyReportForm({ onSubmit }: WeeklyReportFormProps) {
  const [accountId, setAccountId] = useState<number | null>(null)
  const [organisations, setOrganisations] = useState<Organisation[]>([])
  const [selectedOrgId, setSelectedOrgId] = useState<string>('')
  const [selectedWeekStart, setSelectedWeekStart] = useState<Date | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadingUsers, setLoadingUsers] = useState(false)
  const [error, setError] = useState('')
  const [weeks, setWeeks] = useState<Date[]>([])
  const [apiKey, setApiKey] = useState<string>('')
  const [showApiKeyInput, setShowApiKeyInput] = useState(false)
  const [apiKeyError, setApiKeyError] = useState<string>('')

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

        // Use default organization for local/bundled app
        const defaultOrg = getDefaultOrganisation(accountIdValue)
        setOrganisations([defaultOrg])
        setSelectedOrgId('0')

        // Generate weeks for current year
        const yearWeeks = getWeeksInCurrentYear()
        setWeeks(yearWeeks)
        
        // Default to current week
        const today = new Date()
        const currentMonday = getMondayOfWeek(new Date(today))
        setSelectedWeekStart(currentMonday)
      } catch (err) {
        console.error('Failed to load form data:', err)
        // Set defaults for local version
        setAccountId(0)
        const defaultOrg = getDefaultOrganisation(0)
        setOrganisations([defaultOrg])
        setSelectedOrgId('0')
        setError('') // Don't show error for local version
      } finally {
        setLoading(false)
      }
    }

    loadData()

    // Load API key from localStorage
    const storedApiKey = localStorage.getItem(GEMINI_API_KEY_STORAGE_KEY)
    if (storedApiKey) {
      setApiKey(storedApiKey)
    } else {
      // Show API key input if not set
      setShowApiKeyInput(true)
    }
  }, [])

  const handleSaveApiKey = () => {
    if (!apiKey || apiKey.trim() === '') {
      setApiKeyError('Please enter a valid API key')
      return
    }

    // Save to localStorage
    localStorage.setItem(GEMINI_API_KEY_STORAGE_KEY, apiKey.trim())
    setApiKeyError('')
    setShowApiKeyInput(false)
  }

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()

    // Use defaults for local version
    const accountIdValue = accountId ?? 0
    const orgIdValue = 0
    const orgName = 'Local Organization'
    const users: Array<{ name: string; id: number }> = [{
      name: 'Local User',
      id: 0
    }]

    if (!selectedWeekStart) {
      alert('Please select a week')
      return
    }

    // Format week start date as YYYY-MM-DD
    const weekStartDateStr = selectedWeekStart.toISOString().split('T')[0]

    // Get Gemini API key from localStorage (same key used by Chat component)
    const geminiApiKey = localStorage.getItem('gemini_api_key') || ''
    
    if (!geminiApiKey) {
      alert('Please enter your Gemini API key below. Reports require a Gemini API key to generate AI-powered insights.')
      return
    }

    onSubmit({
      accountId: accountIdValue,
      users,
      org: orgName,
      orgId: orgIdValue,
      weekStartDate: weekStartDateStr,
      geminiApiKey,
    })
  }

  if (loading) {
    return (
      <div className="w-full max-w-2xl bg-white rounded-lg shadow-sm border border-gray-200 p-10">
        <h1 className="text-3xl font-semibold text-gray-900 mb-2">Generate Weekly Report</h1>
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
        <h1 className="text-3xl font-semibold text-gray-900 mb-2">Generate Weekly Report</h1>
        <div className="bg-red-50 border border-red-200 rounded-md p-4 mb-4">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="w-full max-w-2xl bg-white rounded-lg shadow-sm border border-gray-200 p-10">
      <h1 className="text-3xl font-semibold text-gray-900 mb-2">Generate Weekly Report</h1>
      <p className="text-sm text-gray-600 mb-8">
        Select the week to generate a comprehensive weekly activity report.
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
              Your API key is stored locally and used to generate AI-powered report insights.
            </p>
          </div>
        </div>
      )}

      {!showApiKeyInput && apiKey && (
        <div className="mb-6 p-3 border border-gray-200 rounded-md bg-gray-50 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <svg className="w-4 h-4 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <span className="text-sm text-gray-700">API key configured</span>
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
          <label htmlFor="week" className="block text-sm font-medium text-gray-700 mb-2">Week:</label>
          <select
            id="week"
            name="week"
            required
            value={selectedWeekStart ? selectedWeekStart.toISOString().split('T')[0] : ''}
            onChange={(e) => {
              const dateStr = e.target.value
              if (dateStr) {
                setSelectedWeekStart(new Date(dateStr))
              }
            }}
            className="w-full px-4 py-3 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 bg-white"
          >
            <option value="">Select a week</option>
            {weeks.map((week, index) => (
              <option key={index} value={week.toISOString().split('T')[0]}>
                {formatWeekRange(week)}
              </option>
            ))}
          </select>
          {selectedWeekStart && (
            <p className="mt-2 text-xs text-gray-500">
              Week: {formatWeekRange(selectedWeekStart)}
            </p>
          )}
        </div>

        <div>
          <button 
            type="submit" 
            disabled={loading || !selectedWeekStart || !apiKey}
            className="w-full px-4 py-3 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors font-medium disabled:bg-gray-400 disabled:cursor-not-allowed flex items-center justify-center gap-2"
          >
            Generate Weekly Report
          </button>
        </div>
      </form>
    </div>
  )
}

