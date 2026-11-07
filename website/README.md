# Agentic PDF website

The public site for Agentic PDF: home, download, agent, changelog and privacy pages. Built with Next.js 16 (App Router, Cache Components), React 19 and plain CSS ported from the approved mocks in `../design/`.

## Develop

```
pnpm install
pnpm dev
```

## Checks

```
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

## Data

Release information comes from the GitHub REST API for `joelfickson/pdf-reader` and is cached for an hour (`cacheLife("hours")`, tag `releases`). Responses are validated with zod, and a failed or malformed response fails the build instead of rendering stale or invented data.

Unauthenticated requests are limited by GitHub. Set `GITHUB_TOKEN` (a token with public read access) in the deployment environment to raise the limit. The site works without it.

## Styles

Shared styles live in `src/app/globals.css`. Each page keeps its own stylesheet next to it, scoped under a page class such as `.download-page`, so class names from the mocks can be reused without collisions. Light and dark follow the visitor's system setting.
