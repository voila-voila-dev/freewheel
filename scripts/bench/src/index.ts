import { readBaseline, writeBaseline } from "#baseline.ts";
import { compare } from "#compare.ts";
import { formatReport } from "#format-report.ts";
import { measureAgentSize } from "#measure-agent-size.ts";
import { measureBinarySize } from "#measure-binary-size.ts";
import { measureInstructions } from "#measure-instructions.ts";
import type { MeasurementResult } from "#metrics.ts";
import { baselineFile, resultsFile } from "#paths.ts";

const updateBaseline = process.argv.includes("--update");

const results: ReadonlyArray<MeasurementResult> = [
	await measureBinarySize(),
	await measureInstructions(),
	await measureAgentSize(),
];
const metrics = Object.assign({}, ...results.map((result) => result.metrics));
const skipped = results.flatMap((result) => result.skipped);

await Bun.write(resultsFile, `${JSON.stringify(metrics, null, "\t")}\n`);
const rows = compare(await readBaseline(), metrics);
console.log(formatReport(rows, skipped));

if (updateBaseline) {
	await writeBaseline(metrics);
	console.log(`baseline updated: ${baselineFile}`);
} else if (rows.some((row) => !row.passed)) {
	console.error(
		"performance regression: see FAIL lines above (or `bun run bench --update` if intended)",
	);
	process.exitCode = 1;
}
