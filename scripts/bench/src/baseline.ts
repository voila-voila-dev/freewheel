import type { Metrics } from "#metrics.ts";
import { baselineFile } from "#paths.ts";

interface BaselineFile {
	readonly updated: string;
	readonly metrics: Metrics;
}

/** Metrics committed in `bench/baseline.<platform>.json`; empty when there is none yet. */
export const readBaseline = async (): Promise<Metrics> => {
	const file = Bun.file(baselineFile);
	if (!(await file.exists())) return {};
	const content = (await file.json()) as BaselineFile;
	return content.metrics;
};

/** Merges `metrics` into the baseline, keeping metrics measured elsewhere (e.g. CI-only). */
export const writeBaseline = async (metrics: Metrics): Promise<void> => {
	const previous = await readBaseline();
	const content: BaselineFile = {
		updated: new Date().toISOString().slice(0, 10),
		metrics: Object.fromEntries(
			Object.entries({ ...previous, ...metrics }).sort(([a], [b]) =>
				a.localeCompare(b),
			),
		),
	};
	await Bun.write(baselineFile, `${JSON.stringify(content, null, "\t")}\n`);
};
