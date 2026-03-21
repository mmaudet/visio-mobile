const DEFAULT_INTERVAL = 500;
const DEFAULT_TIMEOUT = parseInt(process.env.E2E_ASSERT_TIMEOUT ?? "5000", 10);

export interface PollOptions {
  /** Polling interval in milliseconds. Default: 500. */
  interval?: number;
  /** Maximum time to wait in milliseconds. Default: E2E_ASSERT_TIMEOUT env or 5000. */
  timeout?: number;
  /** Human-readable description included in the timeout error message. */
  description?: string;
}

/**
 * Poll `fn` at a regular interval until it returns a truthy value.
 *
 * Returns the first truthy result produced by `fn`.
 * Throws if the timeout elapses before a truthy result is obtained.
 */
export async function pollUntil<T>(
  fn: () => T | Promise<T>,
  opts?: PollOptions,
): Promise<T> {
  const interval = opts?.interval ?? DEFAULT_INTERVAL;
  const timeout = opts?.timeout ?? DEFAULT_TIMEOUT;
  const description = opts?.description;

  const deadline = Date.now() + timeout;

  while (true) {
    const result = await fn();
    if (result) return result;

    if (Date.now() >= deadline) {
      const msg = description
        ? `pollUntil timed out after ${timeout}ms: ${description}`
        : `pollUntil timed out after ${timeout}ms`;
      throw new Error(msg);
    }

    await new Promise((resolve) => setTimeout(resolve, interval));
  }
}
