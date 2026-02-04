/**
 * ============================================================================
 * WEEKLY REPORT LOADING COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Displays loading state and handles polling for weekly report generation
 * 
 * DESCRIPTION:
 * This component shows a loading spinner, timer, and polling status while
 * waiting for the backend to generate the weekly report. It polls the status endpoint
 * every 5 seconds until the report is ready.
 * 
 * DEPENDENCIES:
 * - /app/api/reports/generate-weekly/route.ts: Initiates weekly report generation
 * - /app/api/reports/status/route.ts: Polls for report status
 * 
 * ============================================================================
 */

'use client'

import { useEffect, useState, useRef } from 'react'

interface WeeklyReportLoadingProps {
  formData: {
    accountId: number
    users: Array<{ name: string; id: number }>
    org: string
    orgId: number
    weekStartDate: string
  }
  onComplete: (data: any) => void
  onError: (error: string) => void
}

const BACKEND_URL = 'http://localhost:8085'

export default function WeeklyReportLoading({ formData, onComplete, onError }: WeeklyReportLoadingProps) {
  const [timer, setTimer] = useState('00:00:00')
  const [status, setStatus] = useState('Initializing...')
  
  const timerStartTimeRef = useRef<number | null>(null)
  const timerIntervalRef = useRef<NodeJS.Timeout | null>(null)
  const pollingIntervalRef = useRef<NodeJS.Timeout | null>(null)

  /**
   * Starts the timer
   */
  const startTimer = () => {
    timerStartTimeRef.current = Date.now()
    timerIntervalRef.current = setInterval(() => {
      if (timerStartTimeRef.current) {
        const elapsed = Date.now() - timerStartTimeRef.current
        const seconds = Math.floor((elapsed / 1000) % 60)
        const minutes = Math.floor((elapsed / 1000 / 60) % 60)
        const hours = Math.floor(elapsed / 1000 / 60 / 60)
        
        setTimer(
          `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
        )
      }
    }, 100)
  }

  /**
   * Stops the timer
   */
  const stopTimer = () => {
    if (timerIntervalRef.current) {
      clearInterval(timerIntervalRef.current)
      timerIntervalRef.current = null
    }
  }

  /**
   * Stops polling
   */
  const stopPolling = () => {
    if (pollingIntervalRef.current) {
      clearInterval(pollingIntervalRef.current)
      pollingIntervalRef.current = null
    }
  }

  /**
   * Polls the status endpoint to check if report is ready
   */
  const pollReportStatus = async (taskId: string) => {
    try {
      const response = await fetch(`/api/reports/status/${taskId}`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      })

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`)
      }

      const statusData = await response.json()

      if (statusData.status === 'completed') {
        stopPolling()
        stopTimer()

        if (statusData.report) {
          onComplete(statusData.report)
        } else {
          throw new Error('Report data not found in response')
        }
      } else if (statusData.status === 'pending') {
        setStatus(`Status: ${statusData.status}... (Task ID: ${taskId.substring(0, 8)}...)`)
      } else {
        setStatus(`Status: ${statusData.status}`)
      }
    } catch (error) {
      console.error('Error polling report status:', error)
      stopPolling()
      stopTimer()
      onError(`Error polling report status: ${error instanceof Error ? error.message : 'Unknown error'}`)
    }
  }

  /**
   * Initiates weekly report generation
   */
  const generateReport = async (data: {
    accountId: number
    users: Array<{ name: string; id: number }>
    org: string
    orgId: number
    weekStartDate: string
  }) => {
    setStatus('Submitting weekly report generation request...')
    startTimer()

    try {
      const response = await fetch('/api/reports/generate-weekly', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(data),
      })

      if (!response.ok) {
        const errorText = await response.text().catch(() => 'Unknown error')
        throw new Error(`HTTP error! status: ${response.status} - ${errorText}`)
      }

      const generateData = await response.json()

      if (generateData.taskId && generateData.status) {
        setStatus(`Weekly report generation started. Task ID: ${generateData.taskId.substring(0, 8)}...`)

        // Start polling every 5 seconds
        pollingIntervalRef.current = setInterval(() => {
          pollReportStatus(generateData.taskId)
        }, 5000)

        // Poll immediately
        pollReportStatus(generateData.taskId)
      } else {
        throw new Error('Invalid response from generation endpoint. Missing taskId or status.')
      }
    } catch (error) {
      stopTimer()
      stopPolling()

      let errorMessage = error instanceof Error ? error.message : 'Unknown error'
      if (errorMessage === 'Failed to fetch') {
        errorMessage = `Failed to connect to backend at ${BACKEND_URL}. Please check:
- Is the backend server running?
- Is the URL correct?
- Are there CORS issues? (Check browser console for details)`
      }

      onError(errorMessage)
    }
  }

  useEffect(() => {
    // Generate report when component mounts with form data
    if (formData) {
      generateReport(formData)
    }

    return () => {
      stopTimer()
      stopPolling()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="w-full max-w-2xl bg-white rounded-lg shadow-sm border border-gray-200 p-10">
      <h1 className="text-3xl font-semibold text-gray-900 mb-8">Generating Weekly Report...</h1>
      <div className="flex flex-col items-center justify-center space-y-6">
        <div className="w-16 h-16 border-4 border-gray-200 border-t-blue-600 rounded-full animate-spin"></div>
        <div className="text-4xl font-mono font-semibold text-gray-900">{timer}</div>
        <div className="text-sm text-gray-600 text-center">{status}</div>
      </div>
    </div>
  )
}


