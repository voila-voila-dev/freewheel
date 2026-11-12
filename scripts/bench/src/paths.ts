import { join } from "node:path";

export const repositoryRoot = join(import.meta.dir, "..", "..", "..");
/** Sizes and instruction counts differ per platform, so each has its own baseline. */
export const platform = `${process.platform}-${process.arch}`;
export const baselineFile = join(
	repositoryRoot,
	"bench",
	`baseline.${platform}.json`,
);
export const resultsFile = join(repositoryRoot, "bench", "results.json");
export const hotPathsBinary = join(
	repositoryRoot,
	"target",
	"size-report",
	"hot-paths",
);
export const agentBinary = join(
	repositoryRoot,
	"apps",
	"agent",
	"dist",
	"freewheel-agent",
);

/** Crates whose code runs on the box per packet: their size is tracked one by one. */
export const trackedCrates = ["bonding_protocol", "bitrate_control"] as const;
