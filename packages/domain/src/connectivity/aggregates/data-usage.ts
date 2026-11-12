import type { DataPlan } from "#connectivity/value-objects/data-plan.ts";

/** Bytes counted on one SIM at one moment. The full history is kept. */
export interface UsageSample {
	readonly at: Date;
	readonly upBytes: number;
	readonly downBytes: number;
}

export interface PeriodUsage {
	readonly periodStart: Date;
	readonly usedBytes: number;
	/** Share of the cap used; null for unlimited plans. */
	readonly share: number | null;
	readonly remainingBytes: number | null;
	/** Thresholds crossed in this period, ascending. */
	readonly alerts: ReadonlyArray<number>;
	readonly overageCents: number;
}

const GB = 1_000_000_000;

/** Start of the billing period containing `at` (UTC), for a plan renewing on `renewalDay`. */
export const periodStart = (at: Date, renewalDay: number): Date => {
	const year = at.getUTCFullYear();
	const month = at.getUTCMonth();
	const thisMonth = new Date(Date.UTC(year, month, renewalDay));
	return at >= thisMonth
		? thisMonth
		: new Date(Date.UTC(year, month - 1, renewalDay));
};

/** Usage of the current period, from the full history of samples. */
export const usageAt = (
	plan: DataPlan,
	history: ReadonlyArray<UsageSample>,
	at: Date,
	activatedAt: Date,
): PeriodUsage => {
	const start =
		plan.renewalDay === null ? activatedAt : periodStart(at, plan.renewalDay);
	const usedBytes = history
		.filter((sample) => sample.at >= start && sample.at <= at)
		.reduce((sum, sample) => sum + sample.upBytes + sample.downBytes, 0);
	if (plan.capBytes === null) {
		return {
			periodStart: start,
			usedBytes,
			share: null,
			remainingBytes: null,
			alerts: [],
			overageCents: 0,
		};
	}
	const share = usedBytes / plan.capBytes;
	const overBytes = Math.max(0, usedBytes - plan.capBytes);
	return {
		periodStart: start,
		usedBytes,
		share,
		remainingBytes: Math.max(0, plan.capBytes - usedBytes),
		alerts: [...plan.alertAt].sort((a, b) => a - b).filter((t) => share >= t),
		overageCents: Math.ceil((overBytes / GB) * (plan.overageCentsPerGb ?? 0)),
	};
};
