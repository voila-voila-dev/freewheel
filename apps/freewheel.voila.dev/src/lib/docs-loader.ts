// The docs of freewheel.voila.dev are the repository's docs/ folder, read at build time: one
// source, edited next to the code. Each Markdown file becomes a page; its first heading is the
// title, its first paragraph the description. Links between published pages become site links;
// every other relative link (code, plans, bench results) points to the file on GitHub.
import { readdir, readFile } from "node:fs/promises";
import { join, posix, relative, resolve } from "node:path";
import type { Loader } from "astro/loaders";
import { repo } from "../data/bench";

/** The repository's docs/, from the site's folder (apps/freewheel.voila.dev). */
const DOCS = resolve(process.cwd(), "../../docs");
/** Handoff plans stay on GitHub: they are working notes, not documentation. */
const UNPUBLISHED = /^technical\/plans\//;

/** docs/guide/concepts/delay.md → guide/delay; docs/technical/README.md → technical. */
export function slugOf(path: string): string {
	return path
		.replace(/\.md$/, "")
		.replace(/^guide\/concepts\//, "guide/")
		.replace(/\/README$/, "");
}

async function markdownFiles(dir: string): Promise<string[]> {
	const entries = await readdir(dir, { withFileTypes: true });
	const nested = await Promise.all(
		entries.map((e) =>
			e.isDirectory()
				? markdownFiles(join(dir, e.name))
				: e.name.endsWith(".md")
					? [join(dir, e.name)]
					: [],
		),
	);
	return nested.flat();
}

function plain(text: string): string {
	return text
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
		.replace(/[`*_]/g, "")
		.replace(/\s+/g, " ")
		.trim();
}

/** The first block of prose (not a heading, list, table, quote or code), if it comes first. */
function lead(body: string): string | undefined {
	const block =
		body
			.trim()
			.split(/\n\s*\n/)[0]
			?.trim() ?? "";
	return block && !/^(#|[-*|>]|\d+\.|```)/.test(block) ? block : undefined;
}

/** Rewrites the relative links of one file (path relative to docs/). */
function rewriteLinks(
	body: string,
	path: string,
	published: Set<string>,
): string {
	return body.replace(
		/(\]\()([^)\s]+)(\))/g,
		(match, open: string, href: string, close: string) => {
			if (/^([a-z]+:|#|\/)/i.test(href)) return match;
			const [target, hash = ""] = href.split("#");
			const resolved = posix.normalize(posix.join(posix.dirname(path), target));
			const anchor = hash ? `#${hash}` : "";
			const asFile = resolved.replace(/\/$/, "");
			if (published.has(asFile))
				return `${open}/docs/${slugOf(asFile)}${anchor}${close}`;
			if (published.has(`${asFile}/README.md`))
				return `${open}/docs/${slugOf(`${asFile}/README.md`)}${anchor}${close}`;
			// Outside docs/ or unpublished: the file (or folder) on GitHub.
			const fromRoot = posix.normalize(posix.join("docs", resolved));
			const kind =
				target.endsWith("/") || !/\.[a-z0-9]+$/i.test(target) ? "tree" : "blob";
			return `${open}${repo}/${kind}/main/${fromRoot}${anchor}${close}`;
		},
	);
}

export function docsLoader(): Loader {
	return {
		name: "freewheel-docs",
		load: async ({ store, renderMarkdown }) => {
			const files = (await markdownFiles(DOCS))
				.map((file) => relative(DOCS, file).split("\\").join("/"))
				.filter((path) => !UNPUBLISHED.test(path));
			const published = new Set(files);
			store.clear();
			for (const path of files) {
				const raw = await readFile(join(DOCS, path), "utf8");
				const heading = raw.match(/^#\s+(.+)$/m);
				const title = plain(heading?.[1] ?? slugOf(path));
				let body = rewriteLinks(
					heading ? raw.replace(heading[0], "") : raw,
					path,
					published,
				);
				// The opening paragraph becomes the page's lead (and meta description), once.
				const opening = lead(body);
				if (opening) body = body.replace(opening, "");
				store.set({
					id: slugOf(path),
					data: {
						title,
						description: opening ? plain(opening) : title,
						source: `docs/${path}`,
					},
					body,
					rendered: await renderMarkdown(body),
				});
			}
		},
	};
}
