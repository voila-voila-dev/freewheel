import { describe, expect, it } from "bun:test";
import { Schema } from "effect";
import {
	periodStart,
	type UsageSample,
	usageAt,
} from "#connectivity/aggregates/data-usage.ts";
import { DataPlan } from "#connectivity/value-objects/data-plan.ts";
import { Iccid } from "#connectivity/value-objects/iccid.ts";

const GB = 1_000_000_000;
const iccid = Schema.decodeUnknownSync(Iccid)("8921201234567890123");

const plan = (
	overrides: Partial<ConstructorParameters<typeof DataPlan>[0]> = {},
) =>
	new DataPlan({
		iccid,
		label: "Orange Maroc 25 Go",
		capBytes: 25 * GB,
		renewalDay: 15,
		alertAt: [0.8, 0.5, 1],
		overageCentsPerGb: 500,
		...overrides,
	});

const sample = (iso: string, gigabytes: number): UsageSample => ({
	at: new Date(iso),
	upBytes: gigabytes * GB,
	downBytes: 0,
});

const activatedAt = new Date("2026-01-01T00:00:00Z");

describe("periodStart", () => {
	it("starts on the renewal day of this month once it has passed", () => {
		expect(periodStart(new Date("2026-03-20T10:00:00Z"), 15)).toEqual(
			new Date("2026-03-15T00:00:00Z"),
		);
	});

	it("starts on the renewal day of last month before it", () => {
		expect(periodStart(new Date("2026-03-10T10:00:00Z"), 15)).toEqual(
			new Date("2026-02-15T00:00:00Z"),
		);
	});
});

describe("usageAt", () => {
	const history = [
		sample("2026-03-01T12:00:00Z", 10), // previous period
		sample("2026-03-16T12:00:00Z", 12),
		sample("2026-03-18T12:00:00Z", 9),
	];
	const at = new Date("2026-03-20T00:00:00Z");

	it("counts only the current period and keeps history intact", () => {
		const usage = usageAt(plan(), history, at, activatedAt);
		expect(usage.usedBytes).toBe(21 * GB);
		expect(usage.remainingBytes).toBe(4 * GB);
		expect(history).toHaveLength(3);
	});

	it("raises every alert threshold crossed, in order", () => {
		expect(usageAt(plan(), history, at, activatedAt).alerts).toEqual([
			0.5, 0.8,
		]);
	});

	it("prices data beyond the cap", () => {
		const over = [...history, sample("2026-03-19T12:00:00Z", 6)];
		const usage = usageAt(plan(), over, at, activatedAt);
		expect(usage.alerts).toEqual([0.5, 0.8, 1]);
		expect(usage.overageCents).toBe(1000);
	});

	it("never resets a one-off pack", () => {
		const usage = usageAt(plan({ renewalDay: null }), history, at, activatedAt);
		expect(usage.usedBytes).toBe(31 * GB);
	});

	it("has no share or alerts on an unlimited plan", () => {
		const usage = usageAt(plan({ capBytes: null }), history, at, activatedAt);
		expect(usage.share).toBeNull();
		expect(usage.alerts).toEqual([]);
	});
});
