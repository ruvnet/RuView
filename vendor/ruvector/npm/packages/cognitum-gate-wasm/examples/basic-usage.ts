/**
 * Basic Coherence Gate Usage - TypeScript Example
 *
 * This example demonstrates:
 * - Initializing the gate
 * - Requesting action permission
 * - Handling decisions
 *
 * Run with: npx ts-node examples/basic-usage.ts
 */

import { CognitumGate, GateDecision, ActionContext } from '@cognitum/gate';

async function main() {
  console.log('=== Cognitum Gate - Basic Usage ===\n');

  // Initialize the gate
  const gate = await CognitumGate.init({
    thresholds: {
      minCut: 10.0,
      maxShift: 0.5,
      eDeny: 0.01,
      ePermit: 100.0,
    },
    storage: 'memory', // Use 'indexeddb' for persistence
  });

  console.log('Gate initialized\n');

  // Define an action
  const action: ActionContext = {
    actionId: 'deploy-v2.1.0',
    actionType: 'deployment',
    agentId: 'ci-agent',
    target: 'production-cluster',
    metadata: {
      version: '2.1.0',
      changedFiles: 42,
    },
  };

  console.log('Requesting permission for:', action.actionId);

  // Request permission
  const result = await gate.permitAction(action);

  // Handle the decision
  switch (result.decision) {
    case GateDecision.Permit:
      console.log('\n✅ PERMITTED');
      console.log('Token:', result.token.slice(0, 50) + '...');
      console.log('Valid until:', new Date(result.validUntilNs / 1_000_000).toISOString());

      // Agent can now proceed with the action
      await performDeployment(action, result.token);
      break;

    case GateDecision.Defer:
      console.log('\n⏸️  DEFERRED - Human review required');
      console.log('Reason:', result.reason);
      console.log('Escalation URL:', result.escalation?.contextUrl);

      // Wait for human decision or timeout
      const humanDecision = await waitForHumanDecision(result.receiptSequence);
      if (humanDecision.approved) {
        await performDeployment(action, humanDecision.token);
      }
      break;

    case GateDecision.Deny:
      console.log('\n❌ DENIED');
      console.log('Reason:', result.reason);
      console.log('Witness:', result.witness);

      // Log the denial for review
      await logDeniedAction(action, result);
      break;
  }

  // Audit: Get the receipt
  const receipt = await gate.getReceipt(result.receiptSequence);
  console.log('\nReceipt hash:', receipt.hash.slice(0, 16) + '...');

  console.log('\n=== Example Complete ===');
}

async function performDeployment(action: ActionContext, token: string) {
  console.log(`\nDeploying ${action.metadata?.version} to ${action.target}...`);
  console.log('(Deployment would happen here with token validation)');
}

async function waitForHumanDecision(sequence: number) {
  console.log(`\nWaiting for human decision on sequence ${sequence}...`);
  // In production, this would poll an API or use WebSocket
  return { approved: true, token: 'human-approved-token' };
}

async function logDeniedAction(action: ActionContext, result: any) {
  console.log(`\nLogging denied action: ${action.actionId}`);
  // In production, send to logging/alerting system
}

main().catch(console.error);
