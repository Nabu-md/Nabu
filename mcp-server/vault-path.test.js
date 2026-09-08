import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { requireVaultPaths } from "./vault-path.js";

test("registry vault paths expand a leading tilde against the configured home", async () => {
	const rootDir = await mkdtemp(
		path.join(os.tmpdir(), "tolaria-mcp-tilde-paths-"),
	);
	const configDir = path.join(rootDir, "config");
	const homeDir = path.join(rootDir, "home");
	const absoluteVault = path.join(rootDir, "Absolute Vault");
	const configPath = path.join(configDir, "com.nabu.app", "vaults.json");

	await mkdir(path.dirname(configPath), { recursive: true });
	await writeFile(
		configPath,
		JSON.stringify({
			active_vault: "~/Primary Vault",
			vaults: [
				{
					label: "Windows separator",
					path: "~\\Secondary Vault",
					mounted: true,
				},
				{ label: "Home", path: "~", mounted: true },
				{ label: "Absolute", path: absoluteVault, mounted: true },
			],
		}),
		"utf-8",
	);

	try {
		assert.deepEqual(requireVaultPaths({}, { configDir, homeDir }), [
			path.join(homeDir, "Primary Vault"),
			path.join(homeDir, "Secondary Vault"),
			homeDir,
			absoluteVault,
		]);
	} finally {
		await rm(rootDir, { recursive: true, force: true });
	}
});
