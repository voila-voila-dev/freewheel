import { Schema } from "effect";
import { Iccid } from "#connectivity/value-objects/iccid.ts";

/**
 * What the user knows about a SIM's plan. Operators do not expose it, so the user enters it:
 * the cap, when it renews, the price beyond the cap, and when to be warned.
 */
export class DataPlan extends Schema.Class<DataPlan>("DataPlan")({
	iccid: Iccid,
	/** Display name, e.g. "Orange Maroc 25 Go". */
	label: Schema.String,
	/** Included data in bytes; null for unlimited. */
	capBytes: Schema.NullOr(Schema.Number),
	/** Day of the month the counter resets (1–28); null for a one-off pack that never renews. */
	renewalDay: Schema.NullOr(Schema.Number),
	/** Alert thresholds as shares of the cap, e.g. [0.5, 0.8, 1]. */
	alertAt: Schema.Array(Schema.Number),
	/** Price per gigabyte beyond the cap, in cents; null when data stops at the cap. */
	overageCentsPerGb: Schema.NullOr(Schema.Number),
}) {}
