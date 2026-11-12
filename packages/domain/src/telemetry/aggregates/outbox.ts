/**
 * Store-and-forward outbox for telemetry: nothing is lost while offline.
 * Pure model: persistence (SQLite) and transport live in adapters.
 */

export interface OutboxEntry<A> {
	readonly seq: number;
	readonly item: A;
}

export interface Outbox<A> {
	readonly nextSeq: number;
	readonly pending: ReadonlyArray<OutboxEntry<A>>;
}

export const Outbox = {
	empty: <A>(): Outbox<A> => ({ nextSeq: 0, pending: [] }),

	enqueue: <A>(outbox: Outbox<A>, item: A): Outbox<A> => ({
		nextSeq: outbox.nextSeq + 1,
		pending: [...outbox.pending, { seq: outbox.nextSeq, item }],
	}),

	/** The oldest items to send, at most `size` of them. */
	batch: <A>(outbox: Outbox<A>, size: number): ReadonlyArray<OutboxEntry<A>> =>
		outbox.pending.slice(0, size),

	/** The server acknowledges every item up to and including `seq`. */
	acknowledge: <A>(outbox: Outbox<A>, seq: number): Outbox<A> => ({
		nextSeq: outbox.nextSeq,
		pending: outbox.pending.filter((entry) => entry.seq > seq),
	}),
};
