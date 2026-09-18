#!/usr/bin/env -S deno run -A
/**
 * Pre-release checks to ensure everything is in order before creating a new release.
 */
import { parse as parseToml } from "jsr:@std/toml";
import * as core from "npm:@actions/core";

interface CargoToml {
  package: { version: string };
}

function getEnvOrFail(name: string): string {
  const value = Deno.env.get(name);
  if (!value) {
    core.setFailed(`Missing required environment variable: ${name}`);
    Deno.exit(1);
  }
  return value;
}

function isTagRef(): boolean {
  return getEnvOrFail("GITHUB_REF_TYPE") === "tag";
}

function getGitTagVersion(): string {
  let tag = getEnvOrFail("GITHUB_REF_NAME");
  tag = tag.startsWith("v") ? tag.slice(1) : tag;
  core.setOutput("git-tag-version", tag);
  return tag;
}

function getCargoTomlVersion(): string {
  try {
    const content = Deno.readTextFileSync("Cargo.toml");
    const toml = parseToml(content) as unknown as CargoToml;

    if (!toml.package?.version) {
      core.setFailed("Cargo.toml missing package.version");
      Deno.exit(1);
    }

    const version = toml.package.version;
    core.setOutput("cargo-toml-version", version);
    return version;
  } catch (err) {
    core.setFailed(`Failed to read Cargo.toml: ${err}`);
    Deno.exit(1);
  }
}

function run(): void {
  if (!isTagRef()) {
    core.info("Skipping: not a tag ref");
    core.setOutput("skip", "true");
    Deno.exit(0);
  }

  const gitTagVersion = getGitTagVersion();
  const cargoTomlVersion = getCargoTomlVersion();

  if (gitTagVersion !== cargoTomlVersion) {
    core.setFailed(
      `Version mismatch: tag=${gitTagVersion}, Cargo.toml=${cargoTomlVersion}`,
    );
    Deno.exit(1);
  }

  core.info(`Version validated: ${gitTagVersion}`);
  core.setOutput("skip", "false");
}

if (import.meta.main) run();
