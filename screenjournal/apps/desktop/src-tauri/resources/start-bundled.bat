@echo off
REM Bundled service startup script for ScreenJournal Tracker (Windows)
REM This script starts all services using bundled binaries from the Tauri app
REM Outputs structured progress messages for the Rust service manager to parse

REM Debug: write env to %TEMP% so we can confirm script ran and what it received (remove in production if desired)
echo [%date% %time%] start-bundled.bat started >> "%TEMP%\screenjournal-start-bundled-debug.txt"
echo RESOURCE_DIR=%RESOURCE_DIR% >> "%TEMP%\screenjournal-start-bundled-debug.txt"
echo APP_DATA_DIR=%APP_DATA_DIR% >> "%TEMP%\screenjournal-start-bundled-debug.txt"

REM Progress markers for parsing
set PROGRESS_PREFIX=[PROGRESS]
set ERROR_PREFIX=[ERROR]
set SUCCESS_PREFIX=[SUCCESS]
set STEP_PREFIX=[STEP]

REM Get paths from environment variables (set by Rust)
if "%RESOURCE_DIR%"=="" (
    echo %ERROR_PREFIX% RESOURCE_DIR not set
    exit /b 1
)
if "%APP_DATA_DIR%"=="" (
    echo %ERROR_PREFIX% APP_DATA_DIR not set
    exit /b 1
)

REM Determine platform and architecture (Windows)
set PLATFORM=windows
set ARCH=x86_64

REM Jump to main so we do not fall through into helper labels (CMD runs top-to-bottom).
goto :main

REM ----- Helper "functions" (only entered via call :label) -----
REM Check if a port is in use
:port_in_use
netstat -an | findstr ":%1" | findstr "LISTENING" >nul 2>&1
if %errorlevel%==0 (
    exit /b 0
) else (
    exit /b 1
)

REM Function to wait for a service to be ready
:wait_for_service
setlocal enabledelayedexpansion
set service_name=%~1
set port=%~2
set max_attempts=%~3
if "%max_attempts%"=="" set max_attempts=30
set check_url=%~4

echo %STEP_PREFIX% Waiting for %service_name%...
set attempt=0
:wait_loop
set /a attempt+=1
if %attempt% gtr %max_attempts% (
    echo %ERROR_PREFIX% %service_name% failed to start within %max_attempts% seconds
    exit /b 1
)

if not "%check_url%"=="" (
    REM HTTP health check using PowerShell
    powershell -Command "try { $response = Invoke-WebRequest -Uri '%check_url%' -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop; exit 0 } catch { exit 1 }" >nul 2>&1
    if %errorlevel%==0 (
        echo %SUCCESS_PREFIX% %service_name% is ready
        exit /b 0
    )
) else (
    REM TCP port check
    call :port_in_use %port%
    if %errorlevel%==0 (
        echo %SUCCESS_PREFIX% %service_name% is ready
        exit /b 0
    )
)

timeout /t 1 /nobreak >nul 2>&1
goto wait_loop

REM ----- Main sequence (services started in order) -----
:main
REM Start MongoDB
echo %STEP_PREFIX% Starting MongoDB...
set MONGOD_PATH=%RESOURCE_DIR%\databases\mongodb\%PLATFORM%\%ARCH%\mongod.exe
if not exist "%MONGOD_PATH%" (
    echo %ERROR_PREFIX% MongoDB binary not found at: %MONGOD_PATH%
    exit /b 1
)

REM Create MongoDB data directory
set MONGODB_DATA_DIR=%APP_DATA_DIR%\mongodb\data
if not exist "%MONGODB_DATA_DIR%" mkdir "%MONGODB_DATA_DIR%"

REM Start MongoDB
start /b "" "%MONGOD_PATH%" --dbpath "%MONGODB_DATA_DIR%" --port 27017 --bind_ip 127.0.0.1 --wiredTigerCacheSizeGB 0.5 > "%APP_DATA_DIR%\mongodb.log" 2>&1

REM Get PID (Windows doesn't have $! like bash, so we'll use tasklist)
timeout /t 2 /nobreak >nul 2>&1
for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq mongod.exe" /FO LIST ^| findstr "PID:"') do set MONGODB_PID=%%a

REM Wait for MongoDB
call :wait_for_service "MongoDB" 27017 30 ""
if errorlevel 1 goto mongo_failed
echo %PROGRESS_PREFIX% mongodb:ready
goto mongo_done
:mongo_failed
echo %PROGRESS_PREFIX% mongodb:failed
taskkill /F /IM mongod.exe >nul 2>&1
exit /b 1
:mongo_done

REM Start InfluxDB
echo %STEP_PREFIX% Starting InfluxDB...
set INFLUXD_PATH=%RESOURCE_DIR%\databases\influxdb\%PLATFORM%\%ARCH%\influxd.exe
if not exist "%INFLUXD_PATH%" (
    echo %ERROR_PREFIX% InfluxDB binary not found at: %INFLUXD_PATH%
    exit /b 1
)

REM Create InfluxDB data directory
set INFLUXDB_DATA_DIR=%APP_DATA_DIR%\influxdb\data
if not exist "%INFLUXDB_DATA_DIR%" mkdir "%INFLUXDB_DATA_DIR%"

REM Check if InfluxDB needs setup
set BOLT_PATH=%INFLUXDB_DATA_DIR%\influxdb.bolt
set NEEDS_SETUP=false
if not exist "%BOLT_PATH%" (
    set NEEDS_SETUP=true
)

REM Start InfluxDB
set INFLUXD_DATA_DIR=%INFLUXDB_DATA_DIR%
set INFLUXD_BOLT_PATH=%BOLT_PATH%
set INFLUXD_ENGINE_PATH=%INFLUXDB_DATA_DIR%\engine

start /b "" "%INFLUXD_PATH%" --http-bind-address 127.0.0.1:8086 --log-level error > "%APP_DATA_DIR%\influxdb.log" 2>&1

REM Get PID
timeout /t 2 /nobreak >nul 2>&1
for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq influxd.exe" /FO LIST ^| findstr "PID:"') do set INFLUXDB_PID=%%a

REM Wait for InfluxDB to be ready
call :wait_for_service "InfluxDB" 8086 30 "http://localhost:8086/health"
if errorlevel 1 goto influx_failed
echo %PROGRESS_PREFIX% influxdb:ready
REM Check if InfluxDB needs initial setup (flat flow to avoid nested if/else parsing)
timeout /t 2 /nobreak >nul 2>&1
powershell -Command "$response = Invoke-RestMethod -Uri 'http://localhost:8086/api/v2/setup' -Method Get -ErrorAction SilentlyContinue; if ($response.allowed -eq $true) { exit 0 } else { exit 1 }" >nul 2>&1
if errorlevel 1 set NEEDS_SETUP=true
if "%NEEDS_SETUP%"=="true" call :do_influx_setup
goto after_influx
:influx_failed
echo %PROGRESS_PREFIX% influxdb:failed
taskkill /F /IM influxd.exe >nul 2>&1
exit /b 1
:after_influx

REM Start Collector
echo %STEP_PREFIX% Starting Collector...
set COLLECTOR_BINARY=%RESOURCE_DIR%\binaries\sj-collector.exe
if not exist "%COLLECTOR_BINARY%" (
    echo %ERROR_PREFIX% Collector binary not found at: %COLLECTOR_BINARY%
    exit /b 1
)

REM Create storage directory
set STORAGE_DIR=%APP_DATA_DIR%\storage
if not exist "%STORAGE_DIR%" mkdir "%STORAGE_DIR%"

REM Start collector with environment variables
cd /d "%APP_DATA_DIR%"
set SERVER_HOST=0.0.0.0
set SERVER_PORT=8080
set JWT_SECRET=screenjournal-bundled-secret-key
set INFLUXDB2_URL=http://localhost:8086
set INFLUXDB2_TOKEN=screenjournal-admin-token
set INFLUXDB2_ORG=screenjournal-org
set INFLUXDB2_BUCKET=screenjournal-metrics
set STORAGE_BASE_PATH=%STORAGE_DIR%
set STORAGE_BASE_URL=http://localhost:8080/storage

start /b "" "%COLLECTOR_BINARY%" > "%APP_DATA_DIR%\collector.log" 2>&1

REM Get PID
timeout /t 2 /nobreak >nul 2>&1
for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq sj-collector.exe" /FO LIST ^| findstr "PID:"') do set COLLECTOR_PID=%%a

REM Wait for Collector
timeout /t 2 /nobreak >nul 2>&1
call :wait_for_service "Collector" 8080 30 "http://localhost:8080/health"
if errorlevel 1 goto collector_failed
echo %PROGRESS_PREFIX% collector:ready
goto collector_done
:collector_failed
echo %PROGRESS_PREFIX% collector:failed
taskkill /F /IM sj-collector.exe >nul 2>&1
exit /b 1
:collector_done

REM Start Report Service
echo %STEP_PREFIX% Starting Report Service...
set REPORT_BINARY=%RESOURCE_DIR%\binaries\sj-tracker-report.exe
if not exist "%REPORT_BINARY%" (
    echo %ERROR_PREFIX% Report service binary not found at: %REPORT_BINARY%
    exit /b 1
)

REM Start report service with environment variables
cd /d "%APP_DATA_DIR%"
set PORT=8085
set HOST=0.0.0.0
set INFLUXDB2_URL=http://localhost:8086
set INFLUXDB2_TOKEN=screenjournal-admin-token
set INFLUXDB2_ORG=screenjournal-org
set INFLUXDB2_BUCKET=screenjournal-metrics
set MONGODB_HOST=localhost
set MONGODB_PORT=27017
set MONGODB_DATABASE=reports
set MONGODB_USERNAME=admin
set MONGODB_PASSWORD=admin123
set MONGODB_AUTH_SOURCE=admin
set OPENAI_API_KEY=
REM Explicit path to Gemini key file (same location desktop app uses) so report backend can use it when frontend does not send key
set GEMINI_API_KEY_FILE=%APP_DATA_DIR%\gemini_api_key.txt

start /b "" "%REPORT_BINARY%" > "%APP_DATA_DIR%\report.log" 2>&1

REM Get PID
timeout /t 2 /nobreak >nul 2>&1
for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq sj-tracker-report.exe" /FO LIST ^| findstr "PID:"') do set REPORT_PID=%%a

REM Wait for Report Service
timeout /t 2 /nobreak >nul 2>&1
call :wait_for_service "Report Service" 8085 30 "http://localhost:8085/health"
if errorlevel 1 goto report_failed
echo %PROGRESS_PREFIX% report:ready
goto report_done
:report_failed
echo %PROGRESS_PREFIX% report:failed
taskkill /F /IM sj-tracker-report.exe >nul 2>&1
exit /b 1
:report_done

REM Start Chat Agent (using PyInstaller standalone executable)
echo %STEP_PREFIX% Starting Chat Agent...
set CHAT_AGENT_EXE=%RESOURCE_DIR%\python\sj-tracker-chat-agent\sj-chat-agent.exe

if not exist "%CHAT_AGENT_EXE%" (
    echo %ERROR_PREFIX% Chat agent executable not found at: %CHAT_AGENT_EXE%
    exit /b 1
)

REM Start chat agent using the standalone executable
cd /d "%APP_DATA_DIR%"
set BACKEND_URL=http://localhost:8085
set CHAT_AGENT_PORT=8087
set HOST=0.0.0.0

start /b "" "%CHAT_AGENT_EXE%" > "%APP_DATA_DIR%\chat-agent.log" 2>&1

REM Get PID
timeout /t 3 /nobreak >nul 2>&1
for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq sj-chat-agent.exe" /FO LIST ^| findstr "PID:"') do set CHAT_AGENT_PID=%%a

REM Wait for Chat Agent (give it more time - Python startup can be slow). Optional: do not exit on failure.
timeout /t 3 /nobreak >nul 2>&1
call :wait_for_service "Chat Agent" 8087 60 "http://localhost:8087/health"
if errorlevel 1 goto chat_optional_fail
echo %PROGRESS_PREFIX% chat_agent:ready
goto after_chat
:chat_optional_fail
echo %PROGRESS_PREFIX% chat_agent:failed
tasklist /FI "IMAGENAME eq sj-chat-agent.exe" | findstr "sj-chat-agent.exe" >nul 2>&1
if errorlevel 1 goto chat_exited
echo %PROGRESS_PREFIX% chat_agent:starting (process running, may start later)
goto after_chat
:chat_exited
echo %PROGRESS_PREFIX% chat_agent:failed (process exited)
:after_chat

REM Start Report Frontend (optional - requires Node.js)
echo %STEP_PREFIX% Starting Report Frontend...
set FRONTEND_DIR=%RESOURCE_DIR%\frontend\sj-tracker-frontend

REM Function to find Node.js
set NODE_EXE=
where node >nul 2>&1
if %errorlevel%==0 (
    for /f "delims=" %%i in ('where node') do set NODE_EXE=%%i
)

if not exist "%FRONTEND_DIR%" (
    echo %PROGRESS_PREFIX% frontend:skipped (Frontend directory not found at: %FRONTEND_DIR%)
) else if "%NODE_EXE%"=="" (
    echo %PROGRESS_PREFIX% frontend:skipped (Node.js not found - please install Node.js to use the report frontend)
) else (
    set NEXT_BIN=%FRONTEND_DIR%\node_modules\.bin\next.cmd
    
    if not exist "%NEXT_BIN%" (
        echo %PROGRESS_PREFIX% frontend:skipped (Next.js binary not found at: %NEXT_BIN%)
        if not exist "%FRONTEND_DIR%\.next" (
            echo %ERROR_PREFIX% Frontend not built - .next directory missing
        )
    ) else (
        REM Check if standalone build exists (Next.js standalone mode)
        set STANDALONE_DIR=%FRONTEND_DIR%\.next\standalone
        if exist "%STANDALONE_DIR%\server.js" (
            echo %STEP_PREFIX% Using Next.js standalone build
            cd /d "%STANDALONE_DIR%"
            set NODE_ENV=production
            set PORT=3030
            start /b "" "%NODE_EXE%" server.js > "%APP_DATA_DIR%\frontend.log" 2>&1
            
            REM Get PID
            timeout /t 2 /nobreak >nul 2>&1
            for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq node.exe" /FO LIST ^| findstr "PID:"') do set FRONTEND_PID=%%a
            
            echo %STEP_PREFIX% Started standalone server from: %STANDALONE_DIR%
        ) else (
            echo %STEP_PREFIX% Standalone build not found, trying standard Next.js build
            echo %STEP_PREFIX% Starting Next.js server from: %FRONTEND_DIR%
            echo %STEP_PREFIX% Using Node.js at: %NODE_EXE%
            cd /d "%FRONTEND_DIR%"
            set NODE_ENV=production
            start /b "" "%NODE_EXE%" "%NEXT_BIN%" start -p 3030 > "%APP_DATA_DIR%\frontend.log" 2>&1
            
            REM Get PID
            timeout /t 2 /nobreak >nul 2>&1
            for /f "tokens=2" %%a in ('tasklist /FI "IMAGENAME eq node.exe" /FO LIST ^| findstr "PID:"') do set FRONTEND_PID=%%a
        )
        echo %STEP_PREFIX% Frontend process started with PID: %FRONTEND_PID%
        
        timeout /t 3 /nobreak >nul 2>&1
        call :wait_for_service "Frontend" 3030 30 "http://localhost:3030"
        if errorlevel 1 goto frontend_wait_failed
        echo %PROGRESS_PREFIX% frontend:ready
        goto frontend_wait_done
        :frontend_wait_failed
        echo %PROGRESS_PREFIX% frontend:failed
        echo %ERROR_PREFIX% Frontend failed to start. Check logs at: %APP_DATA_DIR%\frontend.log
        tasklist /FI "IMAGENAME eq node.exe" | findstr "node.exe" >nul 2>&1
        if errorlevel 1 goto frontend_exited
        echo %STEP_PREFIX% Frontend process is still running, may start later
        goto frontend_wait_done
        :frontend_exited
        echo %ERROR_PREFIX% Frontend process exited
        :frontend_wait_done
    )
)

REM All services started
echo %SUCCESS_PREFIX% All services started successfully
echo %PROGRESS_PREFIX% all:ready

REM Keep script running and track PIDs
(
    echo MONGODB_PID=%MONGODB_PID%
    echo INFLUXDB_PID=%INFLUXDB_PID%
    echo COLLECTOR_PID=%COLLECTOR_PID%
    echo REPORT_PID=%REPORT_PID%
    echo CHAT_AGENT_PID=%CHAT_AGENT_PID%
    if not "%FRONTEND_PID%"=="" echo FRONTEND_PID=%FRONTEND_PID%
) > "%APP_DATA_DIR%\service_pids.txt"

REM Keep script running
:keep_alive
timeout /t 60 /nobreak >nul 2>&1
goto keep_alive

REM ----- InfluxDB setup subroutine (called only when NEEDS_SETUP is true) -----
:do_influx_setup
echo %STEP_PREFIX% Setting up InfluxDB (creating user, org, bucket)...
set SETUP_USERNAME=admin
set SETUP_PASSWORD=admin123
set SETUP_ORG=screenjournal-org
set SETUP_BUCKET=screenjournal-metrics
set SETUP_TOKEN=screenjournal-admin-token
powershell -Command "$body = @{username='%SETUP_USERNAME%'; password='%SETUP_PASSWORD%'; org='%SETUP_ORG%'; bucket='%SETUP_BUCKET%'; token='%SETUP_TOKEN%'} | ConvertTo-Json; $response = Invoke-RestMethod -Uri 'http://localhost:8086/api/v2/setup' -Method Post -Body $body -ContentType 'application/json' -ErrorAction SilentlyContinue; if ($response.user -or $response.auth) { Write-Host 'SUCCESS'; exit 0 } else { Write-Host 'FAILED'; exit 1 }" >nul 2>&1
if errorlevel 1 goto influx_setup_failed
timeout /t 1 /nobreak >nul 2>&1
powershell -Command "$response = Invoke-RestMethod -Uri 'http://localhost:8086/api/v2/setup' -Method Get -ErrorAction SilentlyContinue; if ($response.allowed -eq $false) { exit 0 } else { exit 1 }" >nul 2>&1
if errorlevel 1 goto influx_setup_verify_fail
echo %SUCCESS_PREFIX% InfluxDB setup verified
exit /b 0
:influx_setup_verify_fail
echo %ERROR_PREFIX% InfluxDB setup may have failed - verification failed
exit /b 0
:influx_setup_failed
echo %ERROR_PREFIX% InfluxDB setup failed
echo %ERROR_PREFIX% You may need to visit http://localhost:8086 to complete setup manually.
echo %ERROR_PREFIX% Use these credentials: Username: %SETUP_USERNAME% Password: %SETUP_PASSWORD% Org: %SETUP_ORG% Bucket: %SETUP_BUCKET%
exit /b 0

