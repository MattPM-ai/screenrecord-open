"use client";

/**
 * Settings Page
 * 
 * Standalone page for the settings window. This page is opened in a separate
 * Tauri window when the user clicks the settings button in the main dashboard.
 * 
 * The page renders the Settings component as a full-page layout rather than
 * a modal overlay. The splash screen is hidden via CSS in the settings layout.
 */

import { Settings } from "../components/Settings";

export default function SettingsPage() {
  return (
    <div className="min-h-screen bg-white">
      <Settings />
    </div>
  );
}

