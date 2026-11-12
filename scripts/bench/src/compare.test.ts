import { describe, expect, it } from "bun:test";
import { compare } from "#compare.ts";

describe("compare", () => {
	it("fails instructions growing more than 2 %", () => {
		const [row] = compare(
			{ "instructions.x.per_packet": 100 },
			{ "instructions.x.per_packet": 103 },
		);
		expect(row?.passed).toBe(false);
	});

	it("passes instructions within 2 %", () => {
		const [row] = compare(
			{ "instructions.x.per_packet": 100 },
			{ "instructions.x.per_packet": 101.5 },
		);
		expect(row?.passed).toBe(true);
	});

	it("tolerates 16 KiB of growth on small binaries", () => {
		const [row] = compare(
			{ "size.binary.x.bytes": 100_000 },
			{ "size.binary.x.bytes": 110_000 },
		);
		expect(row?.passed).toBe(true);
	});

	it("fails a large binary growing more than 2 %", () => {
		const [row] = compare(
			{ "size.binary.x.bytes": 10_000_000 },
			{ "size.binary.x.bytes": 10_300_000 },
		);
		expect(row?.passed).toBe(false);
	});

	it("passes a metric that has no baseline yet", () => {
		const [row] = compare({}, { "size.agent.bytes": 1 });
		expect(row?.passed).toBe(true);
		expect(row?.growth).toBeUndefined();
	});
});
