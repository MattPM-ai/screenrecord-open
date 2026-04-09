import type { Config } from 'tailwindcss';
import sharedConfig from '@repo/tailwind-config';

const config: Config = {
  content: [
    './src/**/*.{ts,tsx}',
    // UI package styles come from pre-built CSS import
  ],
  presets: [sharedConfig],
};

export default config;

