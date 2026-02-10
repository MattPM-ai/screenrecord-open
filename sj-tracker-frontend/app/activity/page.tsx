/**
 * ============================================================================
 * ACTIVITY/TIMELINE PAGE COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Display screen timeline activity for a specific date
 * 
 * DESCRIPTION:
 * This page displays a timeline of screen activity events for a selected date.
 * Shows app usage, productivity scores, and descriptions of activities.
 * 
 * ============================================================================
 */

'use client'

import { useState, useEffect } from 'react'
import { getTimelineEntries, type TimelineEntry } from '@/lib/activityAPI'

export default function ActivityPage() {
  const [timelineEntries, setTimelineEntries] = useState<TimelineEntry[]>([])
  const [selectedDate, setSelectedDate] = useState<Date>(new Date())
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // For open-source local version, use userId=0 and accountId=0
  const userId = 0
  const accountId = 0

  // Fetch timeline entries when date changes
  useEffect(() => {
    const fetchTimeline = async () => {
      setIsLoading(true)
      setError(null)
      try {
        const entries = await getTimelineEntries(userId, accountId, selectedDate)
        setTimelineEntries(entries)
      } catch (err) {
        console.error('Failed to fetch timeline entries:', err)
        setError(err instanceof Error ? err.message : 'Failed to load timeline data')
        setTimelineEntries([])
      } finally {
        setIsLoading(false)
      }
    }
    fetchTimeline()
  }, [selectedDate])

  const formatDate = (date: Date): string => {
    return date.toLocaleDateString('en-US', { 
      weekday: 'long', 
      year: 'numeric', 
      month: 'long', 
      day: 'numeric' 
    })
  }

  const navigateDate = (direction: 'prev' | 'next') => {
    const newDate = new Date(selectedDate)
    newDate.setDate(newDate.getDate() + (direction === 'next' ? 1 : -1))
    setSelectedDate(newDate)
  }

  const getStatusDotColor = (status: TimelineEntry['status']) => {
    switch (status) {
      case 'green':
        return '#10b981'
      case 'blue':
        return '#3b82f6'
      case 'grey':
        return '#9ca3af'
      default:
        return '#9ca3af'
    }
  }

  return (
    <div style={{ 
      minHeight: 'calc(100vh - 64px)', 
      padding: '24px',
      backgroundColor: '#f8f9fa'
    }}>
      <div style={{
        maxWidth: '1200px',
        margin: '0 auto',
        backgroundColor: '#ffffff',
        borderRadius: '8px',
        boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
        overflow: 'hidden'
      }}>
        {/* Header with Date Navigation */}
        <div style={{
          padding: '20px 24px',
          borderBottom: '1px solid #dee2e6',
          backgroundColor: '#f8f9fa',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: '16px'
        }}>
          <div>
            <h1 style={{
              fontSize: '24px',
              fontWeight: 600,
              color: '#212529',
              margin: '0 0 8px 0'
            }}>
              Activity Timeline
            </h1>
            <p style={{
              fontSize: '14px',
              color: '#6c757d',
              margin: 0
            }}>
              {formatDate(selectedDate)}
            </p>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
            <button
              onClick={() => navigateDate('prev')}
              style={{
                padding: '8px 16px',
                backgroundColor: '#ffffff',
                border: '1px solid #dee2e6',
                borderRadius: '6px',
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                fontSize: '14px',
                fontWeight: 500,
                color: '#212529'
              }}
            >
              <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24" style={{ marginRight: '4px' }}>
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
              </svg>
              Previous
            </button>
            <button
              onClick={() => setSelectedDate(new Date())}
              style={{
                padding: '8px 16px',
                backgroundColor: '#0066cc',
                border: 'none',
                borderRadius: '6px',
                cursor: 'pointer',
                fontSize: '14px',
                fontWeight: 500,
                color: '#ffffff'
              }}
            >
              Today
            </button>
            <button
              onClick={() => navigateDate('next')}
              style={{
                padding: '8px 16px',
                backgroundColor: '#ffffff',
                border: '1px solid #dee2e6',
                borderRadius: '6px',
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                fontSize: '14px',
                fontWeight: 500,
                color: '#212529'
              }}
            >
              Next
              <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24" style={{ marginLeft: '4px' }}>
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              </svg>
            </button>
          </div>
        </div>

        {/* Timeline Content */}
        <div style={{
          padding: '24px',
          minHeight: '400px'
        }}>
          {isLoading ? (
            <div style={{ 
              display: 'flex', 
              justifyContent: 'center', 
              alignItems: 'center', 
              height: '400px',
              flexDirection: 'column',
              gap: '16px'
            }}>
              <div style={{
                width: '40px',
                height: '40px',
                border: '4px solid #f1f3f5',
                borderTop: '4px solid #0066cc',
                borderRadius: '50%',
                animation: 'spin 1s linear infinite'
              }}></div>
              <div style={{ fontSize: '14px', color: '#6c757d' }}>Loading timeline data...</div>
            </div>
          ) : error ? (
            <div style={{
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'center',
              height: '400px',
              flexDirection: 'column',
              gap: '8px'
            }}>
              <div style={{ fontSize: '16px', color: '#dc3545', fontWeight: 500 }}>Error loading timeline</div>
              <div style={{ fontSize: '14px', color: '#6c757d' }}>{error}</div>
            </div>
          ) : timelineEntries.length > 0 ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
              {timelineEntries.map((entry) => (
                <div
                  key={entry.id}
                  style={{
                    display: 'flex',
                    gap: '16px',
                    alignItems: 'flex-start',
                    padding: '16px',
                    backgroundColor: '#f8f9fa',
                    borderRadius: '8px',
                    border: '1px solid #dee2e6'
                  }}
                >
                  <div style={{
                    width: '12px',
                    height: '12px',
                    borderRadius: '50%',
                    backgroundColor: getStatusDotColor(entry.status),
                    marginTop: '6px',
                    flexShrink: 0
                  }} />
                  <div style={{ flex: 1 }}>
                    <div style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '12px',
                      marginBottom: '8px',
                      flexWrap: 'wrap'
                    }}>
                      <h3 style={{
                        fontSize: '16px',
                        fontWeight: 600,
                        color: '#212529',
                        margin: 0
                      }}>
                        {entry.title}
                      </h3>
                      <span style={{
                        fontSize: '12px',
                        padding: '4px 8px',
                        backgroundColor: '#e7f3ff',
                        borderRadius: '4px',
                        color: '#0066cc',
                        fontWeight: 500
                      }}>
                        {entry.app}
                      </span>
                      <span style={{
                        fontSize: '12px',
                        padding: '4px 8px',
                        backgroundColor: '#f1f3f5',
                        borderRadius: '4px',
                        color: '#6c757d',
                        fontWeight: 500
                      }}>
                        {entry.type}
                      </span>
                    </div>
                    {entry.description && (
                      <p style={{
                        fontSize: '14px',
                        color: '#6c757d',
                        margin: '8px 0',
                        lineHeight: '1.5'
                      }}>
                        {entry.description}
                      </p>
                    )}
                    <div style={{
                      fontSize: '13px',
                      color: '#6c757d',
                      marginTop: '8px'
                    }}>
                      {entry.startTime} - {entry.endTime}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div style={{
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'center',
              height: '400px',
              flexDirection: 'column',
              gap: '8px'
            }}>
              <div style={{ fontSize: '16px', color: '#6c757d', fontWeight: 500 }}>No timeline entries available</div>
              <div style={{ fontSize: '14px', color: '#6c757d' }}>No activity data found for this date.</div>
            </div>
          )}
        </div>
      </div>

      <style jsx>{`
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  )
}
