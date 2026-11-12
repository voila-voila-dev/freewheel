import { Schema } from "effect";

/** A GPS fix as the box records it. */
export class Position extends Schema.Class<Position>("Position")({
	/** Unix time in milliseconds. */
	atMs: Schema.Number,
	latitude: Schema.Number,
	longitude: Schema.Number,
	/** Metres above sea level; null when the fix has no altitude. */
	altitude: Schema.NullOr(Schema.Number),
}) {}
