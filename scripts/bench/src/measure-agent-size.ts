import type { MeasurementResult } from "#metrics.ts";
import { agentBinary } from "#paths.ts";
import { runOrThrow } from "#run-command.ts";

/** Size of the agent executable shipped to the box (ARM64, Bun runtime included). */
export const measureAgentSize = async (): Promise<MeasurementResult> => {
	await runOrThrow(["bun", "run", "--filter", "@freewheel/agent", "build"]);
	return {
		metrics: { "size.agent.bytes": Bun.file(agentBinary).size },
		skipped: [],
	};
};
