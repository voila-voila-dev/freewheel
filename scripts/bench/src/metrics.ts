/** Measured values, keyed by a stable metric name such as `size.binary.hot-paths.bytes`. */
export type Metrics = Readonly<Record<string, number>>;

/** A measurement that could not run here, and why (e.g. Valgrind missing on macOS). */
export interface Skipped {
	readonly measurement: string;
	readonly reason: string;
}

export interface MeasurementResult {
	readonly metrics: Metrics;
	readonly skipped: ReadonlyArray<Skipped>;
}
