/** Reads the total instruction count from Cachegrind's summary (`I refs: 1,234,567`). */
export const parseInstructionCount = (cachegrindStderr: string): number => {
	const match = /I\s+refs:\s+([\d,]+)/.exec(cachegrindStderr);
	if (!match?.[1]) {
		throw new Error("Cachegrind output has no `I refs` line");
	}
	return Number.parseInt(match[1].replaceAll(",", ""), 10);
};
