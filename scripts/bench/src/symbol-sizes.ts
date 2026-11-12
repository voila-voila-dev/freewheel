/** One code symbol from `nm --print-size --demangle`. */
interface CodeSymbol {
	readonly address: number;
	readonly size: number;
	readonly name: string;
}

const parseSymbols = (nmOutput: string): ReadonlyArray<CodeSymbol> =>
	nmOutput
		.split("\n")
		.map((line) => /^([0-9a-f]+) ([0-9a-f]+) [tT] (.+)$/i.exec(line.trim()))
		.filter((match) => match !== null)
		.map(([, address = "0", size = "0", name = ""]) => ({
			address: Number.parseInt(address, 16),
			size: Number.parseInt(size, 16),
			name: name.replace(/^_+/, "").replace(/^</, ""),
		}))
		.sort((a, b) => a.address - b.address);

/**
 * Mach-O (macOS) reports every size as zero: there, a symbol's size is the gap to the
 * next code symbol. ELF (Linux) reports real sizes, which are kept.
 */
const withSizes = (
	symbols: ReadonlyArray<CodeSymbol>,
): ReadonlyArray<CodeSymbol> =>
	symbols.map((symbol, index) => {
		if (symbol.size > 0) return symbol;
		const next = symbols[index + 1];
		return { ...symbol, size: next ? next.address - symbol.address : 0 };
	});

/**
 * Sums code size per tracked prefix, e.g. `bonding_protocol::` for a crate or
 * `benchmarks::benchmarks::srtla_classify::` for one hot path with everything inlined into it.
 */
export const sumSymbolSizesByPrefix = (
	nmOutput: string,
	prefixes: Readonly<Record<string, string>>,
): Record<string, number> => {
	const totals: Record<string, number> = Object.fromEntries(
		Object.keys(prefixes).map((label) => [label, 0]),
	);
	for (const symbol of withSizes(parseSymbols(nmOutput))) {
		for (const [label, prefix] of Object.entries(prefixes)) {
			if (symbol.name.startsWith(prefix)) {
				totals[label] = (totals[label] ?? 0) + symbol.size;
			}
		}
	}
	return totals;
};
