import { describe, expect, it } from "bun:test";
import { sumSymbolSizesByPrefix } from "#symbol-sizes.ts";

const prefixes = {
	"crate.bonding_protocol": "bonding_protocol::",
	"crate.bitrate_control": "bitrate_control::",
};

describe("sumSymbolSizesByPrefix", () => {
	it("uses the sizes nm reports (Linux, ELF)", () => {
		const nm = [
			"0000000000001000 0000000000000040 T bonding_protocol::srtla::classify::classify",
			"0000000000001040 0000000000000010 t bonding_protocol::srtla::read::raw_type",
			"0000000000001100 0000000000000100 T core::fmt::write",
			"0000000000001200 0000000000000008 T <bitrate_control::Bounds as core::fmt::Debug>::fmt",
		].join("\n");
		expect(sumSymbolSizesByPrefix(nm, prefixes)).toEqual({
			"crate.bonding_protocol": 0x50,
			"crate.bitrate_control": 0x8,
		});
	});

	it("derives sizes from address gaps when nm reports zero (macOS, Mach-O)", () => {
		const nm = [
			"0000000100001000 0000000000000000 t _bonding_protocol::srtla::classify::classify",
			"0000000100001030 0000000000000000 t _core::fmt::write",
			"0000000100001100 0000000000000000 t _bitrate_control::Bounds::clamp",
			"0000000100001120 0000000000000000 t _main",
		].join("\n");
		expect(sumSymbolSizesByPrefix(nm, prefixes)).toEqual({
			"crate.bonding_protocol": 0x30,
			"crate.bitrate_control": 0x20,
		});
	});
});
