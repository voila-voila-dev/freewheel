import sitemap from "@astrojs/sitemap";
import { codeTheme } from "@voila.dev/site/lib/code-theme";
import { defineConfig } from "astro/config";

export default defineConfig({
	site: "https://freewheel.voila.dev",
	trailingSlash: "never",
	build: { format: "directory" },
	integrations: [sitemap()],
	redirects: { "/docs": "/docs/guide" },
	markdown: { shikiConfig: { theme: codeTheme } },
});
