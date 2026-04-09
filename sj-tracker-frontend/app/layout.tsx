/**
 * ============================================================================
 * ROOT LAYOUT COMPONENT
 * ============================================================================
 * 
 * PURPOSE: Provides the root HTML structure and metadata for the application
 * 
 * DESCRIPTION:
 * This component wraps all pages in the application and sets up the basic
 * HTML structure, metadata, and global styles.
 * 
 * ============================================================================
 */

import type { Metadata } from 'next'
import './globals.css'
import NavBar from '@/components/NavBar'

export const metadata: Metadata = {
  title: 'ScreenRecord',
  description: 'ScreenRecord tracker interface',
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en">
      <body>
        <NavBar />
        <div style={{ marginTop: '64px' }}>
          {children}
        </div>
      </body>
    </html>
  )
}

