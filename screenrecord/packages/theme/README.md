# @repo/theme

Shared theme package for ScreenRecord monorepo containing design tokens, CSS variables, and styling foundations.

## Overview

This package provides a centralized theming system with:
- CSS custom properties for colors, spacing, shadows, and typography
- Light and dark mode support
- Prism.js syntax highlighting theme for code blocks
- Reusable CSS utilities and components

## Usage

### In Next.js Apps

Import the global styles in your app's root CSS file:

```css
@import "@repo/theme/globals.css";
@import "@repo/theme/prism-theme.css";
```

Or in your `globals.css`:

```css
@import "tailwindcss";
@import "@repo/theme/globals.css";
```

### Theme Variables

The theme provides CSS variables for:

- **Colors**: `--background`, `--foreground`, `--primary`, `--secondary`, etc.
- **Spacing**: `--spacing`, `--radius`
- **Shadows**: `--shadow-sm`, `--shadow-md`, `--shadow-lg`, etc.
- **Typography**: `--tracking-normal`, `--tracking-wide`, etc.

### Dark Mode

The theme automatically supports dark mode via the `.dark` class selector. Theme variables adjust automatically when dark mode is active.

## Package Exports

```javascript
{
  "./globals.css": "./src/globals.css",
  "./prism-theme.css": "./src/prism-theme.css"
}
```

## Dependencies

No external dependencies - pure CSS.

## Maintenance

When updating theme variables, ensure consistency across both light and dark modes for accessibility and visual harmony.

