# Lighthouse

Mobile audit of the landing page, run locally on 2026-11-18 against the production build.

| Category       | Median of 5 | Range over 14 runs |
| -------------- | ----------- | ------------------ |
| Performance    | 100         | 93–100             |
| Accessibility  | 100         | 100                |
| Best Practices | 100         | 100                |
| SEO            | 100         | 100                |

Typical metrics: FCP 0.9 s, LCP 1.2 s, TBT 0–130 ms, CLS 0, Speed Index 0.9 s. The one run under 95 (TBT 310 ms) came straight after a batch of browser automation on the same machine; TBT is the only metric that moves.

The privacy page scored 100 in all four categories.

## How it was run

```bash
pnpm --filter web build
pnpm --filter web preview            # http://localhost:4321/pare/
CHROME_PATH="<path to Chrome or Edge>" \
  pnpm dlx lighthouse@12.8.2 http://localhost:4321/pare/ \
  --port=9333 --chrome-flags="--headless=new --no-first-run" --output=html --output-path=./lighthouse/report
```

- Lighthouse 12.8.2, default mobile config (emulated Moto G Power, simulated slow 4G, 4× CPU slowdown).
- Browser: Microsoft Edge (Chromium) headless on Windows 11; Chrome wasn't installed. `--port` works around chrome-launcher not finding Edge's dynamic debugging port.
- Reports go to `lighthouse/`, which is git-ignored. Don't commit them.

## What keeps it fast

- No client JavaScript besides the ~1 KB inline mobile-menu script. `@pare/ui` React components render to HTML at build time.
- CSS is inlined (one request for the page). Geist's Latin subset is preloaded; other subsets load only if their glyphs appear.
- Sections below the hero use `content-visibility: auto` with per-breakpoint size estimates (`--cis`), so the first paint only lays out the hero.
