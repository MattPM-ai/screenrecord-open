/**
 * ============================================================================
 * USER MENU COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Display user authentication status and provide quick access to auth actions
 * SCOPE: Login/Register buttons for unauthenticated users, profile menu for authenticated users
 * DEPENDENCIES: authAPI for authentication state and logout
 * 
 * FEATURES:
 * - Dynamic display based on authentication state
 * - Login/Register buttons for unauthenticated users
 * - User profile dropdown for authenticated users
 * - Logout functionality
 * - Responsive design
 * 
 * ============================================================================
 */

'use client';

import { useState, useEffect, useRef } from 'react';
import Link from 'next/link';
import { getDefaultUser, logout, type User } from '@/lib/localTypes';

export default function UserMenu() {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isOpen, setIsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Use default user for local/bundled app (no auth backend)
    setUser(getDefaultUser());
    setIsLoading(false);
  }, []);

  const handleLogout = () => {
    logout();
    setIsOpen(false);
    // No redirect needed for local version - just close menu
  };

  const toggleDropdown = () => {
    setIsOpen(!isOpen);
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (isOpen && menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen]);

  if (isLoading) {
    return (
      <div className="flex items-center gap-2">
        <div className="w-5 h-5 border-2 border-gray-200 border-t-blue-600 rounded-full animate-spin"></div>
      </div>
    );
  }

  // Use default user if profile not loaded
  const displayUser = user || getDefaultUser();

  return (
    <div ref={menuRef} className="relative">
      <button
        onClick={toggleDropdown}
        className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-gray-900 bg-transparent border-none rounded-md cursor-pointer hover:bg-gray-50 transition-colors min-w-0"
      >
        <div className="w-8 h-8 bg-blue-600 rounded-full flex items-center justify-center flex-shrink-0">
          <span className="text-white text-sm font-medium">
            {displayUser.name ? displayUser.name.charAt(0).toUpperCase() : displayUser.email.charAt(0).toUpperCase()}
          </span>
        </div>
        <span className="max-w-[120px] overflow-hidden text-ellipsis whitespace-nowrap flex-shrink-0">
          {displayUser.name || displayUser.email}
        </span>
        <svg 
          className={`w-4 h-4 transition-transform ${isOpen ? 'rotate-180' : ''}`}
          fill="none" 
          stroke="currentColor" 
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {/* Dropdown Menu */}
      {isOpen && (
        <div className="absolute right-0 mt-2 w-48 bg-white rounded-lg shadow-lg border border-gray-200 py-1 z-50">
          <div className="px-3 py-2 border-b border-gray-200">
            <p className="text-sm font-medium text-gray-900 mb-1">
              {displayUser.name || 'Local User'}
            </p>
            <p className="text-xs text-gray-500 overflow-hidden text-ellipsis whitespace-nowrap">
              {displayUser.email}
            </p>
          </div>
          
          <div className="px-3 py-2 text-xs text-gray-500 italic">
            Local Mode - No Authentication
          </div>
        </div>
      )}
    </div>
  );
}
