import { join } from "node:path";
import { downloadSource } from "#download.ts";
import { sources } from "#sources.ts";

const cacheDir = join(import.meta.dir, "..", "..", "..", ".cache", "traces");

for (const source of sources) {
	const fetched = await downloadSource(source, cacheDir);
	console.log(
		`${source.collection}: ${fetched.length} downloaded, ${source.files.length - fetched.length} already cached`,
	);
}
