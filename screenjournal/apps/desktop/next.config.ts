import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Enable static export for Tauri packaging
  output: 'export',
  // Disable image optimization server for static export
  images: { unoptimized: true },
  // Transpile monorepo packages
  transpilePackages: ['@repo/ui', '@repo/theme'],
  devIndicators: {
    position: 'top-left',
  },
  // Exclude directories from Next.js file watching and compilation
  experimental: {
    // This helps prevent Next.js from scanning certain directories
  },
  // Add empty turbopack config to silence Next.js 16 warning
  // We're using webpack explicitly via --webpack flag for memory optimization
  turbopack: {},
  webpack: (config, { isServer, webpack }) => {
    // Memory optimization: Reduce webpack's memory usage
    config.optimization = {
      ...config.optimization,
      // Reduce memory usage during build
      moduleIds: 'deterministic',
      chunkIds: 'deterministic',
      // Disable expensive optimizations that use more memory
      removeAvailableModules: false,
      removeEmptyChunks: false,
      mergeDuplicateChunks: false,
    };
    
    // Limit webpack parallelism to reduce memory usage
    // This reduces the number of worker processes spawned
    // Set to 1 for minimal memory usage (single-threaded, but safer)
    config.parallelism = 1;
    
    // Disable cache to reduce memory usage (slower but uses less RAM)
    config.cache = false;
    
    // Externalize Tauri plugins - they're only available in Tauri runtime
    // This prevents Next.js from trying to bundle them during build
    if (!isServer) {
      // Use IgnorePlugin to completely ignore these modules during build
      config.plugins.push(
        new webpack.IgnorePlugin({
          resourceRegExp: /^@tauri-apps\/plugin-(updater|process)$/,
        })
      );
      
      // Also add to resolve.alias as fallback
      config.resolve.alias = {
        ...config.resolve.alias,
        '@tauri-apps/plugin-updater': false,
        '@tauri-apps/plugin-process': false,
      };
    }
    
    // Ignore the frontend directory and bundle directory (separate Next.js apps)
    // This prevents Next.js from trying to compile the report frontend
    config.plugins.push(
      new webpack.IgnorePlugin({
        checkResource: (resource: string) => {
          // Ignore anything in src-tauri/resources/frontend or src-tauri/target
          return /src-tauri\/(resources\/frontend|target)/.test(resource);
        },
      })
    );
    
    return config;
  },
};

export default nextConfig;
