import type { MeasurementResult } from "#metrics.ts";
import { hotPathsBinary, trackedCrates } from "#paths.ts";
import { runOrThrow } from "#run-command.ts";
import { sumSymbolSizesByPrefix } from "#symbol-sizes.ts";

/** Size of the hot-path binary, and how much of it each tracked crate takes. */
export const measureBinarySize = async (): Promise<MeasurementResult> => {
	await runOrThrow([
		"cargo",
		"build",
		"--quiet",
		"--profile",
		"size-report",
		"--package",
		"benchmarks",
		"--bin",
		"hot-paths",
	]);
	const binaryBytes = Bun.file(hotPathsBinary).size;
	const nm = await runOrThrow([
		"nm",
		"--print-size",
		"--demangle",
		hotPathsBinary,
	]);
	const benchmarks = (await runOrThrow([hotPathsBinary, "--list"])).stdout
		.split("\n")
		.filter(Boolean);
	// With LTO, crate code is inlined into each hot loop: the benchmark functions carry it.
	const prefixes = {
		...Object.fromEntries(
			trackedCrates.map((crate) => [`crate.${crate}`, `${crate}::`]),
		),
		...Object.fromEntries(
			benchmarks.map((name) => [
				`hot-path.${name}`,
				`benchmarks::benchmarks::${name}::`,
			]),
		),
	};
	const sizes = sumSymbolSizesByPrefix(nm.stdout, prefixes);
	return {
		metrics: {
			"size.binary.hot-paths.bytes": binaryBytes,
			...Object.fromEntries(
				Object.entries(sizes).map(([label, bytes]) => [
					`size.${label}.bytes`,
					bytes,
				]),
			),
		},
		skipped: [],
	};
};
