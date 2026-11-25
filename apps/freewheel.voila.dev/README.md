# freewheel.voila.dev

The site of freewheel on [freewheel.voila.dev](https://freewheel.voila.dev): Astro, fully static, served by Cloudflare Workers static assets, built with [`@voila.dev/site`](https://github.com/voila-voila-dev/site).

```sh
bun install
bun run dev      # http://localhost:4321
bun run build    # dist/
```

- Its own install, outside the root workspaces.
- **Docs** (`/docs`) are the repository's `docs/` folder, read at build time (`src/lib/docs-loader.ts`): edit the Markdown there. The navigation is `src/data/docs.ts`; `docs/technical/plans/` stays on GitHub.
- **Demos** (`/demos`): the drive replays the three recorded LTE uplinks of the `trace-drive` bench. `scripts/drive.ts` downloads them (Mahimahi, GPL-3.0, never committed) and writes `src/data/drive.json` before each build.
- Bench numbers, scenarios and roadmap in `src/data/bench.ts`: update them from `bench/README.md` and `docs/technical/decisions.md` when a CI bench run lands.
- Push to `main` deploys (`.github/workflows/site.yml` at the root); a pull request uploads a preview version and comments its URL.
