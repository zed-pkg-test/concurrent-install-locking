import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import readline from "node:readline";

const helpers = {
  rust: [process.env.RUST_HELPER],
  go: [process.env.GO_HELPER],
  ts: [process.execPath, process.env.TS_HELPER],
};
for (const [runtime, command] of Object.entries(helpers)) {
  if (command.some((part) => !part)) throw new Error(`missing ${runtime} helper command`);
}

function commandFor(runtime, args) {
  const [bin, ...prefix] = helpers[runtime];
  return { bin, args: [...prefix, ...args] };
}

async function waitForLine(child, expected) {
  const rl = readline.createInterface({ input: child.stdout });
  const timeout = setTimeout(() => child.kill(), 15_000);
  try {
    for await (const line of rl) {
      if (line.trim() === expected) return;
    }
    throw new Error(`child exited before emitting ${expected}`);
  } finally {
    clearTimeout(timeout);
    rl.close();
  }
}

function runOne(runtime, args, expected) {
  const command = commandFor(runtime, args);
  const result = spawnSync(command.bin, command.args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${runtime} failed (${result.status}): ${result.stderr || result.stdout}`);
  }
  assert.match(result.stdout, new RegExp(`(^|\\n)${expected}(\\n|$)`));
}

const root = await mkdtemp(join(tmpdir(), "ores-cross-runtime-lock-"));
try {
  const runtimes = Object.keys(helpers);
  for (let round = 0; round < runtimes.length; round += 1) {
    const holderRuntime = runtimes[round];
    const contenderRuntimes = runtimes.filter((runtime) => runtime !== holderRuntime);
    const path = join(root, `round-${round}.lock`);
    const releaseSentinel = join(root, `round-${round}.release`);
    const holderCommand = commandFor(holderRuntime, ["hold", path, `${holderRuntime}-holder`, releaseSentinel]);
    const holder = spawn(holderCommand.bin, holderCommand.args, {
      stdio: ["ignore", "pipe", "pipe"],
      encoding: "utf8",
    });
    const stderr = [];
    holder.stderr.on("data", (chunk) => stderr.push(String(chunk)));
    await waitForLine(holder, "HELD");

    for (const contenderRuntime of contenderRuntimes) {
      runOne(contenderRuntime, ["contend", path, `${contenderRuntime}-contender`], "CONTENDED");
    }

    await writeFile(releaseSentinel, "release\n", "utf8");
    const holderStatus = await new Promise((resolve) => holder.on("close", resolve));
    assert.equal(holderStatus, 0, `${holderRuntime} holder failed: ${stderr.join("")}`);

    const postReleaseRuntime = contenderRuntimes[0];
    runOne(postReleaseRuntime, ["acquire", path, `${postReleaseRuntime}-after-release`], "ACQUIRED");
  }
  console.log("cross-runtime rotation passed: Rust, Go, and TypeScript each held while the other runtimes contended");
} finally {
  await rm(root, { recursive: true, force: true });
}
