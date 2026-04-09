/**
 * Date Selector Component
 * 
 * Allows users to select which day to view activity data for.
 * Provides quick navigation buttons and forward/back arrows.
 */

"use client";

import { Calendar, ChevronLeft, ChevronRight } from "lucide-react";

interface DateSelectorProps {
  selectedDate: Date;
  onDateChange: (date: Date) => void;
}

export function DateSelector({ selectedDate, onDateChange }: DateSelectorProps) {
  
  // Navigate to previous day
  const goToPreviousDay = () => {
    const newDate = new Date(selectedDate);
    newDate.setDate(newDate.getDate() - 1);
    onDateChange(newDate);
  };

  // Navigate to next day
  const goToNextDay = () => {
    const newDate = new Date(selectedDate);
    newDate.setDate(newDate.getDate() + 1);
    onDateChange(newDate);
  };

  // Navigate to today
  const goToToday = () => {
    onDateChange(new Date());
  };

  // Navigate to yesterday
  const goToYesterday = () => {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    onDateChange(yesterday);
  };

  // Format date for display
  const formatDate = (date: Date) => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const compareDate = new Date(date);
    compareDate.setHours(0, 0, 0, 0);
    
    if (compareDate.getTime() === today.getTime()) {
      return "Today";
    }
    
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);
    
    if (compareDate.getTime() === yesterday.getTime()) {
      return "Yesterday";
    }
    
    return date.toLocaleDateString(undefined, { 
      weekday: "short", 
      month: "short", 
      day: "numeric",
      year: date.getFullYear() !== today.getFullYear() ? "numeric" : undefined
    });
  };

  // Format date for input field (YYYY-MM-DD)
  const formatInputDate = (date: Date) => {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  };

  // Handle date input change
  const handleDateInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newDate = new Date(e.target.value);
    if (!isNaN(newDate.getTime())) {
      onDateChange(newDate);
    }
  };

  // Check if selected date is today
  const isToday = () => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const selected = new Date(selectedDate);
    selected.setHours(0, 0, 0, 0);
    return selected.getTime() === today.getTime();
  };

  // Check if next day would be in the future
  const isNextDayFuture = () => {
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(0, 0, 0, 0);
    const nextDay = new Date(selectedDate);
    nextDay.setDate(nextDay.getDate() + 1);
    nextDay.setHours(0, 0, 0, 0);
    return nextDay.getTime() >= tomorrow.getTime();
  };

  return (
    <div className="bg-white rounded-lg p-4 shadow-md">
      <h3 className="text-sm font-semibold text-gray-700 mb-3 flex items-center gap-2">
        <Calendar className="w-4 h-4" />
        Select Date
      </h3>

      {/* Date Display with Navigation */}
      <div className="flex items-center gap-2 mb-3">
        <button
          onClick={goToPreviousDay}
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          aria-label="Previous day"
        >
          <ChevronLeft className="w-5 h-5 text-gray-600" />
        </button>
        
        <div className="flex-1 text-center font-semibold text-gray-900">
          {formatDate(selectedDate)}
        </div>
        
        <button
          onClick={goToNextDay}
          disabled={isNextDayFuture()}
          className={`p-2 rounded-lg transition-colors ${
            isNextDayFuture() 
              ? "opacity-30 cursor-not-allowed" 
              : "hover:bg-gray-100"
          }`}
          aria-label="Next day"
        >
          <ChevronRight className="w-5 h-5 text-gray-600" />
        </button>
      </div>

      {/* Date Picker Input */}
      <div className="mb-3">
        <input
          type="date"
          value={formatInputDate(selectedDate)}
          onChange={handleDateInputChange}
          max={formatInputDate(new Date())}
          className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
      </div>

      {/* Quick Navigation Buttons */}
      <div className="space-y-2">
        <button
          onClick={goToToday}
          disabled={isToday()}
          className={`w-full px-3 py-2 text-sm font-medium rounded-lg transition-colors ${
            isToday()
              ? "bg-blue-100 text-blue-700 cursor-default"
              : "bg-gray-100 text-gray-700 hover:bg-gray-200"
          }`}
        >
          Today
        </button>
        
        <button
          onClick={goToYesterday}
          className="w-full px-3 py-2 text-sm font-medium bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg transition-colors"
        >
          Yesterday
        </button>
      </div>

      {/* Additional Info */}
      {/* <div className="mt-4 pt-3 border-t border-gray-200">
        <div className="text-xs text-gray-500">
          Selected: {selectedDate.toLocaleDateString()}
        </div>
        
      </div> */}
    </div>
  );
}

