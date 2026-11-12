import { parseInstructionCount } from "#cachegrind-instructions.ts";
import type { MeasurementResult } from "#metrics.ts";
import { hotPathsBinary } from "#paths.ts";
import { isInstalled, runOrThrow } from "#run-command.ts";

const BASE_PACKETS = 10_000;
const EXTRA_PACKETS = 100_000;

const instructionsFor = async (benchmark: string, packets: number) => {
	const output = await runOrThrow([
		"valgrind",
		"--tool=cachegrind",
		"--cache-sim=no",
		"--cachegrind-out-file=/dev/null",
		hotPathsBinary,
		benchmark,
		String(packets),
	]);
	return parseInstructionCount(output.stderr);
};

/**
 * Instructions per packet for every benchmark, counted by Cachegrind: deterministic, so CI
 * can gate on it. Two runs, and the difference divided by the extra packets, removes startup.
 */
export const measureInstructions = async (): Promise<MeasurementResult> => {
	if (!isInstalled("valgrind")) {
		return {
			metrics: {},
			skipped: [
				{
					measurement: "instructions per packet",
					reason:
						"Valgrind is not installed (Linux only: run in CI or in an OrbStack VM)",
				},
			],
		};
	}
	const list = await runOrThrow([hotPathsBinary, "--list"]);
	const benchmarks = list.stdout.split("\n").filter(Boolean);
	const metrics: Record<string, number> = {};
	for (const benchmark of benchmarks) {
		const base = await instructionsFor(benchmark, BASE_PACKETS);
		const more = await instructionsFor(benchmark, BASE_PACKETS + EXTRA_PACKETS);
		metrics[`instructions.${benchmark}.per_packet`] =
			Math.round(((more - base) / EXTRA_PACKETS) * 100) / 100;
	}
	return { metrics, skipped: [] };
};
