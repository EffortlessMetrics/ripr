export interface LifecycleController {
  start(): Promise<void>;
  stop(): Promise<void>;
}

export type LifecycleStartController = Pick<LifecycleController, 'start'>;

export type LifecycleWait = (
  operation: Promise<void>,
  budgetMs: number,
  description: string
) => Promise<void>;

export const DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS = 30_000;

type DesiredLifecycleState = 'running' | 'stopped';
type LifecycleOperationKind = 'start' | 'restart' | 'stop';

interface LifecycleOperation {
  kind: LifecycleOperationKind;
  generation: number;
  promise: Promise<void>;
}

export async function waitForLifecyclePromise(
  operation: Promise<void>,
  budgetMs: number,
  description: string
): Promise<void> {
  let timeoutHandle: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => {
    timeoutHandle = setTimeout(() => {
      reject(
        new Error(
          `${description} did not settle within ${budgetMs}ms; refusing an unsafe ripr lifecycle transition.`
        )
      );
    }, budgetMs);
  });

  try {
    await Promise.race([operation, timeout]);
  } finally {
    if (timeoutHandle !== undefined) {
      clearTimeout(timeoutHandle);
    }
  }
}

/**
 * Serializes extension-owned lifecycle intent without treating a snapshot of
 * another promise as authority. Intent changes are recorded synchronously;
 * every awaited operation re-checks its generation before it can stop or
 * replace a client. A terminal stop therefore dominates an earlier or
 * concurrent restart. (#2826)
 */
export class ExtensionLifecycleCoordinator {
  private desiredState: DesiredLifecycleState = 'stopped';
  private terminalShutdown = false;
  private operationGeneration = 0;
  private currentOperation: LifecycleOperation | undefined;
  private inFlightStart: Promise<void> | undefined;
  private terminalStopPromise: Promise<void> | undefined;
  // Tracks ownership of a successfully started session so terminal stop does
  // not stop a session twice when it supersedes a restart already in stop().
  private sessionRunning = false;

  constructor(private readonly waitFor: LifecycleWait = waitForLifecyclePromise) {}

  start(
    currentController: LifecycleStartController | undefined,
    settleBudgetMs = DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS
  ): Promise<void> {
    if (!currentController) {
      return Promise.resolve();
    }
    if (this.terminalShutdown) {
      return this.rejectTerminalIntent('start');
    }

    const current = this.currentOperation;
    if (
      this.desiredState === 'running' &&
      current &&
      (current.kind === 'start' || current.kind === 'restart')
    ) {
      return current.promise;
    }
    if (this.sessionRunning) {
      this.desiredState = 'running';
      return Promise.resolve();
    }

    this.desiredState = 'running';
    const generation = this.nextGeneration();
    return this.beginOperation('start', generation, async (previous) => {
      await this.waitForPriorOperation(previous, settleBudgetMs);
      await this.waitForInFlightStart(settleBudgetMs);
      if (!this.isCurrentRunningGeneration(generation) || this.sessionRunning) {
        return;
      }
      await this.startController(currentController);
    });
  }

  restart(
    currentController: LifecycleController | undefined,
    settleBudgetMs = DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS
  ): Promise<void> {
    if (!currentController) {
      return Promise.resolve();
    }
    if (this.terminalShutdown) {
      return this.rejectTerminalIntent('restart');
    }

    const current = this.currentOperation;
    if (this.desiredState === 'running' && current?.kind === 'restart') {
      return current.promise;
    }

    this.desiredState = 'running';
    const generation = this.nextGeneration();
    return this.beginOperation('restart', generation, async (previous) => {
      await this.waitForPriorOperation(previous, settleBudgetMs);
      await this.waitForInFlightStart(settleBudgetMs);
      if (!this.isCurrentRunningGeneration(generation)) {
        return;
      }

      await this.stopController(currentController);
      if (!this.isCurrentRunningGeneration(generation)) {
        return;
      }

      // There is no await between the final generation check and start(), so a
      // later stop cannot interleave after the check but before replacement
      // startup begins.
      await this.startController(currentController);
    });
  }

  stop(
    currentController: LifecycleController | undefined,
    settleBudgetMs = DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS
  ): Promise<void> {
    if (!currentController) {
      return Promise.resolve();
    }
    if (this.terminalStopPromise) {
      return this.terminalStopPromise;
    }

    // Record terminal shutdown before awaiting anything. Any concurrent or
    // later restart sees this state synchronously and cannot enqueue a
    // replacement operation.
    this.desiredState = 'stopped';
    this.terminalShutdown = true;
    const generation = this.nextGeneration();
    const stop = this.beginOperation('stop', generation, async (previous) => {
      await this.waitForPriorOperation(previous, settleBudgetMs);
      await this.waitForInFlightStart(settleBudgetMs);
      if (!this.isCurrentStoppedGeneration(generation)) {
        return;
      }
      await this.stopController(currentController);
    });
    this.terminalStopPromise = stop;
    return stop;
  }

  private nextGeneration(): number {
    this.operationGeneration += 1;
    return this.operationGeneration;
  }

  private beginOperation(
    kind: LifecycleOperationKind,
    generation: number,
    run: (previous: LifecycleOperation | undefined) => Promise<void>
  ): Promise<void> {
    const previous = this.currentOperation;
    const promise = run(previous);
    const operation: LifecycleOperation = { kind, generation, promise };
    this.currentOperation = operation;
    promise.then(
      () => this.clearOperation(operation),
      () => this.clearOperation(operation)
    );
    return promise;
  }

  private clearOperation(operation: LifecycleOperation): void {
    if (this.currentOperation === operation) {
      this.currentOperation = undefined;
    }
  }

  private async waitForPriorOperation(
    previous: LifecycleOperation | undefined,
    settleBudgetMs: number
  ): Promise<void> {
    if (!previous) {
      return;
    }
    await this.waitFor(
      previous.promise.catch(() => undefined),
      settleBudgetMs,
      `ripr server ${previous.kind} operation`
    );
  }

  private async waitForInFlightStart(settleBudgetMs: number): Promise<void> {
    const start = this.inFlightStart;
    if (!start) {
      return;
    }
    await this.waitFor(
      start.catch(() => undefined),
      settleBudgetMs,
      'ripr server startup'
    );
  }

  private async startController(currentController: LifecycleStartController): Promise<void> {
    const start = currentController.start();
    this.inFlightStart = start;
    try {
      await start;
      this.sessionRunning = true;
    } catch (error) {
      this.sessionRunning = false;
      throw error;
    } finally {
      if (this.inFlightStart === start) {
        this.inFlightStart = undefined;
      }
    }
  }

  private async stopController(currentController: LifecycleController): Promise<void> {
    if (!this.sessionRunning) {
      return;
    }
    await currentController.stop();
    this.sessionRunning = false;
  }

  private isCurrentRunningGeneration(generation: number): boolean {
    return (
      !this.terminalShutdown &&
      this.desiredState === 'running' &&
      this.operationGeneration === generation
    );
  }

  private isCurrentStoppedGeneration(generation: number): boolean {
    return (
      this.terminalShutdown &&
      this.desiredState === 'stopped' &&
      this.operationGeneration === generation
    );
  }

  private rejectTerminalIntent(intent: 'start' | 'restart'): Promise<void> {
    return Promise.reject(
      new Error(`ripr server ${intent} was refused because terminal shutdown has begun.`)
    );
  }
}
