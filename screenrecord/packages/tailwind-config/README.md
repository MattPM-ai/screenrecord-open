# @repo/tailwind-config

Shared Tailwind CSS configuration for ScreenRecord monorepo.

## Overview

This package provides a centralized Tailwind CSS configuration that ensures consistent styling across all applications in the monorepo. It extends Tailwind with custom theme tokens that integrate with the `@repo/theme` CSS variables.

## Usage

### In Next.js Apps

Create a `tailwind.config.js` or `tailwind.config.ts` in your app:

```javascript
const sharedConfig = require('@repo/tailwind-config');

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './src/**/*.{js,jsx,ts,tsx}',
    '../../packages/ui/src/**/*.{ts,tsx}',
  ],
  presets: [sharedConfig],
};
```

### PostCSS Configuration

For PostCSS setup, import the shared config:

```javascript
const config = {
  plugins: {
    '@tailwindcss/postcss': {},
  },
};

export default config;
```

## Features

- **CSS Variable Integration**: All theme colors reference CSS custom properties from `@repo/theme`
- **Extended Border Radius**: Custom radius values tied to theme variables
- **Chart Colors**: Pre-configured chart color palette
- **OKLCH Color Space**: Modern color space for better perceptual uniformity

## Customization

Apps can extend the shared configuration by adding their own customizations in their local Tailwind config files:

```javascript
const sharedConfig = require('@repo/tailwind-config');

module.exports = {
  presets: [sharedConfig],
  theme: {
    extend: {
      // App-specific customizations
    },
  },
};
```

## Dependencies

- `@tailwindcss/postcss`: ^4.0.0
- `tailwindcss`: ^4.0.0
- `tw-animate-css`: ^1.4.0

