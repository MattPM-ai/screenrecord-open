# Turbo Organization Test Data

Comprehensive test dataset for the ScreenRecord collector system, containing realistic activity data for 8 users across 9 days (November 15-23, 2025).

## 📊 Overview

This test data simulates real-world employee monitoring data for the **Turbo** organization, featuring:

- **8 distinct users** with unique behavioral profiles and roles
- **9 days of data** including weekdays (Nov 17-21) and weekends (Nov 15-16, 22-23)
- **6,830 total records** across 4 measurement types
- **Realistic patterns** including work hours, breaks, AFK periods, and app usage
- **InfluxDB line protocol format** for InfluxDB 3 Core

### Data Volume

| Measurement | Records | Description |
|-------------|---------|-------------|
| `window_activity` | 5,128 | Fine-grained window/app switching events |
| `app_usage` | 1,272 | Aggregated hourly app usage statistics |
| `afk_status` | 382 | User activity status changes (active/idle/afk) |
| `daily_metrics` | 48 | Daily rollup metrics per user |
| **TOTAL** | **6,830** | |

---

## 👥 User Profiles

### alice - Admin Staff
- **Hostnames:** `alice-laptop`
- **Work Hours:** 9:00 AM - 5:00 PM
- **Apps:** Outlook, Chrome, Excel
- **Pattern:** Highly productive, short breaks, minimal AFK
- **Utilization:** 85-92%

### ben - Accountant
- **Hostnames:** `ben-desktop`, `ben-laptop`
- **Work Hours:** 9:00 AM - 5:30 PM
- **Apps:** Xero, Excel, Outlook, Chrome, Reddit
- **Pattern:** Moderate productivity, long lunches (60-75 min), frequent app switching
- **Utilization:** 70-78%

### chaz - Developer
- **Hostnames:** `chaz-workstation`, `chaz-home`
- **Work Hours:** 8:30 AM - 6:00 PM (+ weekend activity)
- **Apps:** VSCode, Terminal, Slack, Chrome
- **Pattern:** Highly productive, deep work sessions, minimal breaks
- **Utilization:** 90-95%
- **Weekend Work:** Saturday 2-5 PM, Sunday 10 AM-12 PM

### danielle - HR
- **Hostnames:** `danielle-desktop`
- **Work Hours:** 9:00 AM - 5:00 PM
- **Apps:** Teams, Outlook, DocuSign, Chrome
- **Pattern:** Mixed productivity, meeting-heavy, moderate breaks
- **Utilization:** 75-82%

### eric - Sales
- **Hostnames:** `eric-laptop`
- **Work Hours:** 9:30 AM - 5:00 PM (+ Saturday work)
- **Apps:** Salesforce, Chrome, Outlook, Zoom, YouTube, Twitter
- **Pattern:** Fluctuating productivity, midday idle periods, high app switching
- **Utilization:** 55-70%
- **Weekend Work:** Saturday 9-11 AM (sales prep)

### fern - Manager
- **Hostnames:** `fern-desktop`
- **Work Hours:** 9:00 AM - 3:30 PM (leaves early) (+ Sunday evening)
- **Apps:** Teams, Chrome, Calendar, Slack, Outlook
- **Pattern:** Balanced productivity, many meetings, early departure
- **Utilization:** 78-85%
- **Weekend Work:** Sunday 8-9 PM (email check)

### grace - Support
- **Hostnames:** `grace-workstation`
- **Work Hours:** 9:00 AM - 5:00 PM
- **Apps:** Zendesk, Chrome, Outlook, Spotify
- **Pattern:** Productive but unpredictable AFK periods (5-45 min breaks)
- **Utilization:** 72-80%

### hazel - Designer
- **Hostnames:** `hazel-imac`, `hazel-ipad`
- **Work Hours:** 10:30 AM - 6:30 PM (late start)
- **Apps:** Figma, Photoshop, Chrome, Slack, Instagram
- **Pattern:** Creative sporadic work, frequent short breaks
- **Utilization:** 68-76%

---

## 📁 File Structure

```
test-data/
├── README.md                          # This file
├── generate_data.py                   # Data generation script
├── influxdb/
│   ├── app_usage.lp                  # App usage line protocol
│   ├── afk_status.lp                 # AFK status line protocol
│   ├── window_activity.lp            # Window activity line protocol
│   └── daily_metrics.lp              # Daily metrics line protocol
├── scripts/
│   └── upload_with_curl.sh           # Upload script using curl
└── metadata/
    ├── user_profiles.json             # User behavioral specifications
    └── data_manifest.json             # Generation metadata
```

---

## 🚀 Usage Instructions

### InfluxDB Upload

#### Prerequisites
- InfluxDB 3 Core instance running
- `curl` command-line tool (pre-installed on most systems)
- Valid InfluxDB token
- Database created

#### Upload Data

**Automated upload:**
```bash
./upload_with_curl.sh screenrecord-metrics-dev "your-token-here" "http://195.74.52.54:8181"
```

**Manual upload (single file with curl):**
```bash
# Remove comments and upload
grep -v '^#' influxdb/app_usage.lp | grep -v '^$' | \
  curl -X POST "http://localhost:8181/api/v3/write_lp?db=screenrecord-metrics-dev" \
    -H "Authorization: Token your-token-here" \
    -H "Content-Type: text/plain" \
    --data-binary @-
```

#### Verify Upload

**Check record counts with curl:**
```bash
# Query using InfluxDB SQL
curl -X POST "http://localhost:8181/api/v3/query/sql" \
  -H "Authorization: Token your-token-here" \
  -H "Content-Type: application/json" \
  -d '{
    "db": "screenrecord-metrics-dev",
    "query": "SELECT COUNT(*) FROM app_usage"
  }'

# List all users
curl -X POST "http://localhost:8181/api/v3/query/sql" \
  -H "Authorization: Token your-token-here" \
  -H "Content-Type: application/json" \
  -d '{
    "db": "screenrecord-metrics-dev",
    "query": "SELECT DISTINCT user FROM app_usage ORDER BY user"
  }'
```

#### Sample Queries (InfluxDB SQL)

```sql
-- Most active user (by window events)
SELECT user, COUNT(*) as count
FROM window_activity
GROUP BY user
ORDER BY count DESC
LIMIT 1;

-- Top applications by total usage time
SELECT app_name, SUM(duration_seconds) as total_seconds
FROM app_usage
GROUP BY app_name
ORDER BY total_seconds DESC
LIMIT 10;

-- Average utilization by user
SELECT user, AVG(utilization_ratio) as avg_util
FROM daily_metrics
GROUP BY user
ORDER BY avg_util DESC;

-- Hourly activity distribution
SELECT 
  DATE_BIN(INTERVAL '1 hour', time, '2025-11-17T00:00:00Z') as hour,
  COUNT(*) as events
FROM window_activity
WHERE time >= '2025-11-17T00:00:00Z'
  AND time < '2025-11-22T00:00:00Z'
GROUP BY hour
ORDER BY hour;
```

---

## 🔄 Regenerating Data

The test data can be regenerated with different patterns or variations:

```bash
cd test-data
python3 generate_data.py
```

To customize the data:
1. Edit `metadata/user_profiles.json` to modify user behaviors
2. Edit `generate_data.py` to adjust generation logic
3. Run the script to regenerate all line protocol files

---

## 📋 Data Schema Reference

### app_usage

Aggregated application usage per hour.

| Column | Type | Description |
|--------|------|-------------|
| `id` | SERIAL | Auto-incrementing primary key |
| `app_name` | VARCHAR(255) | Application name |
| `duration_seconds` | INTEGER | Total usage duration in seconds |
| `event_count` | INTEGER | Number of window events |
| `hostname` | VARCHAR(255) | User's machine hostname |
| `org` | VARCHAR(255) | Organization name (always "Turbo") |
| `user` | VARCHAR(255) | Username |
| `time` | TIMESTAMP | Timestamp of aggregation period |

### afk_status

User activity status changes (active, idle, away from keyboard).

| Column | Type | Description |
|--------|------|-------------|
| `id` | SERIAL | Auto-incrementing primary key |
| `duration` | INTEGER | Duration in this status (seconds) |
| `hostname` | VARCHAR(255) | User's machine hostname |
| `org` | VARCHAR(255) | Organization name |
| `status` | VARCHAR(32) | Status: 'active', 'idle', or 'afk' |
| `user` | VARCHAR(255) | Username |
| `time` | TIMESTAMP | Status change timestamp |

### window_activity

Fine-grained window focus and application switching events.

| Column | Type | Description |
|--------|------|-------------|
| `id` | SERIAL | Auto-incrementing primary key |
| `app` | VARCHAR(255) | Application name |
| `duration` | INTEGER | Window focus duration (seconds) |
| `hostname` | VARCHAR(255) | User's machine hostname |
| `org` | VARCHAR(255) | Organization name |
| `user` | VARCHAR(255) | Username |
| `time` | TIMESTAMP | Event timestamp |
| `title` | VARCHAR(255) | Window title |

### daily_metrics

Aggregated daily metrics per user.

| Column | Type | Description |
|--------|------|-------------|
| `id` | SERIAL | Auto-incrementing primary key |
| `date` | DATE | Date of metrics |
| `hostname` | VARCHAR(255) | User's machine hostname |
| `org` | VARCHAR(255) | Organization name |
| `user` | VARCHAR(255) | Username |
| `active_seconds` | INTEGER | Total active time |
| `afk_seconds` | INTEGER | Total AFK time |
| `app_switches` | INTEGER | Number of app switches |
| `idle_seconds` | INTEGER | Total idle time |
| `utilization_ratio` | FLOAT | Active / (active + idle + afk) |
| `time` | TIMESTAMP | End of day timestamp |

---

## 🎯 Use Cases

### Testing Analytics Queries
- User productivity analysis
- Application usage patterns
- Time tracking validation
- AFK detection accuracy

### Performance Testing
- InfluxDB query optimization
- Time-series aggregation
- Large dataset handling
- Query response times

### Dashboard Development
- Real-time monitoring interfaces
- Historical trend visualization
- User comparison views
- Alert threshold testing

### Integration Testing
- WebSocket data streaming
- REST endpoint testing
- Data serialization
- Pagination testing

---

## 🔍 Data Quality Notes

### Realistic Patterns Included

✅ **Natural variance** in work start/end times  
✅ **Lunch breaks** of varying lengths (30-90 minutes)  
✅ **Coffee breaks** throughout the day  
✅ **Weekend activity** for specific users (chaz, eric, fern)  
✅ **Multiple hostnames** for users who work from different machines  
✅ **App switching behavior** matching user roles  
✅ **Utilization variations** from 55% (eric) to 95% (chaz)  
✅ **Realistic window titles** for each application  

### Edge Cases & Outliers

The dataset intentionally includes:
- Users with low productivity (eric: 55-70%)
- Early departures (fern leaves at 3:30 PM)
- Late starts (hazel starts at 10:30 AM)
- Long AFK periods (grace: unpredictable 5-45 min breaks)
- Weekend work patterns
- High app switching (ben, eric: 80-150 switches/day)

---

## 🛠 Troubleshooting

### InfluxDB Issues

**Error: "database not found" (404)**
```bash
# Create the database first with curl
curl -X POST "http://localhost:8181/api/v3/configure/database" \
  -H "Authorization: Token your-token" \
  -H "Content-Type: application/json" \
  -d '{"name": "screenrecord-metrics-dev"}'
```

**Error: "authentication failed"**
- Check your token is valid
- Verify the token has write permissions
- Ensure the token hasn't expired

**Error: "connection refused"**
- Verify InfluxDB is running: `curl http://localhost:8181/health`
- Check the correct host URL (HTTP vs HTTPS)
- Ensure firewall isn't blocking the port

**Error: "SSL routines::wrong version number"**
- You're using HTTPS but server only supports HTTP
- Change `https://` to `http://` in the URL

**Data counts don't match expected**
- Check for duplicate uploads (InfluxDB may deduplicate based on timestamp)
- Verify all files were uploaded successfully
- Use curl to query and verify counts (see examples above)

---

## 📝 Metadata Files

### user_profiles.json
Complete behavioral specifications for each user including:
- Work hours and variance
- Application lists (productive/neutral/unproductive)
- AFK patterns and break behaviors
- Weekend activity schedules

### data_manifest.json
Generation metadata including:
- Timestamp of data generation
- Exact record counts per measurement
- Date range coverage
- Output formats

**Generated:** 21st November 2025  
**Organization:** Turbo  
**Version:** 1.0  
**Format:** InfluxDB 3 Line Protocol

