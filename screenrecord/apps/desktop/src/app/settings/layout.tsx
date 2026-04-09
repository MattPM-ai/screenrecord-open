/**
 * Settings Layout
 * 
 * Minimal layout for the settings window. Uses CSS to hide the splash screen
 * rather than modifying DOM directly (which causes hydration mismatches).
 */

import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Settings - ScreenRecord Tracker",
  description: "Configure ScreenRecord Tracker settings",
};

export default function SettingsLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <>
      {/* CSS-based splash screen hiding - avoids hydration mismatch */}
      <style dangerouslySetInnerHTML={{
        __html: `#splash-screen { display: none !important; }`
      }} />
      {children}
    </>
  );
}

