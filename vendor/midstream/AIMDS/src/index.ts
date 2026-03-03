import { AIMDSGateway } from './gateway/server';
import { logger } from './monitoring/telemetry';
import { GatewayConfig, AgentDBConfig, LeanAgenticConfig } from './types';

const PORT = parseInt(process.env.PORT || '3000', 10);
const HOST = process.env.HOST || '0.0.0.0';

// Default configuration
const gatewayConfig: GatewayConfig = {
  port: PORT,
  host: HOST,
  enableCors: true,
  enableCompression: true,
  rateLimit: {
    windowMs: 60000, // 1 minute
    max: 100 // 100 requests per minute
  },
  timeouts: {
    request: 30000, // 30 seconds
    shutdown: 10000 // 10 seconds
  }
};

const agentdbConfig: AgentDBConfig = {
  path: process.env.AGENTDB_PATH || './data/agentdb',
  embeddingDim: 384,
  hnswConfig: {
    m: 16,
    efConstruction: 200,
    efSearch: 100
  },
  quicSync: {
    enabled: false,
    port: 4433,
    peers: []
  },
  memory: {
    maxEntries: 1000000,
    ttl: 86400000 // 24 hours
  }
};

const leanAgenticConfig: LeanAgenticConfig = {
  enableHashCons: true,
  enableDependentTypes: true,
  enableTheoremProving: true,
  cacheSize: 10000,
  proofTimeout: 5000 // 5 seconds
};

async function main() {
  try {
    logger.info('Starting AIMDS Gateway...');

    // Create gateway instance
    const gateway = new AIMDSGateway(
      gatewayConfig,
      agentdbConfig,
      leanAgenticConfig
    );

    // Initialize all components
    await gateway.initialize();

    // Start the server
    await gateway.start();

    logger.info(`AIMDS Gateway listening on ${HOST}:${PORT}`);

    // Graceful shutdown handlers
    const shutdown = async (signal: string) => {
      logger.info(`Received ${signal}, shutting down gracefully...`);
      await gateway.shutdown();
      process.exit(0);
    };

    process.on('SIGTERM', () => shutdown('SIGTERM'));
    process.on('SIGINT', () => shutdown('SIGINT'));

  } catch (error) {
    logger.error('Failed to start gateway', { error });
    process.exit(1);
  }
}

main();
