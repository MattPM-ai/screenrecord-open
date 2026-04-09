/**
 * ============================================================================
 * SERVICES MODULE
 * ============================================================================
 * 
 * PURPOSE: Manages all backend services (Go binaries, Python agent, databases)
 *          for the bundled ScreenRecord application
 * 
 * This module handles:
 * - Launching and managing sj-collector Go backend
 * - Launching and managing sj-tracker-report Go backend  
 * - Launching and managing Python chat agent
 * - Managing embedded databases (SQLite for MongoDB replacement)
 * - Service lifecycle (start on app launch, stop on app exit)
 * 
 * ============================================================================
 */

pub mod manager;
pub mod types;




