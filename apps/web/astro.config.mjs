// @ts-check
import { defineConfig } from "astro/config";
import react from "@astrojs/react";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";

// Static site for GitHub Pages: https://tanzir71.github.io/pare/
// React components from @pare/ui are rendered to HTML at build time (no client:* directives).
export default defineConfig({
  site: "https://tanzir71.github.io",
  base: "/pare",
  // Pages serves directories with a trailing slash; keep canonical URLs and the sitemap consistent.
  trailingSlash: "always",
  output: "static",
  integrations: [react(), sitemap()],
  build: {
    // Small page: inline the CSS to save a render-blocking request.
    inlineStylesheets: "always",
    format: "directory",
  },
  vite: {
    plugins: [tailwindcss()],
  },
});
