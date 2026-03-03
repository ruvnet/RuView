/**
 * Basic Usage Example for AIMDS Gateway
 */

import { AIMDSGateway } from '../src/gateway/server';
import { Config } from '../src/utils/config';
import { AIMDSRequest } from '../src/types';

async function main() {
  // Create configuration
  const config = Config.getInstance();

  // Initialize gateway
  const gateway = new AIMDSGateway(
    config.getGatewayConfig(),
    config.getAgentDBConfig(),
    config.getLeanAgenticConfig()
  );

  await gateway.initialize();
  await gateway.start();

  console.log('AIMDS Gateway started on port 3000');

  // Example: Process a request programmatically
  const testRequest: AIMDSRequest = {
    id: 'example-1',
    timestamp: Date.now(),
    source: {
      ip: '192.168.1.100',
      userAgent: 'Mozilla/5.0',
      headers: {
        'content-type': 'application/json'
      }
    },
    action: {
      type: 'read',
      resource: '/api/users/profile',
      method: 'GET'
    },
    context: {
      userId: 'user123',
      sessionId: 'session456'
    }
  };

  const result = await gateway.processRequest(testRequest);

  console.log('Defense Result:', {
    allowed: result.allowed,
    confidence: result.confidence,
    threatLevel: result.threatLevel,
    latency: `${result.latencyMs}ms`,
    path: result.metadata.pathTaken
  });

  // Example: Suspicious request
  const suspiciousRequest: AIMDSRequest = {
    id: 'example-2',
    timestamp: Date.now(),
    source: {
      ip: '10.0.0.1',
      userAgent: 'sqlmap/1.0',
      headers: {}
    },
    action: {
      type: 'admin',
      resource: '/api/admin/delete-all',
      method: 'DELETE',
      payload: {
        confirm: true,
        force: true
      }
    }
  };

  const suspiciousResult = await gateway.processRequest(suspiciousRequest);

  console.log('Suspicious Request Result:', {
    allowed: suspiciousResult.allowed,
    confidence: suspiciousResult.confidence,
    threatLevel: suspiciousResult.threatLevel,
    latency: `${suspiciousResult.latencyMs}ms`,
    matches: suspiciousResult.matches.length,
    proof: suspiciousResult.verificationProof?.id
  });
}

main().catch(console.error);
