# Bounded Context Map - Genomic Analysis Platform

## Context Map Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GENOMIC ANALYSIS PLATFORM                            │
└─────────────────────────────────────────────────────────────────────────────┘

    ┌──────────────────┐
    │   Pipeline       │ ◄───────── Orchestration Layer
    │   Context        │
    └────────┬─────────┘
             │ ACL (maps domain events to pipeline commands)
             │
    ┌────────┴─────────────────────────────────────────────┐
    │                                                       │
    ▼                                                       ▼
┌─────────────────┐                               ┌─────────────────┐
│   Sequence      │ Customer-Supplier             │   Alignment     │
│   Context       ├──────────────────────────────►│   Context       │
│                 │ (provides k-mer indices)      │                 │
└────────┬────────┘                               └────────┬────────┘
         │                                                 │
         │ Shared Kernel (GenomicPosition, QualityScore)  │
         │                                                 │
         ▼                                                 ▼
┌─────────────────┐                               ┌─────────────────┐
│   Variant       │                               │   Protein       │
│   Context       │◄──────────────────────────────┤   Context       │
│                 │  Partner (variant→structure)  │                 │
└────────┬────────┘                               └─────────────────┘
         │
         │ ACL (translates variants to epigenetic events)
         │
         ▼
┌─────────────────┐
│  Epigenomic     │
│  Context        │
└────────┬────────┘
         │
         │ Customer-Supplier (epigenetic→drug response)
         │
         ▼
┌─────────────────┐
│ Pharmacogenomic │
│ Context         │
└─────────────────┘

Legend:
  Customer-Supplier: →  (upstream provides services to downstream)
  Shared Kernel:     ├─┤ (shared domain model)
  Partner:           ◄─► (mutual dependency)
  ACL:               [A] (anti-corruption layer)
```

## 1. Sequence Context

**Module**: `kmer.rs`

**Responsibility**: K-mer indexing, sequence sketching, and similarity search

**Core Aggregates**:
- `KmerIndex` - Root aggregate managing k-mer → position mappings
- `MinHashSketch` - Aggregate for approximate sequence similarity

**Key Types**:
```rust
pub struct KmerEncoder {
    k: usize,
    alphabet_size: usize,
}

pub struct KmerIndex {
    k: usize,
    index: HashMap<u64, Vec<usize>>, // k-mer hash → positions
}

pub struct MinHashSketch {
    k: usize,
    num_hashes: usize,
    signatures: Vec<u64>,
}
```

**Published Events**:
- `SequenceIndexed { sequence_id: String, kmer_count: usize }`
- `SimilarSequenceFound { query_id: String, match_id: String, similarity: f64 }`

**Domain Language**:
- K-mer: substring of length k
- Minimizer: canonical k-mer representation
- Sketch: compressed sequence signature
- Jaccard similarity: set overlap metric

**Invariants**:
- K-mer length must be 3 ≤ k ≤ 32
- MinHash signature size must be ≥ 1
- All k-mers normalized to canonical form (min(kmer, reverse_complement))

## 2. Alignment Context

**Module**: `alignment.rs`

**Responsibility**: Sequence alignment using attention mechanisms and motif detection

**Core Aggregates**:
- `AttentionAligner` - Root aggregate for pairwise sequence alignment
- `MotifScanner` - Aggregate for regulatory motif discovery

**Key Types**:
```rust
pub struct AttentionAligner {
    attention_service: Arc<AttentionService>,
    gap_penalty: f64,
    match_bonus: f64,
}

pub struct MotifScanner {
    attention_service: Arc<AttentionService>,
    min_score: f64,
    known_motifs: Vec<MotifPattern>,
}

pub struct AlignmentResult {
    pub score: f64,
    pub aligned_query: String,
    pub aligned_target: String,
    pub attention_weights: Vec<Vec<f64>>,
}
```

**Published Events**:
- `AlignmentCompleted { query_id: String, target_id: String, score: f64 }`
- `MotifDetected { sequence_id: String, motif: String, position: usize, score: f64 }`

**Domain Language**:
- Alignment: optimal mapping between two sequences
- Gap penalty: cost of insertions/deletions
- Attention weight: learned similarity between positions
- Motif: conserved sequence pattern (e.g., TATA box)
- PWM (Position Weight Matrix): motif scoring matrix

**Invariants**:
- Gap penalty must be negative
- Match bonus must be positive
- Motif minimum score 0.0 ≤ score ≤ 1.0
- Alignment score monotonically decreases with gaps

**Relationship with Sequence Context**:
- **Type**: Customer-Supplier
- **Direction**: Sequence → Alignment
- **Integration**: Alignment consumes k-mer indices for fast seed-and-extend
- **Translation**: None (direct dependency)

## 3. Variant Context

**Module**: `variant.rs`

**Responsibility**: Variant calling, genotyping, and population genetics

**Core Aggregates**:
- `VariantDatabase` - Root aggregate managing variant collection
- `VariantCaller` - Service aggregate for variant detection

**Key Types**:
```rust
pub struct VariantCaller {
    min_quality: f64,
    min_depth: usize,
    gnn_service: Arc<GnnService>,
}

pub struct Variant {
    pub position: GenomicPosition,
    pub reference: String,
    pub alternate: String,
    pub quality: f64,
    pub genotype: Genotype,
    pub depth: usize,
    pub allele_frequency: Option<f64>,
}

pub struct VariantDatabase {
    variants: HashMap<GenomicPosition, Variant>,
    graph_index: Option<GraphIndex>, // GNN-based variant relationships
}

pub enum Genotype {
    Homozygous(Allele),
    Heterozygous(Allele, Allele),
}
```

**Published Events**:
- `VariantCalled { position: GenomicPosition, variant: Variant }`
- `GenotypeUpdated { sample_id: String, position: GenomicPosition, genotype: Genotype }`
- `PopulationFrequencyCalculated { variant_id: String, frequency: f64 }`

**Domain Language**:
- SNP (Single Nucleotide Polymorphism): single base change
- Indel: insertion or deletion
- Genotype: allele combination (0/0, 0/1, 1/1)
- Allele frequency: population prevalence
- Quality score: confidence in variant call (Phred scale)
- Coverage depth: number of reads supporting variant

**Invariants**:
- Quality score ≥ 0 (Phred scale)
- Coverage depth ≥ 1
- Allele frequency 0.0 ≤ AF ≤ 1.0
- Reference and alternate alleles must differ
- Genotype alleles must match available alleles

**Relationship with Alignment Context**:
- **Type**: Customer-Supplier
- **Direction**: Alignment → Variant
- **Integration**: Variant caller uses alignment results to identify mismatches
- **Translation**: Alignment gaps → insertion/deletion variants

**Shared Kernel with Sequence Context**:
- `GenomicPosition { chromosome: String, position: usize }`
- `QualityScore(f64)` (Phred-scaled)
- `Nucleotide` enum (A, C, G, T)

## 4. Protein Context

**Module**: `protein.rs`

**Responsibility**: Protein structure prediction and contact map generation

**Core Aggregates**:
- `ProteinGraph` - Root aggregate representing protein as graph
- `ContactPredictor` - Service aggregate for 3D contact prediction

**Key Types**:
```rust
pub struct ProteinGraph {
    pub sequence: String, // amino acid sequence
    pub nodes: Vec<AminoAcid>,
    pub edges: Vec<(usize, usize, ContactType)>,
}

pub struct ContactPredictor {
    gnn_service: Arc<GnnService>,
    attention_service: Arc<AttentionService>,
    distance_threshold: f64, // Ångströms
}

pub struct ContactPrediction {
    pub residue_i: usize,
    pub residue_j: usize,
    pub probability: f64,
    pub distance: Option<f64>,
}

pub enum ContactType {
    Backbone,
    SideChain,
    HydrogenBond,
    DisulfideBridge,
}
```

**Published Events**:
- `ProteinTranslated { gene_id: String, protein_sequence: String }`
- `StructurePredicted { protein_id: String, contact_count: usize }`
- `FoldingPathwayComputed { protein_id: String, energy: f64 }`

**Domain Language**:
- Amino acid: protein building block (20 standard types)
- Residue: amino acid position in sequence
- Contact: spatial proximity between residues (<8Å)
- Secondary structure: local folding patterns (helix, sheet, loop)
- Tertiary structure: 3D protein fold
- Contact map: matrix of residue-residue distances

**Invariants**:
- Sequence length ≥ 1
- Contact probability 0.0 ≤ p ≤ 1.0
- Distance threshold > 0.0 (typically 8.0Å)
- Contact pairs must be |i - j| ≥ 4 (exclude local contacts)

**Relationship with Variant Context**:
- **Type**: Partner (bidirectional)
- **Direction**: Variant ↔ Protein
- **Integration**:
  - Variant → Protein: coding variants cause amino acid changes
  - Protein → Variant: structural changes inform variant pathogenicity
- **Translation**:
  - Variant ACL translates nucleotide changes to codon changes
  - Protein ACL maps structure disruption to clinical significance

## 5. Epigenomic Context

**Module**: `epigenomics.rs`

**Responsibility**: DNA methylation analysis and epigenetic age prediction

**Core Aggregates**:
- `EpigeneticIndex` - Root aggregate managing methylation sites
- `HorvathClock` - Service aggregate for epigenetic age calculation

**Key Types**:
```rust
pub struct MethylationProfile {
    pub cpg_sites: HashMap<GenomicPosition, f64>, // position → beta value
    pub total_sites: usize,
    pub mean_methylation: f64,
}

pub struct HorvathClock {
    pub coefficients: HashMap<String, f64>, // CpG site → weight
    pub intercept: f64,
}

pub struct CpGSite {
    pub position: GenomicPosition,
    pub beta_value: f64, // 0.0 (unmethylated) to 1.0 (methylated)
    pub coverage: usize,
}

pub struct EpigeneticAge {
    pub chronological_age: Option<f64>,
    pub predicted_age: f64,
    pub acceleration: f64, // predicted - chronological
}
```

**Published Events**:
- `MethylationProfileGenerated { sample_id: String, site_count: usize }`
- `EpigeneticAgeCalculated { sample_id: String, age: f64, acceleration: f64 }`
- `DifferentialMethylationDetected { region: GenomicRegion, delta_beta: f64 }`

**Domain Language**:
- CpG site: cytosine-guanine dinucleotide (methylation target)
- Beta value: methylation level (0 = unmethylated, 1 = fully methylated)
- Epigenetic clock: age predictor based on methylation
- Age acceleration: difference between epigenetic and chronological age
- DMR (Differentially Methylated Region): region with changed methylation

**Invariants**:
- Beta value 0.0 ≤ β ≤ 1.0
- Coverage ≥ 1
- Horvath coefficients sum to meaningful scale
- Age ≥ 0.0

**Relationship with Variant Context**:
- **Type**: Anti-Corruption Layer
- **Direction**: Variant → Epigenomic
- **Integration**: Variants in regulatory regions affect methylation patterns
- **Translation**:
  - ACL translates genetic variants to epigenetic effects
  - Maps SNPs → methylation quantitative trait loci (mQTL)
  - Prevents variant domain concepts from leaking into epigenetic model

## 6. Pharmacogenomic Context

**Module**: `pharma.rs`

**Responsibility**: Pharmacogenetic analysis and drug-gene interaction prediction

**Core Aggregates**:
- `DrugInteractionGraph` - Root aggregate representing drug-gene network
- `StarAlleleCaller` - Service aggregate for haplotype phasing

**Key Types**:
```rust
pub struct StarAlleleCaller {
    gene_definitions: HashMap<String, GeneDefinition>,
    min_coverage: usize,
}

pub struct StarAllele {
    pub gene: String,
    pub allele: String, // e.g., "*1", "*2", "*17"
    pub variants: Vec<Variant>,
    pub function: AlleleFunction,
}

pub enum AlleleFunction {
    Normal,
    Increased,
    Decreased,
    NoFunction,
}

pub struct DrugInteractionGraph {
    pub nodes: Vec<DrugGeneNode>,
    pub edges: Vec<(usize, usize, InteractionType)>,
}

pub struct DrugResponse {
    pub drug: String,
    pub diplotype: Diplotype,
    pub phenotype: MetabolizerPhenotype,
    pub recommendation: ClinicalRecommendation,
}

pub enum MetabolizerPhenotype {
    UltraRapid,
    Rapid,
    Normal,
    Intermediate,
    Poor,
}
```

**Published Events**:
- `StarAlleleIdentified { gene: String, allele: String, diplotype: String }`
- `DrugResponsePredicted { drug: String, phenotype: MetabolizerPhenotype }`
- `InteractionDetected { drug1: String, drug2: String, severity: Severity }`

**Domain Language**:
- Star allele: named haplotype variant (e.g., CYP2D6*4)
- Diplotype: pair of haplotypes (e.g., *1/*4)
- Metabolizer phenotype: drug metabolism rate
- Pharmacogene: gene affecting drug response
- Drug-gene interaction: how genetics modulates drug efficacy/toxicity

**Invariants**:
- Diplotype must have exactly 2 alleles
- Phenotype derivable from diplotype
- Coverage ≥ minimum threshold for calling
- All star allele variants must exist in variant database

**Relationship with Epigenomic Context**:
- **Type**: Customer-Supplier
- **Direction**: Epigenomic → Pharmacogenomic
- **Integration**: Methylation affects drug metabolism gene expression
- **Translation**: Methylation beta values → gene expression levels → phenotype

## 7. Pipeline Context

**Module**: `pipeline.rs`

**Responsibility**: Orchestration of multi-stage genomic analysis workflow

**Core Aggregates**:
- `GenomicPipeline` - Root aggregate orchestrating all contexts

**Key Types**:
```rust
pub struct GenomicPipeline {
    pub kmer_encoder: KmerEncoder,
    pub aligner: AttentionAligner,
    pub variant_caller: VariantCaller,
    pub protein_predictor: ContactPredictor,
    pub methylation_analyzer: MethylationAnalyzer,
    pub pharma_analyzer: StarAlleleCaller,
}

pub struct PipelineConfig {
    pub k: usize,
    pub min_variant_quality: f64,
    pub min_coverage: usize,
    pub enable_protein_prediction: bool,
    pub enable_epigenetic_analysis: bool,
    pub enable_pharmacogenomics: bool,
}

pub struct AnalysisResult {
    pub sequence_stats: SequenceStats,
    pub variants: Vec<Variant>,
    pub protein_structures: Vec<ProteinGraph>,
    pub methylation_profile: Option<MethylationProfile>,
    pub drug_responses: Vec<DrugResponse>,
}
```

**Published Events**:
- `PipelineStarted { sample_id: String, stages: Vec<String> }`
- `StageCompleted { stage: String, duration_ms: u64 }`
- `PipelineCompleted { sample_id: String, total_duration_ms: u64 }`
- `PipelineFailed { stage: String, error: String }`

**Domain Language**:
- Pipeline: directed acyclic graph of analysis stages
- Stage: atomic analysis unit (alignment, variant calling, etc.)
- Workflow: ordered execution of stages
- Checkpoint: saved intermediate state
- Provenance: lineage tracking of analysis steps

**Invariants**:
- All enabled stages must execute in dependency order
- Failed stage halts downstream execution
- All results traceable to input data and parameters

**Anti-Corruption Layers**:

The Pipeline Context uses ACLs to prevent downstream contexts from depending on upstream implementation details:

1. **Sequence ACL**: Translates k-mer indices to alignment seeds
2. **Alignment ACL**: Converts alignment gaps to variant candidates
3. **Variant ACL**: Maps variants to protein mutations
4. **Protein ACL**: Translates structure to functional predictions
5. **Epigenetic ACL**: Converts methylation to gene expression estimates
6. **Pharmacogenomic ACL**: Maps genotypes to clinical recommendations

## Context Relationship Matrix

| From ↓ / To → | Sequence | Alignment | Variant | Protein | Epigenomic | Pharma | Pipeline |
|---------------|----------|-----------|---------|---------|------------|--------|----------|
| Sequence      | -        | C-S       | SK      | SK      | -          | -      | ACL      |
| Alignment     | -        | -         | C-S     | -       | -          | -      | ACL      |
| Variant       | -        | -         | -       | Partner | ACL        | -      | ACL      |
| Protein       | -        | -         | Partner | -       | -          | -      | ACL      |
| Epigenomic    | -        | -         | -       | -       | -          | C-S    | ACL      |
| Pharma        | -        | -         | -       | -       | -          | -      | ACL      |
| Pipeline      | C-S      | C-S       | C-S     | C-S     | C-S        | C-S    | -        |

**Legend**:
- C-S: Customer-Supplier
- SK: Shared Kernel
- Partner: Partnership
- ACL: Anti-Corruption Layer

## Integration Patterns

### 1. Event-Driven Integration

Contexts communicate via domain events to maintain loose coupling:

```rust
// Example: Variant Context publishes event
pub enum DomainEvent {
    VariantCalled(VariantCalledEvent),
    ProteinStructurePredicted(ProteinPredictedEvent),
    // ...
}

// Pipeline Context subscribes and translates
impl EventHandler for GenomicPipeline {
    fn handle(&mut self, event: DomainEvent) {
        match event {
            DomainEvent::VariantCalled(e) => {
                if e.variant.is_coding() {
                    self.trigger_protein_analysis(e.variant);
                }
            }
            // ...
        }
    }
}
```

### 2. Shared Kernel Components

Core domain types shared across contexts:

```rust
// In types.rs (core domain)
pub struct GenomicPosition {
    pub chromosome: String,
    pub position: usize,
}

pub struct QualityScore(pub f64); // Phred-scaled

pub enum Nucleotide { A, C, G, T }

pub struct GenomicRegion {
    pub chromosome: String,
    pub start: usize,
    pub end: usize,
}
```

### 3. Anti-Corruption Layer Example

```rust
// Variant → Protein ACL
pub struct VariantToProteinTranslator {
    codon_table: CodonTable,
}

impl VariantToProteinTranslator {
    pub fn translate_variant(&self, variant: &Variant) -> Option<ProteinMutation> {
        // Prevents protein context from depending on variant implementation
        let codon_change = self.map_to_codon(variant)?;
        let aa_change = self.codon_table.translate(codon_change)?;

        Some(ProteinMutation {
            position: variant.position.position / 3,
            reference_aa: aa_change.reference,
            alternate_aa: aa_change.alternate,
        })
    }
}
```

## Bounded Context Responsibilities Summary

1. **Sequence Context**: K-mer indexing and sequence similarity (foundation)
2. **Alignment Context**: Pairwise alignment and motif discovery
3. **Variant Context**: Variant calling and population genetics
4. **Protein Context**: Structure prediction and functional analysis
5. **Epigenomic Context**: Methylation profiling and age prediction
6. **Pharmacogenomic Context**: Drug-gene interactions and clinical recommendations
7. **Pipeline Context**: Workflow orchestration and result aggregation

Each context maintains its own ubiquitous language, domain model, and business rules while integrating through well-defined relationships.
