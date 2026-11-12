import type { Comparison } from "#compare.ts";
import type { Skipped } from "#metrics.ts";

const formatValue = (metric: string, value: number | undefined): string => {
	if (value === undefined) return "—";
	if (metric.endsWith(".bytes")) return `${(value / 1024).toFixed(1)} KiB`;
	return String(value);
};

const formatGrowth = (growth: number | undefined): string =>
	growth === undefined
		? "new"
		: `${growth >= 0 ? "+" : ""}${(growth * 100).toFixed(2)} %`;

/** A plain-text table, readable in a terminal and in CI logs. */
export const formatReport = (
	rows: ReadonlyArray<Comparison>,
	skipped: ReadonlyArray<Skipped>,
): string => {
	const lines = rows.map(
		(row) =>
			`${row.passed ? "ok  " : "FAIL"} ${row.metric.padEnd(46)} ${formatValue(row.metric, row.baseline).padStart(12)} → ${formatValue(row.metric, row.current).padStart(12)}  ${formatGrowth(row.growth)}`,
	);
	const notes = skipped.map((s) => `skip ${s.measurement}: ${s.reason}`);
	return [...lines, ...notes].join("\n");
};
