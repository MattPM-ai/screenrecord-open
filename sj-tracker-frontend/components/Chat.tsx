/**
 * ============================================================================
 * CHAT COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Main chat interface component with message display and input
 * 
 * DESCRIPTION:
 * This component provides a minimalist chat interface that communicates
 * with the n8n webhook API. It handles message sending, receiving, and
 * displays messages in a clean, aesthetic design.
 * 
 * DEPENDENCIES:
 * - /app/api/chat/route.ts: Handles webhook communication
 * 
 * ============================================================================
 */

'use client'

import { useState, useRef, useEffect } from 'react'
import { getSessionId } from '@/lib/session'
import { parseMessageContent } from '@/lib/graphParser'
import { getGeminiKeyStatus, getGeminiKey, saveGeminiKey } from '@/lib/geminiApiKey'
import GraphDisplay from './GraphDisplay'

interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
}

/**
 * Renders message content with support for inline graphs
 * 
 * INPUTS:
 * - content: string - The message content
 * - role: 'user' | 'assistant' - Message role for styling
 * 
 * OUTPUTS:
 * - JSX.Element - Rendered message content with graphs
 */
function MessageContent({ content, role }: { content: string; role: 'user' | 'assistant' }) {
  const segments = parseMessageContent(content)

  return (
    <>
      {segments.map((segment, index) => {
        if (segment.type === 'graph' && segment.graphData) {
          return (
            <div key={`graph-${index}`} className="w-full my-1">
              <GraphDisplay graphData={segment.graphData} />
            </div>
          )
        } else if (segment.type === 'text' && segment.content) {
          return (
            <span key={`text-${index}`} className="block">
              {segment.content}
            </span>
          )
        }
        return null
      })}
    </>
  )
}

const GEMINI_API_KEY_STORAGE_KEY = 'gemini_api_key'

export default function Chat() {
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [apiKey, setApiKey] = useState<string>('')
  const [showApiKeyInput, setShowApiKeyInput] = useState(false)
  const [apiKeyError, setApiKeyError] = useState<string>('')
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)

  /**
   * Initializes session ID and loads API key on component mount (shared with desktop app via backend)
   */
  useEffect(() => {
    const id = getSessionId()
    setSessionId(id)
    
    const loadKey = async () => {
      try {
        const { set } = await getGeminiKeyStatus()
        if (set) {
          const key = await getGeminiKey()
          if (key) setApiKey(key)
          setShowApiKeyInput(false)
          return
        }
      } catch {
        // Backend not available
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

  /**
   * Scrolls the chat container to the bottom when new messages are added
   */
  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }

  useEffect(() => {
    scrollToBottom()
  }, [messages])

  /**
   * Handles saving API key (writes to shared backend so desktop app and reports see it too)
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
      setShowApiKeyInput(false)
      setApiKeyError('')
    } catch (err) {
      setApiKeyError(err instanceof Error ? err.message : 'Failed to save key')
    }
  }

  /**
   * Handles sending a message to the chat agent
   * 
   * INPUTS:
   * - message: string - The user's message content
   * 
   * OUTPUTS:
   * - Updates messages state with user message and assistant response
   * - Sets loading state during API call
   * 
   * ERROR HANDLING:
   * - Displays error message if API call fails
   * - Prompts for API key if not set
   */
  const sendMessage = async (message: string) => {
    if (!message.trim() || isLoading || !sessionId) return

    // Check if API key is set
    if (!apiKey || !apiKey.trim()) {
      setShowApiKeyInput(true)
      setApiKeyError('Please enter your Gemini API key to use the chat')
      return
    }

    const userMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: message.trim(),
      timestamp: new Date(),
    }

    setMessages((prev) => [...prev, userMessage])
    setInput('')
    setIsLoading(true)

    try {
      const response = await fetch('/api/chat', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ 
          chatInput: message.trim(),
          sessionId: sessionId,
          geminiApiKey: apiKey.trim(),
        }),
      })

      const data = await response.json()

      if (!response.ok) {
        // Extract error message from API response
        const errorMsg = data.error || 'Failed to send message'
        throw new Error(errorMsg)
      }

      const assistantMessage: Message = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: data.response || data.message || 'No response received',
        timestamp: new Date(),
      }

      setMessages((prev) => [...prev, assistantMessage])
    } catch (error) {
      // Display the specific error message from the API
      const errorContent = error instanceof Error 
        ? error.message 
        : 'Sorry, I encountered an error. Please try again.'
      
      const errorMessage: Message = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: errorContent,
        timestamp: new Date(),
      }
      setMessages((prev) => [...prev, errorMessage])
    } finally {
      setIsLoading(false)
    }
  }

  /**
   * Handles form submission
   */
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    sendMessage(input)
  }

  /**
   * Handles Enter key press (with Shift for new line)
   */
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      sendMessage(input)
    }
  }

  return (
    <div className="w-full max-w-4xl h-[90vh] max-h-[800px] flex flex-col bg-white rounded-xl shadow-lg overflow-hidden border border-gray-200">
      <div className="px-6 py-4 border-b border-gray-200 bg-white flex items-center justify-between">
        <h1 className="text-xl font-semibold text-gray-900 m-0 tracking-tight">Chat</h1>
        {!showApiKeyInput && apiKey && (
          <button
            onClick={() => {
              setShowApiKeyInput(true)
              setApiKeyError('')
            }}
            className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded-md hover:bg-gray-100 transition-colors"
            title="Change API key"
          >
            API Key
          </button>
        )}
      </div>

      {/* API Key Input Modal */}
      {showApiKeyInput && (
        <div className="px-6 py-4 border-b border-gray-200 bg-blue-50">
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
                onClick={handleSaveApiKey}
                className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors text-sm font-medium"
              >
                Save
              </button>
              {apiKey && (
                <button
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
              Your API key is stored locally and shared with the desktop app. Get your key from{' '}
              <a href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener noreferrer" className="text-blue-600 hover:underline">
                Google AI Studio
              </a>
            </p>
          </div>
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-6 flex flex-col gap-4 bg-white">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full p-8">
            <p className="text-base text-gray-600 text-center">
              Start a conversation by sending a message below.
            </p>
          </div>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={`flex max-w-[75%] animate-[fadeIn_0.3s_ease-in-out] ${
                message.role === 'user' ? 'self-end ml-auto' : 'self-start'
              } ${message.content.includes('graph') ? 'max-w-[95%]' : ''}`}
            >
              <div
                className={`px-4 py-3 rounded-lg text-base leading-relaxed break-words flex flex-col gap-2 ${
                  message.role === 'user'
                    ? 'bg-blue-600 text-white rounded-br-sm'
                    : 'bg-gray-100 text-gray-900 rounded-bl-sm'
                }`}
              >
                <MessageContent content={message.content} role={message.role} />
              </div>
            </div>
          ))
        )}
        {isLoading && (
          <div className="flex max-w-[75%] self-start">
            <div className="px-4 py-3 rounded-lg bg-gray-100 text-gray-900 rounded-bl-sm">
              <span className="inline-flex gap-1 items-center">
                <span className="w-2 h-2 rounded-full bg-gray-600 animate-[typing_1.4s_infinite_ease-in-out]" style={{ animationDelay: '-0.32s' }}></span>
                <span className="w-2 h-2 rounded-full bg-gray-600 animate-[typing_1.4s_infinite_ease-in-out]" style={{ animationDelay: '-0.16s' }}></span>
                <span className="w-2 h-2 rounded-full bg-gray-600 animate-[typing_1.4s_infinite_ease-in-out]"></span>
              </span>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      <form onSubmit={handleSubmit} className="px-6 py-4 border-t border-gray-200 bg-white">
        <div className="flex gap-2 items-end bg-gray-50 rounded-lg p-2 border border-gray-200 focus-within:border-blue-500 focus-within:ring-2 focus-within:ring-blue-200 transition-colors">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type your message..."
            className="flex-1 border-none bg-transparent resize-none focus:outline-none text-base text-gray-900 placeholder-gray-400"
            rows={1}
            disabled={isLoading}
          />
          <button
            type="submit"
            disabled={!input.trim() || isLoading}
            className="p-2 text-blue-600 hover:text-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            aria-label="Send message"
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <line x1="22" y1="2" x2="11" y2="13"></line>
              <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
            </svg>
          </button>
        </div>
      </form>
    </div>
  )
}

