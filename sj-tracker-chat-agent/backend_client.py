#!/usr/bin/env python3
"""
============================================================================
BACKEND CLIENT
============================================================================

PURPOSE: Client for interacting with Go backend tool API
         Fetches tool definitions and executes tools

============================================================================
"""

import requests
from typing import List, Dict, Any, Optional


class BackendToolClient:
    """Client for interacting with Go backend tool API"""
    
    def __init__(self, backend_url: str = "http://localhost:8085"):
        """
        Initialize backend client
        
        INPUTS:
        - backend_url: str - Base URL of the Go backend service
        """
        self.backend_url = backend_url.rstrip('/')
        self.tools_cache: Optional[List[Dict[str, Any]]] = None
    
    def fetch_tools(self) -> List[Dict[str, Any]]:
        """
        Fetch all available tools from the backend
        
        OUTPUTS:
        - List[Dict[str, Any]] - List of tool definitions with name, description, parameters
        
        ERROR HANDLING:
        - Raises exception if backend is unreachable or returns error
        """
        try:
            response = requests.get(
                f"{self.backend_url}/api/chat/tools",
                timeout=10
            )
            response.raise_for_status()
            
            data = response.json()
            tools = data.get("tools", [])
            self.tools_cache = tools
            return tools
            
        except requests.exceptions.RequestException as e:
            raise Exception(f"Failed to fetch tools from backend: {str(e)}")
    
    def execute_tool(self, tool_name: str, params: Dict[str, Any]) -> str:
        """
        Execute a tool with given parameters
        
        INPUTS:
        - tool_name: str - Name of the tool to execute
        - params: Dict[str, Any] - Parameters for the tool
        
        OUTPUTS:
        - str - JSON string result from tool execution (empty array "[]" if no data)
        
        ERROR HANDLING:
        - Raises exception if tool execution fails
        """
        try:
            # Log the request for debugging
            print(f"[CHAT-AGENT] Executing tool '{tool_name}' with params: {params}")
            
            response = requests.post(
                f"{self.backend_url}/api/chat/tools/execute",
                json={
                    "tool_name": tool_name,
                    "params": params
                },
                timeout=60  # Tools may take longer to execute
            )
            
            print(f"[CHAT-AGENT] Tool '{tool_name}' response status: {response.status_code}")
            
            # Check for HTTP errors and extract detailed error message
            if not response.ok:
                try:
                    error_data = response.json()
                    error_msg = error_data.get("message") or error_data.get("error", f"HTTP {response.status_code}")
                    raise Exception(f"Failed to execute tool '{tool_name}': {error_msg}")
                except ValueError:
                    # If response is not JSON, use status text
                    raise Exception(f"Failed to execute tool '{tool_name}': {response.status_code} {response.text}")
            
            data = response.json()
            result = data.get("result")
            
            # Handle null/None results - return empty array JSON string
            if result is None or result == "null" or result == "":
                return "[]"
            
            # If result is already a string, return it
            if isinstance(result, str):
                return result
            
            # If result is a list/dict, convert to JSON string
            import json
            return json.dumps(result)
            
        except requests.exceptions.RequestException as e:
            # Extract more details from the error
            error_detail = str(e)
            if hasattr(e, 'response') and e.response is not None:
                try:
                    error_data = e.response.json()
                    error_msg = error_data.get("message") or error_data.get("error", error_detail)
                    error_detail = f"{error_detail} - {error_msg}"
                except ValueError:
                    error_detail = f"{error_detail} - Response: {e.response.text[:200]}"
            raise Exception(f"Failed to execute tool '{tool_name}': {error_detail}")

