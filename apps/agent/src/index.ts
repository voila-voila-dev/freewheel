import { BunRuntime } from "@effect/platform-bun";
import { Effect } from "effect";

const main = Effect.logInfo("freewheel agent started").pipe(
	Effect.andThen(Effect.never),
);

BunRuntime.runMain(main);
