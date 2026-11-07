<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->

## Site rules

- Visual changes start as a standalone HTML mock in `../design/` and wait for approval. Copy and data changes do not.
- Every claim about the app must be true today. Label unreleased work "Next release" and agent work "In development" with a link to issue 18.
- Numbers come from measurements recorded in `../SPEC.md` with their machine and method. Never round them into new claims.
- Release data is fetched, not hardcoded. Keep zod validation on every GitHub response.
- No card framing, no code comments, no emojis, no em dashes in copy.
- Run `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm build` before handing off.
