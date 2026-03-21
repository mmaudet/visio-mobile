import { execSync } from "node:child_process";
import { networkInterfaces } from "node:os";

const CONTAINER_NAME = "livekit-e2e";
const IMAGE = "livekit/livekit-server";
const HEALTH_URL = "http://localhost:7880";

/**
 * Start a LiveKit dev server in a Docker container.
 * Stops any previously running e2e container first.
 */
export function startServer(): void {
  // Clean up any stale container first
  try {
    execSync(`docker rm -f ${CONTAINER_NAME}`, { stdio: "pipe", timeout: 10_000 });
  } catch {
    // No existing container — fine.
  }

  execSync(
    [
      "docker", "run", "-d", "--rm",
      "--name", CONTAINER_NAME,
      "-p", "7880:7880",
      "-p", "7881:7881",
      "-p", "7882:7882/udp",
      IMAGE,
      "--dev", "--bind", "0.0.0.0",
    ].join(" "),
    { stdio: "pipe", timeout: 30_000 },
  );
}

/**
 * Stop the LiveKit e2e Docker container.
 */
export function stopServer(): void {
  try {
    execSync(`docker stop ${CONTAINER_NAME}`, { stdio: "pipe", timeout: 15_000 });
  } catch {
    // Container may already be stopped — ignore.
  }
}

/**
 * Check whether the LiveKit e2e container is currently running.
 */
export function isRunning(): boolean {
  try {
    execSync(`docker inspect ${CONTAINER_NAME}`, { stdio: "pipe" });
    return true;
  } catch {
    return false;
  }
}

/**
 * Wait until the LiveKit server responds to HTTP requests.
 */
export async function waitForHealthy(timeoutMs: number = 10_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  const interval = 500;

  while (Date.now() < deadline) {
    try {
      const resp = await fetch(HEALTH_URL);
      if (resp.ok || resp.status < 500) {
        return;
      }
    } catch {
      // Not ready yet.
    }
    await new Promise((resolve) => setTimeout(resolve, interval));
  }

  throw new Error(`LiveKit server did not become healthy within ${timeoutMs}ms`);
}

/**
 * Return a local (non-loopback) IPv4 address that an Android device on the
 * same network can use to reach this host.
 */
export function getLocalIp(): string {
  const ifaces = networkInterfaces();
  for (const name of Object.keys(ifaces)) {
    const addrs = ifaces[name];
    if (!addrs) continue;
    for (const addr of addrs) {
      if (addr.family === "IPv4" && !addr.internal) {
        return addr.address;
      }
    }
  }
  throw new Error("Could not determine local IP address");
}
