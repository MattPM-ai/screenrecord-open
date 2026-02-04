/**
 * ============================================================================
 * WEEKLY REPORTS EMAIL SETTINGS COMPONENT
 * ============================================================================
 * 
 * PURPOSE: UI component for managing weekly reports email opt-in/opt-out
 * 
 * DESCRIPTION:
 * Allows account owners to configure weekly report email subscriptions.
 * Users can select an organization, email address, and users to include.
 * 
 * ============================================================================
 */

'use client'

import { useState, useEffect, FormEvent } from 'react'
import { getDefaultOrganisation, type Organisation, type OrganisationUser } from '@/lib/localTypes'
import { optInWeeklyReports, optOutWeeklyReports, getOptedInAccounts, OptedInAccount } from '@/lib/weeklyReportsAPI'

interface WeeklyReportsEmailSettingsProps {
  accountId: number
  ownerEmail: string
}

export default function WeeklyReportsEmailSettings({ accountId, ownerEmail }: WeeklyReportsEmailSettingsProps) {
  const [organisations, setOrganisations] = useState<Organisation[]>([])
  const [selectedOrgId, setSelectedOrgId] = useState<string>('')
  const [users, setUsers] = useState<OrganisationUser[]>([])
  const [email, setEmail] = useState('')
  const [nextTriggerTime, setNextTriggerTime] = useState('')
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [loading, setLoading] = useState(true)
  const [loadingUsers, setLoadingUsers] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')
  const [isOptedIn, setIsOptedIn] = useState(false)
  const [currentOptedInOrgId, setCurrentOptedInOrgId] = useState<number | null>(null)
  const [optedInAccounts, setOptedInAccounts] = useState<OptedInAccount[]>([])
  const [loadingOptedIn, setLoadingOptedIn] = useState(false)

  // Load organizations and opted-in accounts
  useEffect(() => {
    const loadData = async () => {
      try {
        setLoading(true)
        setError('')
        
        // Use default organization for local/bundled app (no auth backend)
        setOrganisations([getDefaultOrganisation(accountId)])
        
        // Load opted-in accounts
        setLoadingOptedIn(true)
        try {
          const optedInResponse = await getOptedInAccounts(accountId)
          setOptedInAccounts(optedInResponse.accounts || [])
        } catch (err) {
          console.error('Failed to load opted-in accounts:', err)
          // Don't show error for this, just log it
        } finally {
          setLoadingOptedIn(false)
        }
      } catch (err) {
        console.error('Failed to load organizations:', err)
        setError(err instanceof Error ? err.message : 'Failed to load organizations')
      } finally {
        setLoading(false)
      }
    }

    loadData()
  }, [accountId])

  // Load all users for selected organization (silently, no UI)
  useEffect(() => {
    const loadOrgUsers = async () => {
      if (!selectedOrgId || selectedOrgId === '') {
        setUsers([])
        return
      }

      try {
        setLoadingUsers(true)
        const orgId = Number(selectedOrgId)
        if (!isNaN(orgId)) {
          // For local/bundled app, no users available (no auth backend)
          const allUsers: OrganisationUser[] = []

          setUsers(allUsers)
        }
      } catch (err) {
        console.error('Failed to load organization users:', err)
        setError(err instanceof Error ? err.message : 'Failed to load organization users')
        setUsers([])
      } finally {
        setLoadingUsers(false)
      }
    }

    loadOrgUsers()
  }, [selectedOrgId])

  const handleOptIn = async (e: FormEvent) => {
    e.preventDefault()

    if (!selectedOrgId || selectedOrgId === '') {
      setError('Please select an organization')
      return
    }

    if (users.length === 0 && !loadingUsers) {
      setError('No users found in this organization')
      return
    }

    const selectedOrg = organisations.find(org => String(org.id) === String(selectedOrgId))
    if (!selectedOrg) {
      setError('Selected organization not found')
      return
    }

    try {
      setSubmitting(true)
      setError('')
      setSuccess('')

      const orgIdNum = typeof selectedOrg.id === 'string' ? Number(selectedOrg.id) : selectedOrg.id
      if (isNaN(orgIdNum)) {
        throw new Error('Invalid organization ID')
      }

      // Build users array - always include ALL users
      const usersArray = users
        .map(user => {
          const userIdNum = typeof user.id === 'string' ? Number(user.id) : user.id
          if (isNaN(userIdNum)) {
            return null
          }
          
          return {
            name: user.name || user.email,
            id: userIdNum
          }
        })
        .filter((user): user is { name: string; id: number } => user !== null)

      if (usersArray.length === 0) {
        throw new Error('No valid users found in organization')
      }

      // Use provided email or default to owner's email
      const emailToUse = email && email.trim() ? email.trim() : ownerEmail

      // Convert datetime-local to ISO 8601 UTC format if provided
      let nextTriggerTimeISO: string | undefined = undefined
      if (nextTriggerTime && nextTriggerTime.trim()) {
        // datetime-local format is "YYYY-MM-DDTHH:mm" in local time
        // Convert to ISO 8601 UTC format
        const localDate = new Date(nextTriggerTime)
        nextTriggerTimeISO = localDate.toISOString()
      }

      const optInData = {
        accountId: Number(accountId),
        orgId: orgIdNum,
        orgName: selectedOrg.name,
        email: emailToUse,
        users: usersArray,
        ...(nextTriggerTimeISO ? { nextTriggerTime: nextTriggerTimeISO } : {})
      }

      await optInWeeklyReports(optInData)
      
      setSuccess('Successfully opted in to weekly reports!')
      setIsOptedIn(true)
      setCurrentOptedInOrgId(orgIdNum)
      
      // Reload opted-in accounts
      try {
        const optedInResponse = await getOptedInAccounts(accountId)
        setOptedInAccounts(optedInResponse.accounts || [])
      } catch (err) {
        console.error('Failed to reload opted-in accounts:', err)
      }
      
      // Clear form
      setEmail('')
      setNextTriggerTime('')
      setShowAdvanced(false)
    } catch (err) {
      console.error('Failed to opt in:', err)
      setError(err instanceof Error ? err.message : 'Failed to opt in to weekly reports')
    } finally {
      setSubmitting(false)
    }
  }

  const handleOptOut = async (orgId?: number) => {
    const orgIdToOptOut = orgId || currentOptedInOrgId
    if (!orgIdToOptOut) {
      setError('No active subscription found')
      return
    }

    if (!confirm('Are you sure you want to opt out of weekly reports for this organization?')) {
      return
    }

    try {
      setSubmitting(true)
      setError('')
      setSuccess('')

      await optOutWeeklyReports({
        accountId: Number(accountId),
        orgId: orgIdToOptOut
      })

      setSuccess('Successfully opted out of weekly reports')
      setIsOptedIn(false)
      setCurrentOptedInOrgId(null)
      setSelectedOrgId('')
      setUsers([])
      setEmail('')
      setNextTriggerTime('')
      setShowAdvanced(false)
      
      // Reload opted-in accounts
      try {
        const optedInResponse = await getOptedInAccounts(accountId)
        setOptedInAccounts(optedInResponse.accounts || [])
      } catch (err) {
        console.error('Failed to reload opted-in accounts:', err)
      }
    } catch (err) {
      console.error('Failed to opt out:', err)
      setError(err instanceof Error ? err.message : 'Failed to opt out of weekly reports')
    } finally {
      setSubmitting(false)
    }
  }


  if (loading) {
    return (
      <div className="p-6 bg-white rounded-lg border border-gray-200">
        <h4 className="text-lg font-semibold text-gray-900 mb-4">Weekly Reports Email Settings</h4>
        <div className="flex items-center justify-center py-8">
          <div className="w-6 h-6 border-2 border-blue-600 border-t-transparent rounded-full animate-spin"></div>
        </div>
      </div>
    )
  }

  return (
    <div className="p-6 bg-white rounded-lg border border-gray-200">
      <h4 className="text-lg font-semibold text-gray-900 mb-4">Weekly Reports Email Settings</h4>
      <p className="text-sm text-gray-600 mb-6">
        Configure automatic weekly report emails. Reports will be sent every Monday at 00:00 UTC.
      </p>

      {error && (
        <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded-md text-sm text-red-700">
          {error}
        </div>
      )}

      {success && (
        <div className="mb-4 p-3 bg-green-50 border border-green-200 rounded-md text-sm text-green-700">
          {success}
        </div>
      )}

      {/* Display Opted-In Accounts */}
      {optedInAccounts.length > 0 && (
        <div className="mb-6">
          <h5 className="text-md font-semibold text-gray-900 mb-3">Currently Opted-In Organizations</h5>
          <div className="space-y-3">
            {optedInAccounts.map((account) => (
              <div key={account.orgId} className="p-4 bg-blue-50 border border-blue-200 rounded-md">
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-2">
                      <h6 className="text-sm font-semibold text-blue-900">{account.orgName}</h6>
                      <span className="text-xs text-blue-600">(ID: {account.orgId})</span>
                    </div>
                    <div className="text-xs text-blue-700 space-y-1">
                      <p><strong>Email:</strong> {account.email}</p>
                      <p><strong>Users:</strong> {account.users.map(u => u.name).join(', ')}</p>
                      <p><strong>Opted in:</strong> {new Date(account.optedInAt).toLocaleString()}</p>
                      {account.nextTriggerTime && (
                        <p><strong>Next trigger:</strong> {new Date(account.nextTriggerTime).toLocaleString()}</p>
                      )}
                    </div>
                  </div>
                  <button
                    onClick={() => handleOptOut(account.orgId)}
                    disabled={submitting}
                    className={`ml-4 px-3 py-1.5 rounded text-xs font-medium transition-colors ${
                      submitting
                        ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                        : 'bg-red-600 text-white hover:bg-red-700'
                    }`}
                  >
                    {submitting ? 'Processing...' : 'Opt Out'}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {loadingOptedIn && (
        <div className="mb-4 flex items-center justify-center py-2">
          <div className="w-4 h-4 border-2 border-blue-600 border-t-transparent rounded-full animate-spin"></div>
          <span className="ml-2 text-xs text-gray-500">Loading opted-in accounts...</span>
        </div>
      )}

      {/* Opt-In Form */}
      <div className={optedInAccounts.length > 0 ? 'mt-6 pt-6 border-t border-gray-200' : ''}>
        {optedInAccounts.length > 0 && (
          <h5 className="text-md font-semibold text-gray-900 mb-4">Opt In Another Organization</h5>
        )}
          <form onSubmit={handleOptIn} className="space-y-4">
            <div>
              <label htmlFor="org" className="block text-sm font-medium text-gray-700 mb-2">
                Organization:
              </label>
              <select
                id="org"
                name="org"
                required
                value={selectedOrgId}
                onChange={(e) => setSelectedOrgId(e.target.value)}
                className="w-full px-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 bg-white"
              >
                <option value="">Select an organization</option>
                {organisations
                  .filter(org => {
                    // Filter out already opted-in organizations
                    const orgIdNum = typeof org.id === 'string' ? Number(org.id) : org.id
                    return !optedInAccounts.some(acc => acc.orgId === orgIdNum)
                  })
                  .map((org) => (
                    <option key={org.id} value={org.id}>
                      {org.name}
                    </option>
                  ))}
              </select>
              {selectedOrgId && loadingUsers && (
                <p className="mt-2 text-xs text-gray-500">Loading users...</p>
              )}
              {selectedOrgId && !loadingUsers && users.length > 0 && (
                <p className="mt-2 text-xs text-gray-500">
                  Reports will include all {users.length} user{users.length !== 1 ? 's' : ''} in this organization
                </p>
              )}
            </div>

            {/* Advanced Settings - Expandable */}
            <div>
              <button
                type="button"
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="flex items-center justify-between w-full px-4 py-2 bg-gray-100 text-gray-700 rounded-md text-sm font-medium hover:bg-gray-200 transition-colors"
              >
                <span>Advanced Settings</span>
                <svg
                  className={`w-5 h-5 text-blue-600 transition-transform ${showAdvanced ? 'transform rotate-180' : ''}`}
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>

              {showAdvanced && (
                <div className="mt-4 space-y-4 pl-4 border-l-2 border-gray-200">
                  <div>
                    <label htmlFor="email" className="block text-sm font-medium text-gray-700 mb-2">
                      Email Address (Optional):
                    </label>
                    <input
                      type="email"
                      id="email"
                      name="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder={ownerEmail}
                      className="w-full px-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 bg-white"
                    />
                    <p className="mt-1 text-xs text-gray-500">
                      Leave empty to use your email ({ownerEmail}). Weekly reports will be sent to this address.
                    </p>
                  </div>

                  <div>
                    <label htmlFor="nextTriggerTime" className="block text-sm font-medium text-gray-700 mb-2">
                      Trigger Date/Time (Optional - defines when the report will be sent):
                    </label>
                    <input
                      type="datetime-local"
                      id="nextTriggerTime"
                      name="nextTriggerTime"
                      value={nextTriggerTime}
                      onChange={(e) => setNextTriggerTime(e.target.value)}
                      className="w-full px-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 bg-white"
                    />
                    <p className="mt-1 text-xs text-gray-500">
                      Leave empty to use default (Monday 00:00 UTC).
                    </p>
                  </div>
                </div>
              )}
            </div>

            <div>
              <button
                type="submit"
                disabled={submitting || !selectedOrgId || loadingUsers}
                className={`w-full px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                  submitting || !selectedOrgId || loadingUsers
                    ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                    : 'bg-blue-600 text-white hover:bg-blue-700'
                }`}
              >
                {submitting ? 'Submitting...' : 'Opt In to Weekly Reports'}
              </button>
            </div>
          </form>
        </div>
    </div>
  )
}

