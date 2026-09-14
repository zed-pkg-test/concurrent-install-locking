import { existsSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";
import {
  try_acquire_local_file_lock,
} from "../../../../upstream/ores-locks-and-leases/src/ts/dist/index.js";

const [mode, path, owner, releaseSentinel] = process.argv.slice(2);
if (!mode || !path || !owner) {
  throw new Error("usage: helper <hold|contend|acquire> <path> <owner> [release-sentinel]");
}

if (mode === "hold") {
  if (!releaseSentinel) throw new Error("hold requires release sentinel");
  const lock = await try_acquire_local_file_lock(path, owner);
  if (!lock) throw new Error("holder unexpectedly contended");
  console.log("HELD");
  const deadline = Date.now() + 20_000;
  while (!existsSync(releaseSentinel)) {
    if (Date.now() >= deadline) throw new Error("timed out waiting for release sentinel");
    await sleep(5);
  }
  await lock.release();
  console.log("RELEASED");
} else if (mode === "contend") {
  const lock = await try_acquire_local_file_lock(path, owner);
  if (lock) {
    await lock.release();
    throw new Error("contender unexpectedly acquired live cross-runtime lock");
  }
  console.log("CONTENDED");
} else if (mode === "acquire") {
  const lock = await try_acquire_local_file_lock(path, owner);
  if (!lock) throw new Error("post-release acquire unexpectedly contended");
  console.log("ACQUIRED");
  await lock.release();
} else {
  throw new Error(`unknown mode ${JSON.stringify(mode)}`);
}
