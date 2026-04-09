/**
 * SplashScreen Component
 * 
 * Displays an initial loading splash screen with the ScreenRecord logo,
 * app name, and a spinner. This component renders as part of the
 * initial HTML and is dismissed by page.tsx after React hydrates.
 */

// ScreenRecord logo SVG component
const SplashLogo = () => (
  <svg width="40" height="90" viewBox="0 0 153.06 343.77" xmlns="http://www.w3.org/2000/svg">
    <path 
      transform="translate(-30.47 55.406)" 
      d="m104.94-53.634a10.799 10.799 0 0 0-7.3721 5.9518l-60.033 128.37a53.776 53.776 0 0 0-1.2753 42.607l62.584 157.79a9.1583 9.1583 0 0 0 17.039-0.0312l61.894-157.73a54.353 54.353 0 0 0-1.2681-42.678l-59.357-128.29a10.799 10.799 0 0 0-12.21-5.9917zm-1.6924 197.56h7.5075a8.0688 9.9149 0 0 1 8.0688 9.915v50.733a8.0688 9.9149 0 0 1-8.0688 9.915h-7.5075a8.0688 9.9149 0 0 1-8.0688-9.915v-50.733a8.0688 9.9149 0 0 1 8.0688-9.915z" 
      fill="#000000"
    />
  </svg>
);

// Inline styles for splash screen (injected into head for instant rendering)
export const SplashScreenStyles = () => (
  <style dangerouslySetInnerHTML={{ __html: `
    #splash-screen {
      position: fixed;
      inset: 0;
      z-index: 9999;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      background-color: white;
      transition: opacity 0.3s ease-out;
    }
    #splash-screen.splash-hidden {
      opacity: 0;
      pointer-events: none;
    }
    .splash-name {
      font-size: 20px;
      font-weight: 600;
      color: black;
      margin-top: 24px;
      margin-bottom: 32px;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    }
    .splash-spinner {
      width: 24px;
      height: 24px;
      border: 2px solid rgba(0, 0, 0, 0.1);
      border-top-color: black;
      border-radius: 50%;
      animation: splash-spin 0.8s linear infinite;
    }
    @keyframes splash-spin {
      to { transform: rotate(360deg); }
    }
  `}} />
);

// Main splash screen component
export const SplashScreen = () => (
  <div id="splash-screen">
    <SplashLogo />
    <div className="splash-name">ScreenRecord Tracker</div>
    <div className="splash-spinner" />
  </div>
);

