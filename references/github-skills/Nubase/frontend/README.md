# Nubase Frontend

Monorepo containing nubase web surfaces.

```
frontend/
├── apps/
│   ├── studio/   # Admin dashboard (port 3000)
│   └── www/      # Marketing site (port 3001)
└── packages/
    ├── ui/       # Shared component library
    └── config/   # Shared tailwind / tsconfig presets
```

## Setup

```bash
pnpm install
pnpm dev:studio   # http://localhost:3000
pnpm dev:www      # http://localhost:3001
```

Studio talks to the Java backend at `http://localhost:9999` by default.
Override via `NEXT_PUBLIC_NUBASE_API_URL`.
