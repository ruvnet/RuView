const { Worker, isMainThread, parentPort, workerData } = require('worker_threads');

if (!isMainThread) {
  // Worker thread code
  const { createSolver } = require('../src/solver');

  let currentSolver = null;
  let currentSession = null;
  let solving = false;

  parentPort.on('message', async (message) => {
    try {
      switch (message.type) {
        case 'solve':
          await handleSolve(message);
          break;

        case 'stop':
          await handleStop(message);
          break;

        case 'status':
          handleStatus(message);
          break;

        default:
          parentPort.postMessage({
            type: 'error',
            sessionId: message.sessionId,
            error: `Unknown message type: ${message.type}`
          });
      }
    } catch (error) {
      parentPort.postMessage({
        type: 'error',
        sessionId: message.sessionId,
        error: error.message,
        stack: error.stack
      });
    }
  });

  async function handleSolve(message) {
    if (solving) {
      throw new Error('Worker is already solving a problem');
    }

    solving = true;
    currentSession = message.sessionId;

    try {
      // Create solver with provided configuration
      currentSolver = await createSolver({
        matrix: message.matrix,
        method: message.method || 'adaptive',
        tolerance: message.options?.tolerance || 1e-10,
        maxIterations: message.options?.maxIterations || 1000,
        enableVerification: message.options?.enableVerification || false
      });

      // Start streaming solve
      const startTime = Date.now();
      let lastMemoryCheck = startTime;

      for await (const update of currentSolver.streamSolve(message.vector)) {
        // Check if we should stop
        if (!solving || currentSession !== message.sessionId) {
          break;
        }

        // Memory usage tracking
        const now = Date.now();
        let memoryUsage = 0;

        if (now - lastMemoryCheck > 5000) { // Check every 5 seconds
          const memInfo = process.memoryUsage();
          memoryUsage = Math.round(memInfo.heapUsed / 1024 / 1024); // MB
          lastMemoryCheck = now;
        }

        // Send iteration update
        parentPort.postMessage({
          type: 'iteration',
          sessionId: message.sessionId,
          iteration: update.iteration,
          residual: update.residual,
          convergenceRate: update.convergenceRate,
          memoryUsage,
          verified: update.verified || false,
          timestamp: new Date().toISOString()
        });

        // Check for convergence
        if (update.converged) {
          parentPort.postMessage({
            type: 'converged',
            sessionId: message.sessionId,
            solution: update.solution,
            stats: {
              iterations: update.iteration,
              residual: update.residual,
              solveTime: now - startTime,
              memoryUsage,
              converged: true
            }
          });
          break;
        }
      }

    } catch (error) {
      parentPort.postMessage({
        type: 'error',
        sessionId: message.sessionId,
        error: error.message
      });
    } finally {
      solving = false;
      currentSolver = null;
      currentSession = null;
    }
  }

  async function handleStop(message) {
    if (currentSession === message.sessionId) {
      solving = false;

      if (currentSolver && typeof currentSolver.stop === 'function') {
        await currentSolver.stop();
      }

      parentPort.postMessage({
        type: 'stopped',
        sessionId: message.sessionId
      });
    }
  }

  function handleStatus(message) {
    parentPort.postMessage({
      type: 'status',
      sessionId: message.sessionId,
      solving,
      currentSession,
      memory: process.memoryUsage(),
      uptime: process.uptime()
    });
  }

  // Handle worker errors
  process.on('uncaughtException', (error) => {
    parentPort.postMessage({
      type: 'error',
      sessionId: currentSession,
      error: error.message,
      fatal: true
    });
    process.exit(1);
  });

  process.on('unhandledRejection', (error) => {
    parentPort.postMessage({
      type: 'error',
      sessionId: currentSession,
      error: error.message,
      fatal: true
    });
    process.exit(1);
  });

  // Send ready signal
  parentPort.postMessage({
    type: 'ready',
    workerId: process.pid
  });

} else {
  // Main thread code - export worker creation utility
  const path = require('path');

  function createSolverWorker() {
    return new Worker(__filename);
  }

  module.exports = { createSolverWorker };
}