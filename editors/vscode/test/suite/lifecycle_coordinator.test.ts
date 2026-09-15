import * as assert from 'assert';
import {
  ExtensionLifecycleCoordinator,
  LifecycleController,
  LifecycleWait,
} from '../../src/lifecycleCoordinator';

interface Deferred {
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: Error) => void;
}

function deferred(): Deferred {
  let resolve: (() => void) | undefined;
  let reject: ((error: Error) => void) | undefined;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return {
    promise,
    resolve: () => resolve?.(),
    reject: (error: Error) => reject?.(error),
  };
}

function manualTimeoutWait(timeout: Deferred): LifecycleWait {
  return async (operation, budgetMs, description) => {
    await Promise.race([
      operation,
      timeout.promise.then(() => {
        throw new Error(`${description} did not settle within ${budgetMs}ms; manual timeout.`);
      }),
    ]);
  };
}

suite('Extension Lifecycle Coordinator', () => {
  test('restart waits for in-flight startup before stopping or creating a replacement', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    const firstStart = deferred();
    const firstStartEntered = deferred();
    const replacementStart = deferred();
    const replacementStartEntered = deferred();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        if (startCalls === 1) {
          firstStartEntered.resolve();
          await firstStart.promise;
        } else {
          replacementStartEntered.resolve();
          await replacementStart.promise;
        }
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    const initialStart = coordinator.start(controller);
    await firstStartEntered.promise;
    const restart = coordinator.restart(controller, 1_000);

    assert.strictEqual(stopCalls, 0, 'a Starting client must not receive stop()');
    assert.strictEqual(startCalls, 1, 'no replacement starts while the first start is unresolved');

    firstStart.resolve();
    await replacementStartEntered.promise;
    assert.strictEqual(stopCalls, 1);
    assert.strictEqual(startCalls, 2);

    replacementStart.resolve();
    await Promise.all([initialStart, restart]);
    assert.strictEqual(stopCalls, 1);
    assert.strictEqual(startCalls, 2, 'restart creates exactly one replacement after startup settles');
  });

  test('terminal stop supersedes a restart that begins while stop awaits startup', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    const firstStart = deferred();
    const firstStartEntered = deferred();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        firstStartEntered.resolve();
        await firstStart.promise;
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    const initialStart = coordinator.start(controller);
    await firstStartEntered.promise;

    // Exact #2826 regression: stop observes no restart, then a restart request
    // arrives while stop is waiting for the original startup.
    const stop = coordinator.stop(controller, 1_000);
    const duplicateStop = coordinator.stop(controller, 1_000);
    const restartError = coordinator.restart(controller, 1_000).then(
      () => undefined,
      (error: unknown) => error
    );

    assert.strictEqual(stop, duplicateStop, 'duplicate terminal stops coalesce');
    assert.strictEqual(stopCalls, 0);
    firstStart.resolve();
    await Promise.all([initialStart, stop]);

    assert.match(String(await restartError), /terminal shutdown has begun/);
    assert.strictEqual(stopCalls, 1, 'the original session is stopped exactly once');
    assert.strictEqual(startCalls, 1, 'no replacement starts after terminal stop intent');
  });

  test('concurrent restart requests coalesce into one stop and replacement start', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    const replacementStart = deferred();
    const replacementStartEntered = deferred();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        if (startCalls === 2) {
          replacementStartEntered.resolve();
          await replacementStart.promise;
        }
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    await coordinator.start(controller);
    const firstRestart = coordinator.restart(controller, 1_000);
    const secondRestart = coordinator.restart(controller, 1_000);
    assert.strictEqual(firstRestart, secondRestart, 'duplicate restarts share one operation');

    await replacementStartEntered.promise;
    assert.strictEqual(stopCalls, 1);
    assert.strictEqual(startCalls, 2);

    replacementStart.resolve();
    await Promise.all([firstRestart, secondRestart]);
    assert.strictEqual(stopCalls, 1, 'coalesced restart must not stop twice');
    assert.strictEqual(startCalls, 2, 'coalesced restart must not create two replacements');
  });

  test('terminal stop during restart stop does not stop the same session twice', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    const firstStop = deferred();
    const firstStopEntered = deferred();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
      },
      stop: async () => {
        stopCalls += 1;
        if (stopCalls === 1) {
          firstStopEntered.resolve();
          await firstStop.promise;
        }
      },
    };

    await coordinator.start(controller);
    const restart = coordinator.restart(controller, 1_000);
    await firstStopEntered.promise;
    const stop = coordinator.stop(controller, 1_000);

    firstStop.resolve();
    await Promise.all([restart, stop]);

    assert.strictEqual(startCalls, 1, 'terminal stop suppresses the replacement start');
    assert.strictEqual(stopCalls, 1, 'the already-stopped session is not stopped twice');
  });

  test('stop begun during replacement startup remains final', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    const replacementStart = deferred();
    const replacementStartEntered = deferred();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        if (startCalls === 2) {
          replacementStartEntered.resolve();
          await replacementStart.promise;
        }
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    await coordinator.start(controller);
    const restart = coordinator.restart(controller, 1_000);
    await replacementStartEntered.promise;
    assert.strictEqual(stopCalls, 1, 'restart stops the original session once');

    const stop = coordinator.stop(controller, 1_000);
    replacementStart.resolve();
    await Promise.all([restart, stop]);

    assert.strictEqual(startCalls, 2, 'no third session starts after stop intent');
    assert.strictEqual(stopCalls, 2, 'each started session is stopped at most once');
    await assert.rejects(
      coordinator.restart(controller, 1_000),
      /terminal shutdown has begun/
    );
  });

  test('failed stop retains session ownership and retries before replacement', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
      },
      stop: async () => {
        stopCalls += 1;
        if (stopCalls === 1) {
          throw new Error('sentinel stop failure');
        }
      },
    };

    await coordinator.start(controller);
    await assert.rejects(coordinator.restart(controller, 1_000), /sentinel stop failure/);

    assert.strictEqual(startCalls, 1, 'a failed stop must not create a replacement');
    assert.strictEqual(stopCalls, 1);

    await coordinator.restart(controller, 1_000);
    assert.strictEqual(stopCalls, 2, 'the possibly-running session is stopped again before replacement');
    assert.strictEqual(startCalls, 2, 'replacement starts only after a confirmed stop');
  });

  test('startup rejection permits recovery restart before terminal shutdown', async () => {
    const coordinator = new ExtensionLifecycleCoordinator();
    const firstStart = deferred();
    const firstStartEntered = deferred();
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        if (startCalls === 1) {
          firstStartEntered.resolve();
          await firstStart.promise;
        }
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    const initialStart = coordinator.start(controller);
    await firstStartEntered.promise;
    const observedInitial = initialStart.then(
      () => undefined,
      (error: unknown) => error
    );
    const restart = coordinator.restart(controller, 1_000);
    firstStart.reject(new Error('sentinel startup rejection'));

    assert.match(String(await observedInitial), /sentinel startup rejection/);
    await restart;
    assert.strictEqual(stopCalls, 0, 'rejected startup does not own a running session to stop');
    assert.strictEqual(startCalls, 2, 'recovery starts one fresh session after rejection');
  });

  test('completed startup remains single after a bounded restart timeout', async () => {
    const firstStart = deferred();
    const firstStartEntered = deferred();
    const timeout = deferred();
    const coordinator = new ExtensionLifecycleCoordinator(manualTimeoutWait(timeout));
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        firstStartEntered.resolve();
        await firstStart.promise;
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    const initialStart = coordinator.start(controller);
    await firstStartEntered.promise;
    const restart = coordinator.restart(controller, 30_000);
    timeout.resolve();
    await assert.rejects(restart, /did not settle within 30000ms/);

    firstStart.resolve();
    await initialStart;
    await coordinator.start(controller, 30_000);

    assert.strictEqual(startCalls, 1, 'the surviving completed startup is reused');
    assert.strictEqual(stopCalls, 0);
  });

  test('wedged startup reaches a deterministic finite failure without side effects', async () => {
    const firstStart = deferred();
    const firstStartEntered = deferred();
    const timeout = deferred();
    const coordinator = new ExtensionLifecycleCoordinator(manualTimeoutWait(timeout));
    let startCalls = 0;
    let stopCalls = 0;
    const controller: LifecycleController = {
      start: async () => {
        startCalls += 1;
        firstStartEntered.resolve();
        await firstStart.promise;
      },
      stop: async () => {
        stopCalls += 1;
      },
    };

    const initialStart = coordinator.start(controller);
    await firstStartEntered.promise;
    const restart = coordinator.restart(controller, 30_000);
    timeout.resolve();

    await assert.rejects(restart, /did not settle within 30000ms/);
    assert.strictEqual(stopCalls, 0, 'timeout must not call stop() on a Starting client');
    assert.strictEqual(startCalls, 1, 'timeout must not start a replacement client');

    firstStart.resolve();
    await initialStart;
  });
});
