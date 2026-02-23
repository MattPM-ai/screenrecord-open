#!/usr/bin/env python3
"""
============================================================================
CHAT AGENT HTTP SERVER - LLM Chat with Tools
============================================================================

PURPOSE: HTTP API server for LLM chat with tool support
         Receives chat messages and returns agent responses

============================================================================
"""

import os
import sys
import traceback
from flask import Flask, request, jsonify
from flask_cors import CORS

# Import parent package first to ensure it's initialized in PyInstaller bundle
import langchain.agents
# Then import directly from agent module to avoid __init__.py import order issues
from langchain.agents.agent import AgentExecutor
from langchain_core.messages import HumanMessage, AIMessage

# Import from main.py
from main import create_agent, process_message
from backend_client import BackendToolClient

# Configuration
GEMINI_MODEL = os.getenv("GEMINI_MODEL", "gemini-2.5-flash")
BACKEND_URL = os.getenv("BACKEND_URL", "http://localhost:8085")
PORT = int(os.getenv("CHAT_AGENT_PORT", "8087"))
HOST = os.getenv("HOST", "0.0.0.0")

# Initialize Flask app
app = Flask(__name__)
CORS(app)  # Enable CORS for frontend

# Global agent instances (keyed by API key for caching)
agent_cache = {}  # api_key -> agent_executor instance
chat_sessions = {}  # session_id -> chat history
backend_client = BackendToolClient(BACKEND_URL)


def get_or_create_agent(api_key: str) -> AgentExecutor:
    """Get or create an agent instance for the given API key"""
    global agent_cache
    
    # Return cached agent if exists
    if api_key in agent_cache:
        return agent_cache[api_key]
    
    # Create new agent with user's API key
    try:
        agent_executor = create_agent(api_key, GEMINI_MODEL, backend_client)
    except Exception as e:
        print(f"[CHAT-AGENT] ERROR: Failed to create agent (model={GEMINI_MODEL}): {type(e).__name__}: {e}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        raise
    agent_cache[api_key] = agent_executor
    print(f"[CHAT-AGENT] Created new agent for API key (key: {api_key[:10]}...) model={GEMINI_MODEL}")
    
    return agent_executor


@app.route("/health", methods=["GET"])
def health():
    """Health check endpoint"""
    return jsonify({
        "status": "ok",
        "agents_cached": len(agent_cache),
        "backend_url": BACKEND_URL
    })


@app.route("/api/chat", methods=["POST"])
def chat():
    """Handle chat messages from frontend"""
    try:
        data = request.json
        chat_input = data.get("chatInput")
        session_id = data.get("sessionId")
        gemini_api_key = data.get("geminiApiKey") or data.get("openaiApiKey")  # Support both for backward compatibility
        
        # Validate input
        if not chat_input or not isinstance(chat_input, str) or not chat_input.strip():
            return jsonify({"error": "chatInput is required and must be a non-empty string"}), 400
        
        if not session_id or not isinstance(session_id, str):
            return jsonify({"error": "sessionId is required and must be a string"}), 400
        
        if not gemini_api_key or not isinstance(gemini_api_key, str) or not gemini_api_key.strip():
            return jsonify({"error": "geminiApiKey is required and must be a non-empty string"}), 400
        
        api_key_trimmed = gemini_api_key.strip()
        print(f"[CHAT-AGENT] Chat request: session_id={session_id} model={GEMINI_MODEL} api_key_prefix={api_key_trimmed[:10]}...")
        
        # Get or create agent for this API key
        try:
            agent_executor = get_or_create_agent(api_key_trimmed)
        except Exception as e:
            err_msg = f"Failed to initialize agent: {str(e)}"
            print(f"[CHAT-AGENT] ERROR: {err_msg}", file=sys.stderr)
            traceback.print_exc(file=sys.stderr)
            return jsonify({"error": err_msg}), 500
        
        # Get or create chat history for this session
        if session_id not in chat_sessions:
            chat_sessions[session_id] = []
        
        chat_history = chat_sessions[session_id]
        
        # Process message through agent
        try:
            response_text = process_message(agent_executor, chat_input.strip(), chat_history)
            
            # Update chat history
            chat_history.append(HumanMessage(content=chat_input.strip()))
            chat_history.append(AIMessage(content=response_text))
            
            # Limit history size (keep last 20 messages)
            if len(chat_history) > 20:
                chat_history = chat_history[-20:]
                chat_sessions[session_id] = chat_history
            
            return jsonify({"response": response_text})
            
        except Exception as e:
            print(f"[CHAT-AGENT] ERROR: process_message failed: {type(e).__name__}: {e}", file=sys.stderr)
            traceback.print_exc(file=sys.stderr)
            return jsonify({"error": f"Error processing message: {str(e)}"}), 500
            
    except Exception as e:
        print(f"[CHAT-AGENT] ERROR: chat endpoint: {type(e).__name__}: {e}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        return jsonify({"error": str(e)}), 500


@app.route("/api/chat/session/<session_id>", methods=["DELETE"])
def clear_session(session_id: str):
    """Clear chat history for a session"""
    if session_id in chat_sessions:
        del chat_sessions[session_id]
        return jsonify({"message": "Session cleared"})
    return jsonify({"message": "Session not found"}), 404


if __name__ == "__main__":
    try:
        print(f"Initializing chat agent server...")
        print(f"Gemini Model: {GEMINI_MODEL}")
        print(f"Backend URL: {BACKEND_URL}")
        print("Agents will be created per-request using user-provided API keys")
        
        print(f"\n" + "="*60)
        print(f"Chat Agent HTTP Server")
        print(f"Listening on {HOST}:{PORT}")
        print("="*60 + "\n")
        
        # Start Flask server
        app.run(host=HOST, port=PORT, debug=False)
        
    except Exception as e:
        print(f"Failed to start server: {e}")
        exit(1)
