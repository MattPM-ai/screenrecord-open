// Package services: Gemini API key file helpers.
// Single source of truth for the Gemini API key file used by the desktop app (Tauri) and report backend.
// Path: GEMINI_API_KEY_FILE env, else APP_DATA_DIR/gemini_api_key.txt, else cwd/gemini_api_key.txt
package services

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// GetGeminiKeyFilePath returns the path to the shared Gemini API key file.
// Used by the settings API so the report frontend and desktop app share the same key.
func GetGeminiKeyFilePath() string {
	if p := strings.TrimSpace(os.Getenv("GEMINI_API_KEY_FILE")); p != "" {
		return p
	}
	dir := os.Getenv("APP_DATA_DIR")
	if dir == "" {
		dir, _ = os.Getwd()
	}
	if dir == "" {
		return ""
	}
	return filepath.Join(dir, "gemini_api_key.txt")
}

// ReadGeminiKeyFromFile reads the Gemini API key from the shared key file only (no env/request).
// Returns empty string if file missing or empty.
func ReadGeminiKeyFromFile() string {
	path := GetGeminiKeyFilePath()
	if path == "" {
		return ""
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

// WriteGeminiKeyToFile writes the Gemini API key to the shared key file.
// Creates parent directory if needed. Used when user saves the key from the report frontend.
func WriteGeminiKeyToFile(key string) error {
	path := GetGeminiKeyFilePath()
	if path == "" {
		return fmt.Errorf("cannot determine Gemini key file path (set GEMINI_API_KEY_FILE or APP_DATA_DIR)")
	}
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0700); err != nil {
		return fmt.Errorf("create key file dir: %w", err)
	}
	return os.WriteFile(path, []byte(strings.TrimSpace(key)), 0600)
}
