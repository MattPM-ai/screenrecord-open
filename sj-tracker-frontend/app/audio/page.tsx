/**
 * ============================================================================
 * AUDIO TRANSCRIPTS PAGE COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Display audio transcript entries from the backend
 * 
 * DESCRIPTION:
 * This page displays audio transcripts grouped by audio URL. Each group has
 * an audio player and a table of transcripts. Users can select a user and
 * optionally filter by date.
 * 
 * ============================================================================
 */

'use client'

import { useState, useEffect } from 'react'
import { getAudioTranscripts, type AudioTranscriptGroup, type AudioTranscript } from '@/lib/audioAPI'

export default function AudioPage() {
  const [selectedUserId, setSelectedUserId] = useState<number>(0) // 0 for local version
  const [selectedDate, setSelectedDate] = useState<Date | null>(null)
  const [transcriptGroups, setTranscriptGroups] = useState<AudioTranscriptGroup[]>([])
  const [expandedGroups, setExpandedGroups] = useState<Set<number>>(new Set())
  const [isLoadingTranscripts, setIsLoadingTranscripts] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Fetch transcripts when user or date changes
  useEffect(() => {
    if (selectedUserId !== null) {
      const fetchTranscripts = async () => {
        try {
          setIsLoadingTranscripts(true)
          setError(null)
          console.log('[AudioPage] Fetching transcripts for userId:', selectedUserId, 'date:', selectedDate)
          const data = await getAudioTranscripts(
            selectedUserId,
            0, // accountId - 0 for local version
            undefined, // orgId - optional
            selectedDate || undefined
          )
          console.log('[AudioPage] Received transcript groups:', data.length, 'groups')
          console.log('[AudioPage] Full API Response:', JSON.stringify(data, null, 2))
          
          // Log detailed information about each group
          data.forEach((group, index) => {
            console.log(`[AudioPage] ===== Group ${index} =====`)
            console.log(`[AudioPage] Group ${index} audioUrl:`, group.audioUrl, 'Type:', typeof group.audioUrl, 'Length:', group.audioUrl?.length || 0)
            console.log(`[AudioPage] Group ${index} transcript count:`, group.transcripts.length)
            
            // Log the first few transcripts to see their fields
            if (group.transcripts.length > 0) {
              console.log(`[AudioPage] Group ${index} first transcript fields:`, group.transcripts[0].fields)
              console.log(`[AudioPage] Group ${index} first transcript audio_url field:`, group.transcripts[0].fields?.audio_url)
              
              // Check all transcripts for audio_url
              const transcriptsWithAudio = group.transcripts.filter(t => t.fields?.audio_url)
              const transcriptsWithoutAudio = group.transcripts.filter(t => !t.fields?.audio_url)
              console.log(`[AudioPage] Group ${index} transcripts WITH audio_url:`, transcriptsWithAudio.length)
              console.log(`[AudioPage] Group ${index} transcripts WITHOUT audio_url:`, transcriptsWithoutAudio.length)
              
              if (transcriptsWithAudio.length > 0) {
                console.log(`[AudioPage] Group ${index} sample audio_url values:`, 
                  transcriptsWithAudio.slice(0, 3).map(t => t.fields?.audio_url)
                )
              }
            }
          })
          
          setTranscriptGroups(data)
        } catch (err) {
          console.error('[AudioPage] Failed to fetch transcripts:', err)
          setError(err instanceof Error ? err.message : 'Failed to load transcripts')
          setTranscriptGroups([])
        } finally {
          setIsLoadingTranscripts(false)
        }
      }
      fetchTranscripts()
    }
  }, [selectedUserId, selectedDate])

  // Format time for display
  const formatTime = (timeString: string): string => {
    try {
      const date = new Date(timeString)
      return date.toLocaleString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        timeZoneName: 'short'
      })
    } catch {
      return timeString
    }
  }

  // Format duration from milliseconds
  const formatDuration = (ms: number): string => {
    const seconds = Math.floor(ms / 1000)
    const minutes = Math.floor(seconds / 60)
    const remainingSeconds = seconds % 60
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`
  }

  // Format date for input (YYYY-MM-DD)
  const formatDateForInput = (date: Date | null): string => {
    if (!date) return ''
    return date.toISOString().split('T')[0]
  }

  // Handle date input change
  const handleDateChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.value) {
      setSelectedDate(new Date(e.target.value))
    } else {
      setSelectedDate(null)
    }
  }

  // Toggle group expansion
  const toggleGroup = (groupIndex: number) => {
    setExpandedGroups((prev) => {
      const newSet = new Set(prev)
      if (newSet.has(groupIndex)) {
        newSet.delete(groupIndex)
      } else {
        newSet.add(groupIndex)
      }
      return newSet
    })
  }

  // Convert local file path to a URL that can be served
  const getAudioUrl = (audioPath: string): string => {
    console.log('[getAudioUrl] Input audioPath:', audioPath, 'Type:', typeof audioPath)
    
    // Handle null/undefined/empty
    if (!audioPath || audioPath === '') {
      console.warn('[getAudioUrl] Empty audioPath provided')
      return ''
    }
    
    try {
      // If it's already a URL, return it
      if (audioPath.startsWith('http://') || audioPath.startsWith('https://')) {
        console.log('[getAudioUrl] Already a full URL, returning as-is')
        return audioPath
      }
      
      // If it's already a relative path to our API endpoint, use it as-is
      // The Next.js API route will proxy it to the backend
      if (audioPath.startsWith('/api/audio-file')) {
        console.log('[getAudioUrl] API endpoint path, using as-is (Next.js will proxy):', audioPath)
        return audioPath
      }
      
      // For local paths, convert to the serving endpoint
      // The backend should have already converted these, but handle it here as fallback
      const backendUrl = process.env.NEXT_PUBLIC_BACKEND_URL || ''
      const encodedPath = encodeURIComponent(audioPath)
      const fullUrl = `${backendUrl}/api/audio-file?path=${encodedPath}`
      console.log('[getAudioUrl] Local path converted, backendUrl:', backendUrl, 'encodedPath:', encodedPath, 'fullUrl:', fullUrl)
      return fullUrl
    } catch (error) {
      console.error('[getAudioUrl] Error processing audioPath:', error, 'audioPath:', audioPath)
      return ''
    }
  }

  return (
    <div style={{
      padding: '24px',
      maxWidth: '100%',
      overflowX: 'auto'
    }}>
      <h1 style={{
        fontSize: '24px',
        fontWeight: '600',
        color: '#1f2937',
        marginBottom: '24px'
      }}>
        Audio Transcripts
      </h1>

      {/* Filters */}
      <div style={{
        marginBottom: '32px',
        backgroundColor: '#ffffff',
        border: '1px solid #e5e7eb',
        borderRadius: '12px',
        boxShadow: '0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px 0 rgba(0, 0, 0, 0.06)',
        overflow: 'hidden'
      }}>
        {/* Blue Header Bar */}
        <div style={{
          backgroundColor: '#0066cc',
          padding: '12px 20px',
          display: 'flex',
          alignItems: 'center',
          gap: '8px'
        }}>
          <svg
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            style={{ color: '#ffffff' }}
          >
            <path
              d="M3 4C3 3.44772 3.44772 3 4 3H16C16.5523 3 17 3.44772 17 4V6C17 6.55228 16.5523 7 16 7H4C3.44772 7 3 6.55228 3 6V4Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M3 10C3 9.44772 3.44772 9 4 9H10C10.5523 9 11 9.44772 11 10V16C11 16.5523 10.5523 17 10 17H4C3.44772 17 3 16.5523 3 16V10Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M14 9C14.5523 9 15 9.44772 15 10V16C15 16.5523 14.5523 17 14 17C13.4477 17 13 16.5523 13 16V10C13 9.44772 13.4477 9 14 9Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M17 9C17.5523 9 18 9.44772 18 10V16C18 16.5523 17.5523 17 17 17C16.4477 17 16 16.5523 16 16V10C16 9.44772 16.4477 9 17 9Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <h2 style={{
            fontSize: '16px',
            fontWeight: '600',
            color: '#ffffff',
            margin: 0
          }}>
            Filters
          </h2>
        </div>
        
        {/* Filter Content */}
        <div style={{
          padding: '20px'
        }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
          gap: '16px'
        }}>
          {/* User Filter - For local version, just show user ID 0 */}
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px'
          }}>
            <label style={{
              fontSize: '13px',
              fontWeight: '500',
              color: '#6b7280',
              textTransform: 'uppercase',
              letterSpacing: '0.05em'
            }}>
              User ID
            </label>
            <input
              type="number"
              value={selectedUserId}
              onChange={(e) => setSelectedUserId(parseInt(e.target.value) || 0)}
              disabled={isLoadingTranscripts}
              style={{
                padding: '10px 14px',
                fontSize: '14px',
                border: '1px solid #d1d5db',
                borderRadius: '8px',
                backgroundColor: '#ffffff',
                color: '#1f2937',
                cursor: isLoadingTranscripts ? 'not-allowed' : 'text',
                transition: 'all 0.2s',
                outline: 'none'
              }}
              onFocus={(e) => {
                e.currentTarget.style.borderColor = '#3b82f6'
                e.currentTarget.style.boxShadow = '0 0 0 3px rgba(59, 130, 246, 0.1)'
              }}
              onBlur={(e) => {
                e.currentTarget.style.borderColor = '#d1d5db'
                e.currentTarget.style.boxShadow = 'none'
              }}
            />
          </div>

          {/* Date Filter */}
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px'
          }}>
            <label style={{
              fontSize: '13px',
              fontWeight: '500',
              color: '#6b7280',
              textTransform: 'uppercase',
              letterSpacing: '0.05em'
            }}>
              Date <span style={{ color: '#9ca3af', fontWeight: '400' }}>(optional)</span>
            </label>
            <div style={{
              display: 'flex',
              gap: '8px',
              alignItems: 'flex-end'
            }}>
              <input
                type="date"
                value={formatDateForInput(selectedDate)}
                onChange={handleDateChange}
                disabled={isLoadingTranscripts}
                style={{
                  flex: 1,
                  padding: '10px 14px',
                  fontSize: '14px',
                  border: '1px solid #d1d5db',
                  borderRadius: '8px',
                  backgroundColor: '#ffffff',
                  color: '#1f2937',
                  cursor: isLoadingTranscripts ? 'not-allowed' : 'pointer',
                  transition: 'all 0.2s',
                  outline: 'none'
                }}
                onFocus={(e) => {
                  e.currentTarget.style.borderColor = '#3b82f6'
                  e.currentTarget.style.boxShadow = '0 0 0 3px rgba(59, 130, 246, 0.1)'
                }}
                onBlur={(e) => {
                  e.currentTarget.style.borderColor = '#d1d5db'
                  e.currentTarget.style.boxShadow = 'none'
                }}
              />
              {selectedDate && (
                <button
                  onClick={() => setSelectedDate(null)}
                  style={{
                    padding: '10px 12px',
                    fontSize: '14px',
                    border: '1px solid #d1d5db',
                    borderRadius: '8px',
                    backgroundColor: '#f9fafb',
                    color: '#6b7280',
                    cursor: 'pointer',
                    transition: 'all 0.2s',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center'
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = '#f3f4f6'
                    e.currentTarget.style.borderColor = '#9ca3af'
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = '#f9fafb'
                    e.currentTarget.style.borderColor = '#d1d5db'
                  }}
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 4L4 12M4 4L12 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </button>
              )}
            </div>
          </div>
        </div>
        </div>
      </div>

      {/* Error Message */}
      {error && (
        <div style={{
          padding: '12px 16px',
          backgroundColor: '#fee2e2',
          border: '1px solid #fecaca',
          borderRadius: '6px',
          color: '#991b1b',
          marginBottom: '24px'
        }}>
          {error}
        </div>
      )}

      {/* Loading State */}
      {isLoadingTranscripts && (
        <div style={{
          padding: '40px',
          textAlign: 'center'
        }}>
          <div style={{
            width: '48px',
            height: '48px',
            border: '4px solid #e2e8f0',
            borderTopColor: '#3b82f6',
            borderRadius: '50%',
            animation: 'spin 1s linear infinite',
            margin: '0 auto 16px auto'
          }}></div>
          <div style={{ fontSize: '16px', color: '#212529' }}>Loading transcripts...</div>
        </div>
      )}

      {/* Empty State */}
      {!isLoadingTranscripts && transcriptGroups.length === 0 && selectedUserId !== null && (
        <div style={{
          padding: '40px',
          textAlign: 'center',
          color: '#6c757d'
        }}>
          No transcripts found for this user{selectedDate ? ` on ${formatDateForInput(selectedDate)}` : ' (last 30 days)'}.
        </div>
      )}

      {/* Transcript Groups */}
      {!isLoadingTranscripts && transcriptGroups.length > 0 && (
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '32px'
        }}>
          {transcriptGroups.map((group, groupIndex) => (
            <div
              key={`group-${groupIndex}-${group.audioUrl}`}
              style={{
                border: '1px solid #e5e7eb',
                borderRadius: '8px',
                backgroundColor: '#ffffff',
                overflow: 'hidden'
              }}
            >
              {/* Group Header with Audio Player */}
              <div style={{
                padding: '16px',
                backgroundColor: '#f9fafb',
                borderBottom: expandedGroups.has(groupIndex) ? '1px solid #e5e7eb' : 'none'
              }}>
                <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  marginBottom: expandedGroups.has(groupIndex) ? '12px' : '0'
                }}>
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    flex: 1
                  }}>
                    <button
                      onClick={() => toggleGroup(groupIndex)}
                      style={{
                        padding: '6px',
                        border: 'none',
                        backgroundColor: 'transparent',
                        cursor: 'pointer',
                        borderRadius: '6px',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        transition: 'background-color 0.2s',
                        color: '#6b7280'
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = '#e5e7eb'
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'transparent'
                      }}
                    >
                      <svg
                        width="20"
                        height="20"
                        viewBox="0 0 20 20"
                        fill="none"
                        xmlns="http://www.w3.org/2000/svg"
                        style={{
                          transform: expandedGroups.has(groupIndex) ? 'rotate(90deg)' : 'rotate(0deg)',
                          transition: 'transform 0.2s'
                        }}
                      >
                        <path
                          d="M7.5 15L12.5 10L7.5 5"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      </svg>
                    </button>
                    <h3 style={{
                      fontSize: '16px',
                      fontWeight: '600',
                      color: '#1f2937',
                      margin: 0
                    }}>
                      Audio Group {groupIndex + 1}
                    </h3>
                    <span style={{
                      fontSize: '12px',
                      color: '#6b7280',
                      padding: '4px 8px',
                      backgroundColor: '#e5e7eb',
                      borderRadius: '4px'
                    }}>
                      {group.transcripts.length} transcript{group.transcripts.length !== 1 ? 's' : ''}
                    </span>
                  </div>
                </div>
                {expandedGroups.has(groupIndex) && (
                <div>
                  {group.audioUrl ? (
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    marginTop: '12px'
                  }}>
                    <span style={{
                      fontSize: '12px',
                      fontWeight: '500',
                      color: '#6b7280',
                      minWidth: '80px'
                    }}>
                      Audio:
                    </span>
                    {(() => {
                      const audioUrl = getAudioUrl(group.audioUrl)
                      console.log('[AudioPage] Rendering audio player for group', groupIndex, 'originalUrl:', group.audioUrl, 'processedUrl:', audioUrl)
                    
                    if (!audioUrl) {
                      console.warn('[AudioPage] Empty audio URL, cannot render audio player')
                      return <span style={{ color: '#ef4444', fontSize: '12px' }}>Invalid audio URL</span>
                    }
                    
                    // For relative URLs, we don't need to validate with new URL()
                    // The browser will handle relative paths correctly
                    console.log('[AudioPage] Rendering audio element with URL:', audioUrl)
                    
                    return (
                      <audio
                        controls
                        style={{
                          flex: 1,
                          maxWidth: '600px',
                          height: '32px'
                        }}
                        onError={(e) => {
                          const audioElement = e.currentTarget
                          console.error('[AudioPage] Audio element error:', {
                            error: audioElement.error,
                            networkState: audioElement.networkState,
                            readyState: audioElement.readyState,
                            src: audioUrl
                          })
                        }}
                        onLoadStart={() => {
                          console.log('[AudioPage] Audio loading started, src:', audioUrl)
                        }}
                        onCanPlay={() => {
                          console.log('[AudioPage] Audio can play, src:', audioUrl)
                        }}
                        onLoadedMetadata={() => {
                          console.log('[AudioPage] Audio metadata loaded, duration:', audioUrl)
                        }}
                      >
                        <source src={audioUrl} type="audio/mp4" />
                        <source src={audioUrl} type="audio/mpeg" />
                        Your browser does not support the audio element.
                      </audio>
                    )
                  })()}
                  </div>
                  ) : (
                    <div style={{
                      marginTop: '12px',
                      padding: '8px 12px',
                      backgroundColor: '#fef3c7',
                      border: '1px solid #fbbf24',
                      borderRadius: '4px',
                      fontSize: '12px',
                      color: '#92400e'
                    }}>
                      No audio file available for this transcript group
                    </div>
                  )}
                </div>
                )}
              </div>

              {/* Transcripts Table */}
              {expandedGroups.has(groupIndex) && (
              <div style={{
                overflowX: 'auto'
              }}>
                <table style={{
                  width: '100%',
                  borderCollapse: 'collapse',
                  minWidth: '500px'
                }}>
                  <thead>
                    <tr style={{
                      backgroundColor: '#f9fafb',
                      borderBottom: '2px solid #e5e7eb'
                    }}>
                      <th style={{
                        padding: '12px 16px',
                        textAlign: 'left',
                        fontSize: '12px',
                        fontWeight: '600',
                        color: '#6b7280',
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em'
                      }}>
                        Time
                      </th>
                      <th style={{
                        padding: '12px 16px',
                        textAlign: 'left',
                        fontSize: '12px',
                        fontWeight: '600',
                        color: '#6b7280',
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em'
                      }}>
                        Text
                      </th>
                      <th style={{
                        padding: '12px 16px',
                        textAlign: 'left',
                        fontSize: '12px',
                        fontWeight: '600',
                        color: '#6b7280',
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em'
                      }}>
                        Speaker
                      </th>
                      <th style={{
                        padding: '12px 16px',
                        textAlign: 'left',
                        fontSize: '12px',
                        fontWeight: '600',
                        color: '#6b7280',
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em'
                      }}>
                        Duration
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {group.transcripts.map((transcript, index) => (
                      <tr
                        key={`${transcript.time}-${index}`}
                        style={{
                          borderBottom: '1px solid #e5e7eb',
                          transition: 'background-color 0.15s'
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.backgroundColor = '#f9fafb'
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.backgroundColor = '#ffffff'
                        }}
                      >
                        <td style={{
                          padding: '12px 16px',
                          fontSize: '14px',
                          color: '#1f2937'
                        }}>
                          {formatTime(transcript.time)}
                        </td>
                        <td style={{
                          padding: '12px 16px',
                          fontSize: '14px',
                          color: '#1f2937',
                          maxWidth: '300px',
                          wordBreak: 'break-word'
                        }}>
                          {transcript.fields.text}
                        </td>
                        <td style={{
                          padding: '12px 16px',
                          fontSize: '14px',
                          color: '#1f2937'
                        }}>
                          <span style={{
                            padding: '4px 8px',
                            borderRadius: '4px',
                            fontSize: '12px',
                            fontWeight: '500',
                            backgroundColor: transcript.fields.speaker === 'customer' ? '#dbeafe' : '#fef3c7',
                            color: transcript.fields.speaker === 'customer' ? '#1e40af' : '#92400e'
                          }}>
                            {transcript.fields.speaker}
                          </span>
                        </td>
                        <td style={{
                          padding: '12px 16px',
                          fontSize: '14px',
                          color: '#1f2937'
                        }}>
                          {formatDuration(transcript.fields.duration_ms)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
