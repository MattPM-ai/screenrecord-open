# ScreenRecord Monorepo

A Turborepo-powered monorepo for the ScreenRecord project containing shared packages and multiple applications.

## Project Structure

```
screenrecord/
├── apps/
│   ├── desktop/          # Tauri + Next.js desktop application
│   └── landing/          # Marketing landing page (Next.js)
├── packages/
│   ├── ui/               # Shared React UI components
│   ├── theme/            # Shared theming and CSS variables
│   ├── tailwind-config/  # Shared Tailwind CSS configuration
│   ├── eslint-config/    # Shared ESLint configuration
│   └── typescript-config/# Shared TypeScript configuration
├── package.json
├── turbo.json
└── README.md
```

## Getting Started
### Prerequisites

- Node.js >= 18
- npm 11.4.2 (or compatible package manager)

### Installation

From the root directory, install all dependencies:

```bash
npm install
```

This will install dependencies for all packages and applications in the monorepo.

## Development


### Run All Apps in Development Mode

```bash
npm run dev
```

### Run Specific App

```bash
npm run dev --filter=landing
npm run dev --filter=desktop
```

### Build All Apps

```bash
npm run build
```

### Lint All Code

```bash
npm run lint
```

### Type Check All Code

```bash
npm run check-types
```

### Format Code

```bash
npm run format
```

## Applications

### Desktop App (`apps/desktop`)

The desktop application built with Tauri and Next.js. Provides native desktop functionality with web technologies.

**Tech Stack:**
- Next.js 16
- React 19
- Tauri 2
- TypeScript
- Tailwind CSS v4

**Development:**
```bash
cd apps/desktop
npm run dev          # Run Next.js dev server
npm run tauri:dev    # Run Tauri desktop app
npm run tauri:build  # Build desktop app
```

### Landing Page (`apps/landing`)

Marketing website and landing page.

**Tech Stack:**
- Next.js 15
- React 19
- Tailwind CSS v4
- MDX for blog content

**Development:**
```bash
cd apps/landing
npm run dev    # Run development server
npm run build  # Build for production
```

## Shared Packages

### `@repo/ui`

Shared React component library with:
- Radix UI primitives
- Tailwind CSS styling
- Framer Motion animations
- TypeScript support

**Key Components:**
- Accordion, Badge, Button, Drawer
- ThemeToggle, ThemeMetaUpdater
- AIPromptBox, AnimatedRadialChart

**Usage:**
```typescript
import { Button, Badge, ThemeToggle } from '@repo/ui';
```

### `@repo/theme`

Centralized theming system with:
- CSS custom properties for colors, spacing, shadows
- Light and dark mode support
- Prism.js syntax highlighting theme

**Usage:**
```css
@import "@repo/theme/globals.css";
@import "@repo/theme/prism-theme.css";
```

### `@repo/tailwind-config`

Shared Tailwind CSS configuration with theme tokens integrated with CSS variables.

**Usage:**
```javascript
const sharedConfig = require('@repo/tailwind-config');

module.exports = {
  content: ['./src/**/*.{js,jsx,ts,tsx}'],
  presets: [sharedConfig],
};
```

### `@repo/eslint-config`

ESLint configurations for different project types:
- `base.js` - Base configuration
- `next.js` - Next.js specific rules
- `react-internal.js` - Internal React libraries

### `@repo/typescript-config`

Shared TypeScript configurations:
- `base.json` - Base configuration
- `nextjs.json` - Next.js specific settings
- `react-library.json` - React library settings

## Workflow

### Adding a New Package

1. Create directory in `packages/`
2. Add `package.json` with name `@repo/package-name`
3. Add to workspaces in root `package.json` (if not using wildcard)
4. Install dependencies from root: `npm install`

### Adding a New App

1. Create directory in `apps/`
2. Add `package.json`
3. Configure to use shared packages
4. Install dependencies from root: `npm install`

### Using Shared Packages in Apps

1. Add dependency in app's `package.json`:
```json
{
  "dependencies": {
    "@repo/ui": "*",
    "@repo/theme": "*"
  }
}
```

2. Configure transpilation in `next.config.js`:
```javascript
{
  transpilePackages: ['@repo/ui', '@repo/theme']
}
```

3. Add path aliases in `tsconfig.json`:
```json
{
  "compilerOptions": {
    "paths": {
      "@repo/ui": ["../../packages/ui/src"],
      "@repo/theme": ["../../packages/theme/src"]
    }
  }
}
```

## Turborepo Features

- **Intelligent caching**: Build outputs are cached and reused
- **Remote caching**: Share cache with team members (configure with `turbo login`)
- **Parallel execution**: Tasks run in parallel when possible
- **Task pipelines**: Dependencies between tasks are respected

## Scripts Reference

| Script | Description |
|--------|-------------|
| `npm run dev` | Start all apps in development mode |
| `npm run build` | Build all apps and packages |
| `npm run lint` | Lint all code |
| `npm run check-types` | Type check all TypeScript code |
| `npm run format` | Format code with Prettier |

## Environment Variables

Each app can have its own `.env` files:
- `.env` - Default environment variables
- `.env.local` - Local overrides (gitignored)
- `.env.production` - Production variables

## Deployment

### Desktop App

```bash
cd apps/desktop
npm run tauri:build
```

Outputs will be in `apps/desktop/src-tauri/target/release/`

### Landing Page

```bash
npm run build --filter=landing
```

Output will be in `apps/landing/.next/`

## Contributing

1. Create a feature branch
2. Make your changes
3. Run `npm run lint` and `npm run check-types`
4. Test your changes in relevant apps
5. Submit a pull request

## Troubleshooting

### Cache Issues

Clear Turborepo cache:
```bash
npx turbo clean
```

### Dependency Issues

Reinstall all dependencies:
```bash
rm -rf node_modules apps/*/node_modules packages/*/node_modules
npm install
```

### Type Errors

Rebuild type definitions:
```bash
npm run check-types
```

## License

[Your License Here]

## Links

- [Turborepo Documentation](https://turborepo.com/docs)
- [Next.js Documentation](https://nextjs.org/docs)
- [Tauri Documentation](https://tauri.app/)

