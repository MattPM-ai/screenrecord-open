/**
 * ============================================================================
 * CHAT API ROUTE
 * ============================================================================
 * 
 * PURPOSE: Handles communication with the LangChain chat agent service
 * 
 * DESCRIPTION:
 * This API route receives chat messages from the frontend and forwards
 * them to the local LangChain chat agent service. It processes the response
 * and returns it to the client in a standardized format.
 * 
 * DEPENDENCIES:
 * - External: LangChain chat agent service (configured via CHAT_AGENT_URL environment variable)
 * 
 * INPUTS:
 * - POST body: { chatInput: string, sessionId: string } - The user's message and session ID
 * 
 * OUTPUTS:
 * - JSON: { response: string } - The assistant's response from chat agent
 * 
 * ERROR HANDLING:
 * - Returns appropriate HTTP status codes
 * - Provides error messages for debugging
 * 
 * ============================================================================
 */

import { NextRequest, NextResponse } from 'next/server'

// Hardcoded chat agent URL for bundled app
const CHAT_AGENT_URL = 'http://localhost:8087'

/**
 * Handles POST requests to send messages to the LangChain chat agent
 * 
 * INPUTS:
 * - request: NextRequest - Contains the user's chat input and session ID in JSON body
 * 
 * OUTPUTS:
 * - NextResponse with chat agent response or error
 * 
 * ERROR HANDLING:
 * - Validates request body
 * - Handles network errors
 * - Returns appropriate status codes
 */
export async function POST(request: NextRequest) {
  try {
    const body = await request.json()
    const { chatInput, sessionId } = body

    // Validate input
    if (!chatInput || typeof chatInput !== 'string' || !chatInput.trim()) {
      return NextResponse.json(
        { error: 'chatInput is required and must be a non-empty string' },
        { status: 400 }
      )
    }

    if (!sessionId || typeof sessionId !== 'string') {
      return NextResponse.json(
        { error: 'Session ID is required and must be a string' },
        { status: 400 }
      )
    }

    // Get Gemini API key from request body (provided by frontend)
    const geminiApiKey = body.geminiApiKey || body.openaiApiKey // Support both for backward compatibility

    if (!geminiApiKey || typeof geminiApiKey !== 'string' || !geminiApiKey.trim()) {
      return NextResponse.json(
        { error: 'Gemini API key is required. Please enter your API key in the chat interface.' },
        { status: 400 }
      )
    }

    // Prepare request body for chat agent
    const chatAgentBody = {
      chatInput: chatInput.trim(),
      sessionId: sessionId,
      geminiApiKey: geminiApiKey.trim(),
    }

    // Forward request to LangChain chat agent
    const chatAgentResponse = await fetch(`${CHAT_AGENT_URL}/api/chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(chatAgentBody),
    })

    // Handle chat agent response
    if (!chatAgentResponse.ok) {
      const errorText = await chatAgentResponse.text()
      console.error('Chat agent error:', chatAgentResponse.status, errorText)
      
      return NextResponse.json(
        { 
          error: chatAgentResponse.status === 500 
            ? 'Chat agent responded with a status of 500 (Internal Server Error)'
            : 'Failed to get response from chat agent',
          details: errorText 
        },
        { status: chatAgentResponse.status }
      )
    }

    // Parse chat agent response
    const responseData = await chatAgentResponse.json()
    const response = responseData.response

    if (response === undefined || response === null) {
      console.error('Chat agent response missing response field:', responseData)
      return NextResponse.json(
        { error: 'Chat agent response missing response field' },
        { status: 500 }
      )
    }

    // Return success response
    return NextResponse.json({ response })
  } catch (error) {
    console.error('API route error:', error)
    
    // Handle different error types
    if (error instanceof SyntaxError) {
      return NextResponse.json(
        { error: 'Invalid JSON in request body' },
        { status: 400 }
      )
    }

    if (error instanceof TypeError && error.message.includes('fetch')) {
      return NextResponse.json(
        { 
          error: 'Failed to connect to chat agent service',
          details: `Could not reach chat agent at ${CHAT_AGENT_URL}. Please ensure the chat agent service is running.`
        },
        { status: 503 }
      )
    }

    return NextResponse.json(
      { error: 'Internal server error', details: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 }
    )
  }
}

