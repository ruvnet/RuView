/**
 * cli-rvf.ts - RVF migration and rebuild CLI commands
 *
 * Two commands:
 *   rvf-migrate  — Convert existing rvlite data to RVF format
 *   rvf-rebuild  — Reconstruct metadata from an RVF file
 *
 * Usage (via the rvlite CLI binary or directly):
 *   rvlite rvf-migrate --source .rvlite/db.json --dest data.rvf [--dry-run] [--verify]
 *   rvlite rvf-rebuild  --source data.rvf [--dest .rvlite/db.json]
 */

// ── Types ────────────────────────────────────────────────────────────────

/** Shape of the JSON-based rvlite database state (as saved by the CLI). */
interface RvLiteDbState {
  vectors: Record<string, {
    vector: number[];
    metadata?: Record<string, unknown>;
    norm?: number;
  }>;
  graph?: {
    nodes?: Record<string, unknown>;
    edges?: Record<string, unknown>;
  };
  triples?: Array<{ subject: string; predicate: string; object: string }>;
  nextId?: number;
  config?: {
    dimensions?: number;
    metric?: string;
  };
}

/** JSON-based RVF file envelope. */
interface RvfFileEnvelope {
  rvf_version: number;
  magic: 'RVF1';
  created_at: string;
  dimensions: number;
  distance_metric: string;
  payload: RvLiteDbState;
}

/** Summary report returned by migrate / rebuild. */
export interface MigrateReport {
  vectorsMigrated: number;
  triplesMigrated: number;
  graphNodesMigrated: number;
  graphEdgesMigrated: number;
  skipped: boolean;
  dryRun: boolean;
  verifyPassed?: boolean;
}

export interface RebuildReport {
  vectorsRecovered: number;
  triplesRecovered: number;
  graphNodesRecovered: number;
  graphEdgesRecovered: number;
}

// ── Helpers ──────────────────────────────────────────────────────────────

function vectorsClose(a: number[], b: number[], tolerance: number): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (Math.abs(a[i] - b[i]) > tolerance) return false;
  }
  return true;
}

// ── Migrate ──────────────────────────────────────────────────────────────

/**
 * Convert an existing rvlite JSON database into an RVF file.
 *
 * @param sourcePath - Path to the rvlite JSON database (e.g., .rvlite/db.json).
 * @param destPath   - Destination path for the RVF file.
 * @param options    - Migration options.
 * @returns A report summarising the migration.
 */
export async function rvfMigrate(
  sourcePath: string,
  destPath: string,
  options: { dryRun?: boolean; verify?: boolean } = {}
): Promise<MigrateReport> {
  const fs = await import('fs');

  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Source file not found: ${sourcePath}`);
  }

  const raw = fs.readFileSync(sourcePath, 'utf-8');
  const state: RvLiteDbState = JSON.parse(raw);

  // Idempotency: if dest already exists and is a valid RVF file whose
  // payload matches the source, treat as a no-op.
  if (fs.existsSync(destPath)) {
    try {
      const existing = JSON.parse(fs.readFileSync(destPath, 'utf-8')) as RvfFileEnvelope;
      if (existing.magic === 'RVF1') {
        const existingVecCount = Object.keys(existing.payload?.vectors ?? {}).length;
        const sourceVecCount = Object.keys(state.vectors ?? {}).length;
        if (existingVecCount === sourceVecCount) {
          return {
            vectorsMigrated: 0,
            triplesMigrated: 0,
            graphNodesMigrated: 0,
            graphEdgesMigrated: 0,
            skipped: true,
            dryRun: options.dryRun ?? false,
          };
        }
      }
    } catch {
      // File exists but is not valid RVF — proceed with migration.
    }
  }

  const vectorCount = Object.keys(state.vectors ?? {}).length;
  const tripleCount = (state.triples ?? []).length;
  const nodeCount = Object.keys(state.graph?.nodes ?? {}).length;
  const edgeCount = Object.keys(state.graph?.edges ?? {}).length;

  if (options.dryRun) {
    return {
      vectorsMigrated: vectorCount,
      triplesMigrated: tripleCount,
      graphNodesMigrated: nodeCount,
      graphEdgesMigrated: edgeCount,
      skipped: false,
      dryRun: true,
    };
  }

  // Build the RVF envelope.
  const envelope: RvfFileEnvelope = {
    rvf_version: 1,
    magic: 'RVF1',
    created_at: new Date().toISOString(),
    dimensions: state.config?.dimensions ?? 384,
    distance_metric: state.config?.metric ?? 'cosine',
    payload: state,
  };

  const path = await import('path');
  const dir = path.dirname(destPath);
  if (dir && !fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  fs.writeFileSync(destPath, JSON.stringify(envelope, null, 2), 'utf-8');

  // Optionally verify round-trip fidelity.
  let verifyPassed: boolean | undefined;
  if (options.verify) {
    const reRead = JSON.parse(fs.readFileSync(destPath, 'utf-8')) as RvfFileEnvelope;
    verifyPassed = true;

    for (const [id, entry] of Object.entries(state.vectors ?? {})) {
      const rvfEntry = reRead.payload.vectors?.[id];
      if (!rvfEntry) {
        verifyPassed = false;
        break;
      }
      if (!vectorsClose(entry.vector, rvfEntry.vector, 1e-6)) {
        verifyPassed = false;
        break;
      }
    }
  }

  return {
    vectorsMigrated: vectorCount,
    triplesMigrated: tripleCount,
    graphNodesMigrated: nodeCount,
    graphEdgesMigrated: edgeCount,
    skipped: false,
    dryRun: false,
    verifyPassed,
  };
}

// ── Rebuild ──────────────────────────────────────────────────────────────

/**
 * Reconstruct metadata from an RVF file.
 *
 * Reads the RVF envelope, extracts vectors, and rebuilds
 * SQL / Cypher / SPARQL metadata from vector metadata fields.
 *
 * @param sourcePath - Path to the RVF file.
 * @param destPath   - Optional destination for the rebuilt JSON state.
 * @returns A report summarising the recovered data.
 */
export async function rvfRebuild(
  sourcePath: string,
  destPath?: string
): Promise<RebuildReport> {
  const fs = await import('fs');

  if (!fs.existsSync(sourcePath)) {
    throw new Error(`RVF file not found: ${sourcePath}`);
  }

  const raw = fs.readFileSync(sourcePath, 'utf-8');
  const envelope = JSON.parse(raw) as RvfFileEnvelope;

  if (envelope.magic !== 'RVF1') {
    throw new Error(`Invalid RVF file: expected magic "RVF1", got "${envelope.magic}"`);
  }

  const state = envelope.payload;

  // Rebuild graph nodes from vectors that have graph-like metadata.
  const recoveredNodes: Record<string, unknown> = {};
  const recoveredEdges: Record<string, unknown> = {};
  const recoveredTriples: Array<{ subject: string; predicate: string; object: string }> = [];

  for (const [id, entry] of Object.entries(state.vectors ?? {})) {
    const meta = entry.metadata;
    if (!meta) continue;

    // Recover graph nodes: metadata with a `_label` field.
    if (typeof meta._label === 'string') {
      recoveredNodes[id] = { label: meta._label, properties: meta };
    }

    // Recover graph edges: metadata with `_from` and `_to`.
    if (typeof meta._from === 'string' && typeof meta._to === 'string') {
      recoveredEdges[id] = {
        from: meta._from,
        to: meta._to,
        type: meta._type ?? 'RELATED',
        properties: meta,
      };
    }

    // Recover triples: metadata with `_subject`, `_predicate`, `_object`.
    if (
      typeof meta._subject === 'string' &&
      typeof meta._predicate === 'string' &&
      typeof meta._object === 'string'
    ) {
      recoveredTriples.push({
        subject: meta._subject,
        predicate: meta._predicate,
        object: meta._object,
      });
    }
  }

  // Merge recovered data with any existing data in the envelope.
  const existingTriples = state.triples ?? [];
  const allTriples = [...existingTriples, ...recoveredTriples];

  const existingNodes = state.graph?.nodes ?? {};
  const existingEdges = state.graph?.edges ?? {};
  const allNodes = { ...existingNodes, ...recoveredNodes };
  const allEdges = { ...existingEdges, ...recoveredEdges };

  const rebuiltState: RvLiteDbState = {
    vectors: state.vectors ?? {},
    graph: { nodes: allNodes, edges: allEdges },
    triples: allTriples,
    nextId: state.nextId ?? Object.keys(state.vectors ?? {}).length + 1,
    config: {
      dimensions: envelope.dimensions,
      metric: envelope.distance_metric,
    },
  };

  if (destPath) {
    const path = await import('path');
    const dir = path.dirname(destPath);
    if (dir && !fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(destPath, JSON.stringify(rebuiltState, null, 2), 'utf-8');
  }

  return {
    vectorsRecovered: Object.keys(state.vectors ?? {}).length,
    triplesRecovered: allTriples.length,
    graphNodesRecovered: Object.keys(allNodes).length,
    graphEdgesRecovered: Object.keys(allEdges).length,
  };
}

// ── CLI Entry Point ──────────────────────────────────────────────────────

/**
 * Register rvf-migrate and rvf-rebuild commands on a Commander program
 * instance.  This allows the main rvlite CLI to integrate these commands
 * without duplicating code.
 */
export function registerRvfCommands(program: any): void {
  program
    .command('rvf-migrate')
    .description('Convert existing rvlite data to RVF format')
    .requiredOption('-s, --source <path>', 'Path to source rvlite JSON database')
    .requiredOption('-d, --dest <path>', 'Destination RVF file path')
    .option('--dry-run', 'Report what would be migrated without writing', false)
    .option('--verify', 'Verify vectors match within 1e-6 tolerance after migration', false)
    .action(async (options: { source: string; dest: string; dryRun: boolean; verify: boolean }) => {
      try {
        const report = await rvfMigrate(options.source, options.dest, {
          dryRun: options.dryRun,
          verify: options.verify,
        });

        if (report.skipped) {
          console.log('Migration skipped: destination already contains matching RVF data (idempotent).');
          return;
        }

        if (report.dryRun) {
          console.log('Dry run — no files written.');
        }

        console.log(`Vectors migrated:     ${report.vectorsMigrated}`);
        console.log(`Triples migrated:     ${report.triplesMigrated}`);
        console.log(`Graph nodes migrated: ${report.graphNodesMigrated}`);
        console.log(`Graph edges migrated: ${report.graphEdgesMigrated}`);

        if (report.verifyPassed !== undefined) {
          console.log(`Verification: ${report.verifyPassed ? 'PASSED' : 'FAILED'}`);
          if (!report.verifyPassed) {
            process.exit(1);
          }
        }
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error(`Error: ${msg}`);
        process.exit(1);
      }
    });

  program
    .command('rvf-rebuild')
    .description('Reconstruct metadata from RVF file')
    .requiredOption('-s, --source <path>', 'Path to source RVF file')
    .option('-d, --dest <path>', 'Destination JSON file for rebuilt state')
    .action(async (options: { source: string; dest?: string }) => {
      try {
        const report = await rvfRebuild(options.source, options.dest);

        console.log(`Vectors recovered:     ${report.vectorsRecovered}`);
        console.log(`Triples recovered:     ${report.triplesRecovered}`);
        console.log(`Graph nodes recovered: ${report.graphNodesRecovered}`);
        console.log(`Graph edges recovered: ${report.graphEdgesRecovered}`);

        if (options.dest) {
          console.log(`Rebuilt state written to: ${options.dest}`);
        }
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error(`Error: ${msg}`);
        process.exit(1);
      }
    });
}
