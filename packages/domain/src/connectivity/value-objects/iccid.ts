import { Schema } from "effect";

/** The SIM card's identity (ICCID): usage history follows the card, not the modem slot. */
export const Iccid = Schema.String.pipe(Schema.brand("Iccid"));
export type Iccid = typeof Iccid.Type;
