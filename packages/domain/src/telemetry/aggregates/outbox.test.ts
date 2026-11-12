import { describe, expect, it } from "bun:test";
import { Outbox } from "#telemetry/aggregates/outbox.ts";
import { Position } from "#telemetry/value-objects/position.ts";

const point = (atMs: number) =>
	new Position({ atMs, latitude: 47.9, longitude: -1.6, altitude: null });

const fill = (times: ReadonlyArray<number>) =>
	times.map(point).reduce(Outbox.enqueue, Outbox.empty<Position>());

describe("Outbox", () => {
	it("keeps every point while offline", () => {
		const outbox = fill(Array.from({ length: 3 * 3600 }, (_, s) => s * 1000));
		expect(outbox.pending).toHaveLength(10_800);
	});

	it("drops only what the server acknowledged", () => {
		const outbox = fill([1, 2, 3]);
		const lastSent = Outbox.batch(outbox, 2).at(-1)?.seq ?? -1;
		const after = Outbox.acknowledge(outbox, lastSent);
		expect(after.pending.map((entry) => entry.item.atMs)).toEqual([3]);
	});

	it("resends the same items when an acknowledgement is lost", () => {
		const outbox = fill([1, 2]);
		expect(Outbox.batch(outbox, 10)).toEqual(Outbox.batch(outbox, 10));
	});
});
