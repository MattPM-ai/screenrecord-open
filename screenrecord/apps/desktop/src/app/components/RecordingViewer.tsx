"use client";

/**
 * ============================================================================
 * RECORDING VIEWER COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Display and navigate multi-display screen recording segments
 * 
 * FEATURES:
 * - HTML5 video player with controls
 * - Multi-display support (tabs for each display)
 * - Keyboard navigation between recordings
 * - Metadata display (duration, size, resolution per display)
 * 
 * ============================================================================
 */

import { useEffect, useState, useRef, useCallback } from "react";
import {
  getRecordingsByDateRange,
  getRecordingUrl,
  formatDuration,
  formatFileSize,
  type RecordingMetadata,
  type DisplayRecording,
} from "@/lib/recordingClient";
import { X, ChevronLeft, ChevronRight, Monitor, Play, Pause } from "lucide-react";

type RecordingViewerProps = {
  startTime: string;
  endTime: string;
  onClose: () => void;
};

export function RecordingViewer({
  startTime,
  endTime,
  onClose,
}: RecordingViewerProps) {
  const [recordings, setRecordings] = useState<RecordingMetadata[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number>(0);
  const [selectedDisplayIndex, setSelectedDisplayIndex] = useState<number>(0);
  const [loading, setLoading] = useState(true);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [recordingsDir, setRecordingsDir] = useState<string>("");
  const videoRef = useRef<HTMLVideoElement>(null);

  // Load recordings on mount
  useEffect(() => {
    loadRecordings();
  }, [startTime, endTime]);

  const loadRecordings = async () => {
    try {
      setLoading(true);
      const response = await getRecordingsByDateRange(startTime, endTime);
      // Sort by start time (newest first from backend, but let's sort by oldest first for viewing)
      const sorted = response.recordings.sort(
        (a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime()
      );
      setRecordings(sorted);
    } catch (error) {
      console.error("Failed to load recordings:", error);
    } finally {
      setLoading(false);
    }
  };

  const currentRecording = recordings[selectedIndex];
  const currentDisplay = currentRecording?.displays[selectedDisplayIndex];

  // Get the video file path for the current display
  const getVideoPath = (recording: RecordingMetadata, display: DisplayRecording): string => {
    // The filename is stored in the display, but we need the full path
    // The recordings are stored in app_data_dir/recordings/YYYY-MM-DD/filename
    const date = new Date(recording.start_time).toISOString().split('T')[0];
    // We'll use Tauri's asset protocol - the path is relative to the recordings dir
    // For now, construct a path that the backend can resolve
    return `recordings/${date}/${display.filename}`;
  };

  const handlePrevious = () => {
    setSelectedIndex((prev) => Math.max(0, prev - 1));
    setSelectedDisplayIndex(0);
  };

  const handleNext = () => {
    setSelectedIndex((prev) => Math.min(recordings.length - 1, prev + 1));
    setSelectedDisplayIndex(0);
  };

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "ArrowLeft") handlePrevious();
    if (e.key === "ArrowRight") handleNext();
    if (e.key === "Escape") onClose();
    if (e.key === " ") {
      e.preventDefault();
      togglePlayPause();
    }
  }, [selectedIndex, recordings.length, onClose]);

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // Reset video when changing recordings or displays
  useEffect(() => {
    if (videoRef.current) {
      videoRef.current.currentTime = 0;
      setCurrentTime(0);
      setIsPlaying(false);
    }
  }, [selectedIndex, selectedDisplayIndex]);

  const togglePlayPause = () => {
    if (videoRef.current) {
      if (isPlaying) {
        videoRef.current.pause();
      } else {
        videoRef.current.play();
      }
      setIsPlaying(!isPlaying);
    }
  };

  const handleTimeUpdate = () => {
    if (videoRef.current) {
      setCurrentTime(videoRef.current.currentTime);
    }
  };

  const handleTimelineClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!videoRef.current || !currentRecording) return;

    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percent = x / rect.width;
    const newTime = percent * currentRecording.duration_seconds;

    videoRef.current.currentTime = newTime;
    setCurrentTime(newTime);
  };

  if (loading) {
    return (
      <div className="absolute inset-0 bg-black bg-opacity-90 flex items-center justify-center z-40">
        <div className="text-white text-xl">Loading recordings...</div>
      </div>
    );
  }

  if (!currentRecording || !currentDisplay) {
    return (
      <div className="absolute inset-0 bg-black bg-opacity-90 flex flex-col items-center justify-center z-40">
        <div className="text-white text-xl mb-4">
          No recordings found for this time range
        </div>
        <button
          onClick={onClose}
          className="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition"
        >
          Close
        </button>
      </div>
    );
  }

  // Build video URL using Tauri's asset protocol
  const videoPath = getVideoPath(currentRecording, currentDisplay);
  const videoUrl = getRecordingUrl(videoPath);

  return (
    <div className="absolute inset-0 bg-black bg-opacity-95 flex flex-col z-40">
      {/* Header */}
      <div className="flex items-center justify-between p-4 bg-black bg-opacity-50">
        <div className="text-white">
          <p className="text-lg font-semibold">
            Recording Segment
          </p>
          <p className="text-sm text-gray-300">
            {new Date(currentRecording.start_time).toLocaleString()}
          </p>
          <p className="text-xs text-gray-400 mt-1">
            Duration: {formatDuration(currentRecording.duration_seconds)} • 
            {currentRecording.display_count} display{currentRecording.display_count !== 1 ? 's' : ''} • 
            {currentRecording.framerate} fps
          </p>
        </div>
        <button
          onClick={onClose}
          className="text-white hover:text-gray-300 transition"
          aria-label="Close viewer"
        >
          <X className="w-8 h-8" />
        </button>
      </div>

      {/* Display Tabs (if multiple displays) */}
      {currentRecording.display_count > 1 && (
        <div className="flex bg-gray-900 px-4">
          {currentRecording.displays.map((display, idx) => (
            <button
              key={display.display_index}
              onClick={() => setSelectedDisplayIndex(idx)}
              className={`flex items-center gap-2 px-4 py-2 text-sm transition ${
                selectedDisplayIndex === idx
                  ? "bg-blue-600 text-white"
                  : "text-gray-400 hover:text-white hover:bg-gray-800"
              }`}
            >
              <Monitor className="w-4 h-4" />
              Display {display.display_index + 1}
              <span className="text-xs opacity-70">
                ({display.width}x{display.height})
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Video Player */}
      <div className="flex-1 flex items-center justify-center p-4 overflow-hidden">
        <div className="relative max-w-full max-h-full">
          <video
            ref={videoRef}
            src={videoUrl}
            className="max-w-full max-h-[60vh] rounded-lg shadow-2xl"
            onTimeUpdate={handleTimeUpdate}
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            onEnded={() => setIsPlaying(false)}
          />
          
          {/* Monitor badge */}
          <div className="absolute top-2 left-2 bg-black bg-opacity-70 text-white px-3 py-1 rounded text-sm flex items-center gap-2">
            <Monitor className="w-4 h-4" />
            Display {currentDisplay.display_index + 1} • {currentDisplay.width}x{currentDisplay.height}
          </div>

          {/* Play/Pause overlay */}
          <button
            onClick={togglePlayPause}
            className="absolute inset-0 flex items-center justify-center bg-black bg-opacity-0 hover:bg-opacity-20 transition group"
          >
            <div className="opacity-0 group-hover:opacity-100 transition bg-black bg-opacity-50 rounded-full p-4">
              {isPlaying ? (
                <Pause className="w-12 h-12 text-white" />
              ) : (
                <Play className="w-12 h-12 text-white" />
              )}
            </div>
          </button>
        </div>
      </div>

      {/* Timeline and Progress */}
      <div className="px-4 pb-2">
        {/* Progress bar */}
        <div 
          className="h-2 bg-gray-700 rounded-full cursor-pointer mb-2"
          onClick={handleTimelineClick}
        >
          <div 
            className="h-full bg-blue-500 rounded-full transition-all"
            style={{ 
              width: `${(currentTime / currentRecording.duration_seconds) * 100}%` 
            }}
          />
        </div>

        {/* Time display */}
        <div className="flex justify-between text-xs text-gray-400 mb-2">
          <span>{formatDuration(currentTime)}</span>
          <span>{formatDuration(currentRecording.duration_seconds)}</span>
        </div>
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between p-4 bg-black bg-opacity-50">
        <button
          onClick={handlePrevious}
          disabled={selectedIndex === 0}
          className="flex items-center text-white disabled:text-gray-600 disabled:cursor-not-allowed hover:text-gray-300 transition px-4 py-2 rounded"
        >
          <ChevronLeft className="w-6 h-6" />
          Previous
        </button>
        
        <div className="text-white text-center">
          <p className="text-lg font-semibold">
            {selectedIndex + 1} / {recordings.length}
          </p>
          <p className="text-xs text-gray-400">
            Space to play/pause • Arrow keys to navigate
          </p>
        </div>
        
        <button
          onClick={handleNext}
          disabled={selectedIndex === recordings.length - 1}
          className="flex items-center text-white disabled:text-gray-600 disabled:cursor-not-allowed hover:text-gray-300 transition px-4 py-2 rounded"
        >
          Next
          <ChevronRight className="w-6 h-6" />
        </button>
      </div>

      {/* Metadata Panel */}
      <div className="p-4 bg-black bg-opacity-70 text-white text-sm">
        <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
          <div>
            <p className="text-gray-400">Total Size</p>
            <p className="font-semibold">
              {formatFileSize(currentRecording.total_file_size_bytes)}
            </p>
          </div>
          <div>
            <p className="text-gray-400">This Display</p>
            <p className="font-semibold">
              {formatFileSize(currentDisplay.file_size_bytes)}
            </p>
          </div>
          <div>
            <p className="text-gray-400">Resolution</p>
            <p className="font-semibold">
              {currentDisplay.width}x{currentDisplay.height}
            </p>
          </div>
          <div>
            <p className="text-gray-400">Framerate</p>
            <p className="font-semibold">{currentRecording.framerate} fps</p>
          </div>
          <div>
            <p className="text-gray-400">Format</p>
            <p className="font-semibold">{currentRecording.format.toUpperCase()} ({currentRecording.codec})</p>
          </div>
        </div>
      </div>
    </div>
  );
}
