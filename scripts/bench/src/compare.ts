import { gateFor } from "#gates.ts";
import type { Metrics } from "#metrics.ts";

export interface Comparison {
	readonly metric: string;
	readonly baseline: number | undefined;
	readonly current: number;
	/** Growth as a share of the baseline; undefined without a baseline. */
	readonly growth: number | undefined;
	readonly passed: boolean;
}

/** Compares current metrics to the baseline; a metric without a baseline always passes. */
export const compare = (
	baseline: Metrics,
	current: Metrics,
): ReadonlyArray<Comparison> =>
	Object.entries(current)
		.sort(([a], [b]) => a.localeCompare(b))
		.map(([metric, value]) => {
			const previous = baseline[metric];
			if (previous === undefined || previous === 0) {
				return {
					metric,
					baseline: previous,
					current: value,
					growth: undefined,
					passed: true,
				};
			}
			const gate = gateFor(metric);
			const growth = (value - previous) / previous;
			const allowed = gate
				? Math.max(previous * gate.maxGrowth, gate.toleratedAbsolute)
				: Number.POSITIVE_INFINITY;
			return {
				metric,
				baseline: previous,
				current: value,
				growth,
				passed: value - previous <= allowed,
			};
		});
