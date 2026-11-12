/** How much a metric may grow before CI fails. */
export interface Gate {
	/** Metrics this gate applies to: a name prefix. */
	readonly prefix: string;
	/** Allowed growth, as a share of the baseline (0.02 = +2 %). */
	readonly maxGrowth: number;
	/** Growth always tolerated in absolute terms, so tiny metrics do not fail on noise. */
	readonly toleratedAbsolute: number;
}

/** First matching prefix wins. Thresholds from `docs/technical/research/05-embedded-performance.md`. */
export const gates: ReadonlyArray<Gate> = [
	{ prefix: "instructions.", maxGrowth: 0.02, toleratedAbsolute: 0 },
	{ prefix: "size.agent.", maxGrowth: 0.03, toleratedAbsolute: 0 },
	{ prefix: "size.", maxGrowth: 0.02, toleratedAbsolute: 16 * 1024 },
];

export const gateFor = (metric: string): Gate | undefined =>
	gates.find((gate) => metric.startsWith(gate.prefix));
