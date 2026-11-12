import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import type { TraceSource } from "#sources.ts";

/** Downloads every file of `source` into `cacheDir/<collection>`, skipping files already there. */
export const downloadSource = async (
	source: TraceSource,
	cacheDir: string,
): Promise<ReadonlyArray<string>> => {
	const folder = join(cacheDir, source.collection);
	await mkdir(folder, { recursive: true });
	const fetched: string[] = [];
	for (const file of source.files) {
		const target = Bun.file(join(folder, file));
		if (await target.exists()) continue;
		const response = await fetch(`${source.baseUrl}/${file}`);
		if (!response.ok) {
			throw new Error(`${file}: HTTP ${response.status}`);
		}
		await Bun.write(target, await response.arrayBuffer());
		fetched.push(file);
	}
	return fetched;
};
