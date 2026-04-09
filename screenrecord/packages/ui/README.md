# @repo/ui

Shared UI component library for ScreenRecord monorepo.

## Overview

This package contains reusable React components built with:
- **Radix UI** primitives for accessibility
- **Tailwind CSS** for styling
- **Framer Motion** for animations
- **TypeScript** for type safety

## Components

### Available Components

- `Accordion` - Collapsible content sections
- `AIPromptBox` - AI-powered prompt input
- `AnimatedRadialChart` - Animated data visualization
- `AuthForm` - Authentication form component
- `Badge` - Status and category badges
- `BentoGrid` - Grid layout for feature sections
- `Button` - Primary button component
- `Drawer` - Slide-out panel component
- `FAQSection` - Frequently asked questions display
- `HowItWorks` - Feature explanation component
- `ThemeMetaUpdater` - Dynamic theme meta tags
- `ThemeToggle` - Light/dark mode switcher

## Usage

### Import Components

```typescript
import { Button, Badge, ThemeToggle } from '@repo/ui';
// Or import specific components
import { Button } from '@repo/ui/components/button';
```

### Import Utilities

```typescript
import { cn } from '@repo/ui/utils';

// Usage
<div className={cn('base-class', conditionalClass && 'conditional-class')} />
```

## Development

### Adding New Components

1. Create component file in `src/components/`
2. Export component in `src/components/index.ts`
3. Ensure TypeScript types are properly defined
4. Follow existing component patterns for consistency

### Type Checking

```bash
npm run check-types
```

### Linting

```bash
npm run lint
```

## Dependencies

All component dependencies are managed at the package level. Apps consuming this package only need React as a peer dependency.

## Integration

This package is designed to work seamlessly with:
- `@repo/theme` - Shared theming and CSS variables
- `@repo/tailwind-config` - Tailwind configuration

Ensure both packages are properly configured in consuming applications.