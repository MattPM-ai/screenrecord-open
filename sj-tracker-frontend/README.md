# ScreenRecord Tracker Frontend

A minimalist, light-theme chat interface built with Next.js and TypeScript, designed to communicate with an n8n webhook.

## Features

- 🎨 Minimalist, aesthetic design with light theme
- 💬 Real-time chat interface
- 📱 Fully responsive design
- ⚡ Built with Next.js 14 and TypeScript
- 🚀 Optimized for Vercel deployment

## Getting Started

### Prerequisites

- Node.js 18+ 
- npm or yarn

### Installation

1. Install dependencies:
```bash
npm install
```

2. Run the development server:
```bash
npm run dev
```

3. Open [http://localhost:3000](http://localhost:3000) in your browser.

## Deployment

This project is configured for Vercel deployment. Simply connect your repository to Vercel and deploy.

The webhook URL is configured in `app/api/chat/route.ts`.

## Project Structure

```
├── app/
│   ├── api/
│   │   └── chat/
│   │       └── route.ts      # API route for webhook communication
│   ├── globals.css            # Global styles and theme variables
│   ├── layout.tsx             # Root layout component
│   └── page.tsx               # Main page component
├── components/
│   ├── Chat.tsx               # Main chat component
│   └── Chat.module.css        # Chat component styles
└── package.json
```

## Technologies

- **Next.js 14** - React framework with App Router
- **TypeScript** - Type-safe JavaScript
- **CSS Modules** - Scoped component styling

## License

MIT

# sj-tracker-frontend
