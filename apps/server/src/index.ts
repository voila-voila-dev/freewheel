import { BunRuntime } from "@effect/platform-bun";
import { Effect } from "effect";

const main = Effect.logInfo("freewheel server started").pipe(
	Effect.andThen(Effect.never),
);

BunRuntime.runMain(main);
