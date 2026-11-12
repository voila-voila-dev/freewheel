/**
 * Public cellular traces we replay. They are downloaded at test time and never committed:
 * Mahimahi's traces ship under its GPL-3.0 licence, so we only read them and derive
 * synthetic traces from their statistics.
 */
export interface TraceSource {
	/** Folder under `.cache/traces`. */
	readonly collection: string;
	readonly baseUrl: string;
	readonly files: ReadonlyArray<string>;
	/** Where the data comes from, for the documentation and benchmark reports. */
	readonly citation: string;
}

export const mahimahi: TraceSource = {
	collection: "mahimahi",
	baseUrl: "https://raw.githubusercontent.com/ravinet/mahimahi/master/traces",
	files: [
		"ATT-LTE-driving-2016.up",
		"ATT-LTE-driving-2016.down",
		"ATT-LTE-driving.up",
		"ATT-LTE-driving.down",
		"TMobile-LTE-driving.up",
		"TMobile-LTE-driving.down",
		"TMobile-LTE-short.up",
		"TMobile-LTE-short.down",
		"Verizon-LTE-driving.up",
		"Verizon-LTE-driving.down",
		"Verizon-LTE-short.up",
		"Verizon-LTE-short.down",
	],
	citation:
		'K. Winstein, A. Sivaraman, H. Balakrishnan, "Stochastic Forecasts Achieve High Throughput and Low Delay over Cellular Networks", USENIX NSDI 2013',
};

export const sources: ReadonlyArray<TraceSource> = [mahimahi];
