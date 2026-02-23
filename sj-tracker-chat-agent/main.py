#!/usr/bin/env python3
"""
============================================================================
CHAT AGENT SERVICE - LLM Chat with Tools
============================================================================

PURPOSE: Chat interface using Google Gemini LLM with tool support
         Fetches tools from Go backend and uses them via LangChain agent

============================================================================
"""

import os
from typing import List, Optional, Dict, Any
from datetime import datetime
import sys
import traceback

from langchain_google_genai import ChatGoogleGenerativeAI
from langchain_core.messages import HumanMessage, AIMessage, SystemMessage
from langchain_core.tools import StructuredTool
# Import parent package first to ensure it's initialized in PyInstaller bundle
import langchain.agents
# Then import directly from modules to avoid __init__.py import order issues
from langchain.agents.agent import AgentExecutor
from langchain.agents.tool_calling_agent.base import create_tool_calling_agent
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

from backend_client import BackendToolClient


# Configuration
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY", "")
GEMINI_MODEL = os.getenv("GEMINI_MODEL", "gemini-2.5-flash")
BACKEND_URL = os.getenv("BACKEND_URL", "http://localhost:8085")


def create_langchain_tools(backend_client: BackendToolClient) -> List[StructuredTool]:
    """
    Create LangChain tools from backend tool definitions
    
    INPUTS:
    - backend_client: BackendToolClient - Client for backend API
    
    OUTPUTS:
    - List[StructuredTool] - List of LangChain tools ready for agent
    
    ERROR HANDLING:
    - Raises exception if tools cannot be fetched
    """
    # Fetch tools from backend
    tool_defs = backend_client.fetch_tools()
    
    langchain_tools = []
    
    for tool_def in tool_defs:
        tool_name = tool_def["name"]
        tool_description = tool_def["description"]
        tool_params = tool_def.get("parameters", {})
        
        # Create a tool execution function with closure to capture tool_name
        # Use default argument to properly capture the name in the closure
        def create_executor(name: str):
            def execute_tool(**kwargs) -> str:
                return backend_client.execute_tool(name, kwargs)
            return execute_tool
        
        # Create tool function with captured name
        tool_func = create_executor(tool_name)
        
        # For Gemini, we need to create tools that match the JSON schema format
        # Create a Pydantic model dynamically from the JSON schema
        from pydantic import create_model, BaseModel
        from typing import Any, Dict
        
        # Extract properties from JSON schema
        properties = tool_params.get("properties", {})
        required = tool_params.get("required", [])
        
        # Create field definitions for Pydantic model
        field_definitions = {}
        for prop_name, prop_schema in properties.items():
            prop_type = prop_schema.get("type", "string")
            
            # Map JSON schema types to Python types
            if prop_type == "string":
                python_type = str
            elif prop_type == "number":
                python_type = float
            elif prop_type == "integer":
                python_type = int
            elif prop_type == "boolean":
                python_type = bool
            elif prop_type == "array":
                # Handle array types with items schema
                items_schema = prop_schema.get("items")
                if items_schema and isinstance(items_schema, dict):
                    items_type = items_schema.get("type", "string")
                    if items_type == "object":
                        # For array of objects, we need to preserve the properties structure
                        # Gemini requires items.properties to be defined, not just type: object
                        # We'll use Dict[str, Any] but need to manually add the properties to the schema
                        from typing import List
                        # Store the original items schema for later patching
                        # Use Dict[str, Any] for now - we'll patch the schema after model creation
                        python_type = List[Dict[str, Any]]
                        # Store the items schema in the prop_schema for later reference
                        prop_schema['_items_schema'] = items_schema  # Store for patching
                    else:
                        # For array of primitives
                        from typing import List
                        if items_type == "string":
                            python_type = List[str]
                        elif items_type == "number":
                            python_type = List[float]
                        elif items_type == "integer":
                            python_type = List[int]
                        else:
                            python_type = List[Any]
                else:
                    # Fallback to generic list
                    python_type = list
            elif prop_type == "object":
                python_type = Dict[str, Any]
            else:
                python_type = Any
            
            # Make field optional if not in required list
            if prop_name not in required:
                from typing import Optional
                python_type = Optional[python_type]
            
            field_definitions[prop_name] = (python_type, ...)
        
        # Create Pydantic model dynamically
        # For tools with array of objects, we need to preserve the items properties
        # Create a custom model class that patches the schema for Gemini compatibility
        if field_definitions:
            # Check if we have any array of objects that need patching
            needs_patching = False
            items_schemas_to_patch = {}
            for prop_name, prop_schema in properties.items():
                if prop_schema.get('type') == 'array':
                    items = prop_schema.get('items', {})
                    if items.get('type') == 'object' and 'properties' in items:
                        needs_patching = True
                        items_schemas_to_patch[prop_name] = items
            
            if needs_patching:
                # Create a custom model class that overrides model_json_schema to patch items
                class PatchedToolArgsModel(BaseModel):
                    class Config:
                        arbitrary_types_allowed = True
                
                # Create the base model
                base_model = create_model(f"{tool_name}_Args", __base__=PatchedToolArgsModel, **field_definitions)
                
                # Override model_json_schema to patch array items
                original_schema_method = base_model.model_json_schema if hasattr(base_model, 'model_json_schema') else base_model.schema
                
                def patched_schema_method(*args, **kwargs):
                    schema = original_schema_method(*args, **kwargs)
                    # Patch array items to include properties
                    for prop_name, items_schema in items_schemas_to_patch.items():
                        if prop_name in schema.get('properties', {}):
                            prop = schema['properties'][prop_name]
                            if prop.get('type') == 'array' and 'items' in prop:
                                # Replace items with the full schema from backend
                                prop['items'] = items_schema
                    return schema
                
                # Monkey-patch the schema method
                if hasattr(base_model, 'model_json_schema'):
                    base_model.model_json_schema = classmethod(patched_schema_method)
                else:
                    base_model.schema = classmethod(patched_schema_method)
                
                ToolArgsModel = base_model
            else:
                ToolArgsModel = create_model(f"{tool_name}_Args", **field_definitions)
        else:
            # If no parameters, create empty model
            class ToolArgsModel(BaseModel):
                pass
        
        # Create StructuredTool with the dynamic model
        langchain_tool = StructuredTool.from_function(
            func=tool_func,
            name=tool_name,
            description=tool_description,
            args_schema=ToolArgsModel,
        )
        
        # Patch array items schemas to include properties for Gemini compatibility
        # Gemini requires items.properties to be defined for array of objects
        try:
            # Get the Pydantic schema
            if hasattr(ToolArgsModel, 'model_json_schema'):
                pydantic_schema = ToolArgsModel.model_json_schema()
            else:
                pydantic_schema = ToolArgsModel.schema()
            
            # Patch array items that are objects to include properties from backend schema
            for prop_name, prop_schema in pydantic_schema.get('properties', {}).items():
                if prop_schema.get('type') == 'array':
                    items = prop_schema.get('items', {})
                    # If items is just type: object without properties, we need to add them
                    if items.get('type') == 'object' and 'properties' not in items:
                        # Get the original items schema from backend
                        backend_prop = tool_params.get('properties', {}).get(prop_name, {})
                        backend_items = backend_prop.get('items', {})
                        if 'properties' in backend_items:
                            # Copy the properties from backend schema
                            items['properties'] = backend_items['properties']
                            if 'required' in backend_items:
                                items['required'] = backend_items['required']
            
            # Note: We can't directly modify the tool's schema, but LangChain should use
            # the model_json_schema() when converting to Gemini format. However, since
            # we can't modify the model's schema method, we need a different approach.
            # 
            # The issue is that Pydantic's model_json_schema() will regenerate the schema
            # each time, so our patch won't persist. We need to either:
            # 1. Create a custom Pydantic model that overrides model_json_schema()
            # 2. Use a custom tool class
            # 3. Patch LangChain's tool conversion
            #
            # For now, let's create a wrapper that patches the schema when needed
            
        except Exception as e:
            print(f"Warning: Could not patch schema for {tool_name}: {e}")
        
        langchain_tools.append(langchain_tool)
    
    return langchain_tools


def build_system_prompt() -> str:
    """
    Build the system prompt with current datetime
    
    OUTPUTS:
    - str - System prompt with current datetime inserted
    """
    from datetime import datetime
    
    # Get current datetime in ISO format
    current_time = datetime.now().isoformat()
    
    # Use .format() instead of f-string to avoid issues with curly braces in JSON examples
    system_prompt = """You are an AI agent concerned with handling user requests relating to productivity data stored in an InfluxDB v2.0 database. Your goal is to return the information that the user requests, in an accurate and concise way - without falsifying data or omitting records. The current datetime is {current_time}. Results stored in the database as seconds should be returned in minutes and hours (eg instead of 16182 seconds, say 4 hours 29 minutes 42 seconds).

For all requests, you should first process the request into individual tool calls, then process the data so as to return to the user the things they want.


You have several tools that allow you to pull data from the database. These pull data from the four main tables in the database which are listed below. You can pull from these by date, using the dedicated tools. We are using InfluxDB v2.0 so queries are done using Flux. With your tools you should only need to provide the date.

You have an additional tool that lets you make arbitrary Flux queries. This should NOT be used unless absolutely necessary.

At the bottom of this prompt after the database info, is instructions on how to generate/format info for images/visuals.

The user/userid/org/orgid/accountid fields are not relevant and are vestigial. No need to worry about those. If in doubt, just use local/0/local/0/0 respectively.

Databases:
  app_usage:
    id SERIAL PRIMARY KEY,
    app_name VARCHAR(255),
    duration_seconds INTEGER,
    event_count INTEGER,
    hostname VARCHAR(255),
    account_id INTEGER,
    org VARCHAR(255),
    org_id INTEGER,
    user VARCHAR(255),
    user_id INTEGER,
    time TIMESTAMP

  afk_status:
    id SERIAL PRIMARY KEY,
    duration INTEGER,
    hostname VARCHAR(255),
    account_id INTEGER,
    org VARCHAR(255),
    org_id INTEGER,
    user VARCHAR(255),
    user_id INTEGER,
    status VARCHAR(32),
    time TIMESTAMP

  window_activity:
    id SERIAL PRIMARY KEY,
    app VARCHAR(255),
    duration INTEGER,
    hostname VARCHAR(255),
    account_id INTEGER,
    org VARCHAR(255),
    org_id INTEGER,
    user VARCHAR(255),
    user_id INTEGER,
    time TIMESTAMP,
    title VARCHAR(255)

  daily_metrics:
    id SERIAL PRIMARY KEY,
    date DATE NOT NULL,
    hostname VARCHAR(255),
    account_id INTEGER,
    org VARCHAR(255),
    org_id INTEGER,
    user VARCHAR(255),
    user_id INTEGER,
    active_seconds INTEGER,
    afk_seconds INTEGER,
    app_switches INTEGER,
    idle_seconds INTEGER,
    utilization_ratio FLOAT,
    time TIMESTAMP


  screen_timeline:
    id SERIAL PRIMARY KEY,
    app VARCHAR(255),
    hostname VARCHAR(255),
    account_id INTEGER,
    org VARCHAR(255),
    org_id INTEGER,
    user VARCHAR(255),
    user_id INTEGER,
    segment_id VARCHAR(255),
    description VARCHAR(255),
    productive_score INTEGER,
    app_title VARCHAR(255),
    duration_seconds INTEGER,
    time_offset VARCHAR(255),
    time TIMESTAMP

# Graph Data Formatting Instructions for n8n Bot

## Overview
When your response contains data that would benefit from visual representation (trends, comparisons, distributions, time series, etc.), you should format it using special graph tags that will be automatically rendered as interactive charts in the chat interface.

## Graph Tag Format

Wrap graph data in XML-style tags with the following structure:

```
<graph type="[chart_type]">
{{{{{{{{ 
  "title": "Chart Title",
  "labels": ["Label1", "Label2", "Label3"],
  "datasets": [
    {{{{{{{{
      "label": "Dataset Name",
      "data": [10, 20, 30],
      "color": "#3b82f6"
    }}}}}}}}
  ],
  "options": {{{{{{{{
    "xAxisLabel": "X Axis Label",
    "yAxisLabel": "Y Axis Label"
  }}}}}}}}
}}}}}}}}
</graph>
```

**Note on Chart Type:**
- The `type` can be specified in the tag attribute: `<graph type="line">`
- OR in the JSON data: `{{{{{{{{ "type": "line", ... }}}}}}}}`
- If both are provided, the JSON `type` takes precedence
- If neither is provided, it defaults to `"line"`

## Supported Chart Types

### 1. Line Chart (`type="line"`)
Use for time series, trends, or continuous data over time.

**Example:**
```
Here's the activity trend over the past week:

<graph type="line">
{{{{
  "title": "Daily Active Time (Hours)",
  "labels": ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
  "datasets": [
    {{{{
      "label": "Active Hours",
      "data": [6.5, 7.2, 5.8, 8.1, 7.5, 4.2, 3.1],
      "color": "#3b82f6"
    }}}}
  ],
  "options": {{{{
    "xAxisLabel": "Day of Week",
    "yAxisLabel": "Hours"
  }}}}
}}}}
</graph>
```

### 2. Bar Chart (`type="bar"`)
Use for comparing discrete categories or values.

**Example:**
```
User activity comparison:

<graph type="bar">
{{{{
  "title": "Total Active Hours by User",
  "labels": ["Alice", "Bob", "Charlie", "Diana"],
  "datasets": [
    {{{{
      "label": "Active Hours",
      "data": [45.2, 38.7, 52.1, 41.3],
      "color": "#10b981"
    }}}}
  ],
  "options": {{{{
    "xAxisLabel": "User",
    "yAxisLabel": "Hours"
  }}}}
}}}}
</graph>
```

### 3. Pie Chart (`type="pie"`)
Use for showing proportions or percentages of a whole.

**Example:**
```
Time distribution breakdown:

<graph type="pie">
{{{{
  "title": "Time Distribution",
  "labels": ["Active", "AFK", "Offline"],
  "datasets": [
    {{{{
      "label": "Time",
      "data": [45, 30, 25],
      "colors": ["#10b981", "#f59e0b", "#ef4444"]
    }}}}
  ]
}}}}
</graph>
```

### 4. Area Chart (`type="area"`)
Use for cumulative data or stacked trends.

**Example:**
```
<graph type="area">
{{{{
  "title": "Cumulative Activity Over Time",
  "labels": ["Week 1", "Week 2", "Week 3", "Week 4"],
  "datasets": [
    {{{{
      "label": "Cumulative Hours",
      "data": [20, 45, 75, 110],
      "color": "#8b5cf6"
    }}}}
  ],
  "options": {{{{
    "xAxisLabel": "Week",
    "yAxisLabel": "Total Hours"
  }}}}
}}}}
</graph>
```

## Data Structure Requirements

### Required Fields:
- **title**: Chart title (string)
- **labels**: Array of strings for X-axis categories or pie segments
- **datasets**: Array of dataset objects

### Optional Fields:
- **type**: Chart type (`line`, `bar`, `pie`, `area`) - Can be in tag attribute or JSON. Defaults to `"line"` if not specified
- **options**: Object containing axis labels and other chart options

### Dataset Object:
- **label**: Name of the dataset (string)
- **data**: Array of numbers corresponding to labels
- **color**: Single color for line/bar/area charts (hex color code)
- **colors**: Array of colors for pie charts (one per segment)

### Optional Fields:
- **options**: Object containing:
  - **xAxisLabel**: Label for X-axis (string)
  - **yAxisLabel**: Label for Y-axis (string)

## Multiple Datasets

You can include multiple datasets for comparison charts. Each dataset will be rendered as a separate line/bar with its own color:

```
<graph type="line">
{{{{
  "title": "Active vs AFK Time Comparison",
  "labels": ["Mon", "Tue", "Wed", "Thu", "Fri"],
  "datasets": [
    {{{{
      "label": "Active Time",
      "data": [6.5, 7.2, 5.8, 8.1, 7.5],
      "color": "#10b981"
    }}}},
    {{{{
      "label": "AFK Time",
      "data": [1.5, 0.8, 2.2, 0.9, 0.5],
      "color": "#f59e0b"
    }}}}
  ],
  "options": {{{{
    "xAxisLabel": "Day",
    "yAxisLabel": "Hours"
  }}}}
}}}}
</graph>
```

**Example: Multiple Users on One Graph**

To compare multiple users' data on a single line graph:

```
<graph type="line">
{{{{
  "title": "Daily Active Hours by User",
  "labels": ["2025-11-15", "2025-11-16", "2025-11-17", "2025-11-18", "2025-11-19"],
  "datasets": [
    {{{{
      "label": "Chaz",
      "data": [2.92, 1.95, 8.65, 9.53, 8.47],
      "color": "#3b82f6"
    }}}},
    {{{{
      "label": "Alice",
      "data": [6.5, 7.2, 5.8, 8.1, 7.5],
      "color": "#10b981"
    }}}},
    {{{{
      "label": "Bob",
      "data": [5.2, 4.8, 6.1, 5.9, 6.3],
      "color": "#f59e0b"
    }}}}
  ],
  "options": {{{{
    "xAxisLabel": "Date",
    "yAxisLabel": "Hours"
  }}}}
}}}}
</graph>
```

**Color Handling:**
- If you specify a `color` for each dataset, those colors will be used
- If you omit `color`, the system will automatically assign distinct colors from a palette
- Each dataset must have the same number of data points as there are labels

## Best Practices

1. **Use graphs when data is numerical and benefits from visualization** - Don't use graphs for single values or purely textual information.

2. **Keep labels concise** - Long labels may not display well on charts.

3. **Choose appropriate chart types**:
   - **Line**: Trends over time, continuous data
   - **Bar**: Comparisons between categories
   - **Pie**: Proportions/percentages of a whole
   - **Area**: Cumulative or stacked data

4. **Provide context** - Always include a brief explanation before or after the graph tag explaining what the chart shows.

5. **Use consistent colors** - For multiple datasets, use distinct colors that are easily distinguishable.

6. **Validate data** - Ensure:
   - Labels array length matches data array length
   - All data values are numbers
   - Color codes are valid hex format (#RRGGBB)

## Example Complete Response

```
Based on the activity data, here's a summary of the week:

The team showed increasing productivity throughout the week, with a peak on Thursday.

<graph type="line">
{{{{
  "title": "Daily Active Hours - Team Average",
  "labels": ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"],
  "datasets": [
    {{{{
      "label": "Average Active Hours",
      "data": [6.2, 6.8, 7.1, 8.3, 7.5],
      "color": "#3b82f6"
    }}}}
  ],
  "options": {{{{
    "xAxisLabel": "Day",
    "yAxisLabel": "Hours"
  }}}}
}}}}
</graph>

The distribution of time across different activities:

<graph type="pie">
{{{{
  "title": "Time Distribution",
  "labels": ["Coding", "Meetings", "Documentation", "Testing", "Other"],
  "datasets": [
    {{{{
      "label": "Hours",
      "data": [25, 10, 8, 5, 2],
      "colors": ["#10b981", "#3b82f6", "#8b5cf6", "#f59e0b", "#ef4444"]
    }}}}
  ]
}}}}
</graph>
```

## Error Handling

If you're unsure about the data format or encounter issues:
- Use plain text tables or lists instead
- The frontend will gracefully handle malformed graph tags by displaying them as text
- When in doubt, provide the data in a clear textual format

## Notes

- Graph tags can appear anywhere in your response
- You can include multiple graphs in a single response
- Text before and after graph tags will be displayed normally
- The graph will be rendered inline with the message content"""
    
    # Format the prompt with current time
    return system_prompt.format(current_time=current_time)


def create_agent(
    api_key: str, 
    model: str, 
    backend_client: Optional[BackendToolClient] = None,
    system_prompt: Optional[str] = None
) -> AgentExecutor:
    """
    Create a LangChain agent with tools
    
    INPUTS:
    - api_key: str - Gemini API key
    - model: str - Gemini model name
    - backend_client: Optional[BackendToolClient] - Backend client for tools
    - system_prompt: Optional[str] - System prompt for the agent (if None, uses default)
    
    OUTPUTS:
    - AgentExecutor - Configured agent executor with tools
    
    ERROR HANDLING:
    - Raises exception if agent creation fails
    """
    # Initialize LLM
    llm = ChatGoogleGenerativeAI(
        model=model,
        temperature=0.3,
        google_api_key=api_key
    )
    print(f"[CHAT-AGENT] LLM initialized: model={model}")
    
    # If no backend client provided, create one
    if backend_client is None:
        backend_client = BackendToolClient(BACKEND_URL)
    
    # Create tools
    try:
        tools = create_langchain_tools(backend_client)
        print(f"✅ Loaded {len(tools)} tools from backend")
    except Exception as e:
        print(f"⚠️  Warning: Failed to load tools: {e}")
        tools = []
    
    # Use provided system prompt or build default one
    if system_prompt is None:
        system_prompt = build_system_prompt()
    
    # Create prompt template
    prompt = ChatPromptTemplate.from_messages([
        ("system", system_prompt),
        MessagesPlaceholder(variable_name="chat_history"),
        ("human", "{input}"),
        MessagesPlaceholder(variable_name="agent_scratchpad"),
    ])
    
    # Create agent
    if tools:
        agent = create_tool_calling_agent(llm, tools, prompt)
        agent_executor = AgentExecutor(
            agent=agent,
            tools=tools,
            verbose=True,
            handle_parsing_errors=True,
            max_iterations=10
        )
    else:
        # Fallback to simple LLM if no tools
        print("⚠️  No tools available, using simple LLM mode")
        # Create a simple agent executor that just uses the LLM without tools
        # For Gemini, we can use create_tool_calling_agent with empty tools
        agent = create_tool_calling_agent(llm, [], prompt)
        agent_executor = AgentExecutor(
            agent=agent,
            tools=[],
            verbose=True,
            handle_parsing_errors=True,
            max_iterations=5
        )
    
    return agent_executor


def process_message(
    agent_executor: AgentExecutor,
    message: str,
    chat_history: Optional[List] = None
) -> str:
    """
    Process a user message through the agent and return response
    
    INPUTS:
    - agent_executor: AgentExecutor - The agent executor with tools
    - message: str - User's message
    - chat_history: Optional[List] - Previous chat messages
    
    OUTPUTS:
    - str - Agent's response
    
    ERROR HANDLING:
    - Returns error message if processing fails
    """
    if chat_history is None:
        chat_history = []
    
    try:
        # Prepare input for agent
        input_dict = {
            "input": message,
            "chat_history": chat_history
        }
        
        # Invoke agent
        result = agent_executor.invoke(input_dict)
        
        return result.get("output", "No response generated")
        
    except Exception as e:
        print(f"[CHAT-AGENT] process_message failed: {type(e).__name__}: {e}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        return f"Error processing message: {str(e)}"


def main():
    """Main entry point for the chat agent service"""
    if not GEMINI_API_KEY:
        print("ERROR: GEMINI_API_KEY environment variable not set")
        return
    
    print(f"Initializing chat agent...")
    print(f"Gemini Model: {GEMINI_MODEL}")
    print(f"Backend URL: {BACKEND_URL}")
    
    # Create backend client
    backend_client = BackendToolClient(BACKEND_URL)
    
    # Create agent with tools
    try:
        agent_executor = create_agent(GEMINI_API_KEY, GEMINI_MODEL, backend_client)
        print("Agent initialized successfully!")
    except Exception as e:
        print(f"ERROR: Failed to create agent: {e}")
        return
    
    # Interactive chat loop
    print("\n" + "="*60)
    print("Chat Agent Ready! Type 'quit' or 'exit' to stop.")
    print("="*60 + "\n")
    
    chat_history = []
    
    while True:
        try:
            user_input = input("You: ").strip()
            
            if user_input.lower() in ["quit", "exit", "q"]:
                print("Goodbye!")
                break
            
            if not user_input:
                continue
            
            print("\nAgent: ", end="", flush=True)
            response = process_message(agent_executor, user_input, chat_history)
            print(response)
            print()
            
            # Update chat history
            chat_history.append(HumanMessage(content=user_input))
            chat_history.append(AIMessage(content=response))
            
        except KeyboardInterrupt:
            print("\n\nGoodbye!")
            break
        except Exception as e:
            print(f"\nError: {e}\n")


if __name__ == "__main__":
    main()
