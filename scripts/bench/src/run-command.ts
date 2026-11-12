import { repositoryRoot } from "#paths.ts";

export interface CommandOutput {
	readonly exitCode: number;
	readonly stdout: string;
	readonly stderr: string;
}

/** Runs a command from the repository root and captures its output. */
export const runCommand = async (
	command: ReadonlyArray<string>,
): Promise<CommandOutput> => {
	const child = Bun.spawn([...command], {
		cwd: repositoryRoot,
		stdout: "pipe",
		stderr: "pipe",
	});
	const [stdout, stderr, exitCode] = await Promise.all([
		new Response(child.stdout).text(),
		new Response(child.stderr).text(),
		child.exited,
	]);
	return { exitCode, stdout, stderr };
};

/** Runs a command and throws with its error output if it fails. */
export const runOrThrow = async (
	command: ReadonlyArray<string>,
): Promise<CommandOutput> => {
	const output = await runCommand(command);
	if (output.exitCode !== 0) {
		throw new Error(`${command.join(" ")} failed:\n${output.stderr}`);
	}
	return output;
};

/** Whether a program is on the PATH. */
export const isInstalled = (program: string): boolean =>
	Bun.which(program) !== null;
