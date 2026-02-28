# Domain Model - Genomic Analysis Platform

## Overview

This document defines all entities, value objects, aggregates, and domain events across the seven bounded contexts. Each type is shown with its Rust signature and business rules.

## Core Domain Types (Shared Kernel)

### Value Objects

```rust
/// Genomic coordinate (immutable)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenomicPosition {
    pub chromosome: String,
    pub position: usize,
}

// Invariants:
// - chromosome must be valid (1-22, X, Y, MT)
// - position must be ≥ 1

/// Quality score using Phred scale: Q = -10 * log10(P_error)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityScore(pub f64);

// Invariants:
// - score ≥ 0
// - Q=10 means 10% error rate
// - Q=20 means 1% error rate
// - Q=30 means 0.1% error rate

/// Single nucleotide
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nucleotide {
    A, // Adenine
    C, // Cytosine
    G, // Guanine
    T, // Thymine
}

// Operations:
impl Nucleotide {
    pub fn complement(&self) -> Self;
    pub fn to_byte(&self) -> u8;
    pub fn from_byte(b: u8) -> Result<Self, Error>;
}

/// Genomic interval
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenomicRegion {
    pub chromosome: String,
    pub start: usize,
    pub end: usize,
}

// Invariants:
// - start < end
// - start ≥ 1
// - Same chromosome validity rules as GenomicPosition

/// Amino acid single-letter code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AminoAcid {
    A, C, D, E, F, G, H, I, K, L,
    M, N, P, Q, R, S, T, V, W, Y,
    Stop,
}

// Invariants:
// - 20 standard amino acids + stop codon
// - Each has specific properties (hydrophobic, charged, etc.)
```

## 1. Sequence Context Domain Model

### Aggregates

```rust
/// Aggregate Root: K-mer index for fast sequence search
pub struct KmerIndex {
    k: usize,
    index: HashMap<u64, Vec<usize>>, // k-mer hash → positions
    sequence_length: usize,
}

// Aggregate boundary: Controls all k-mer operations
// Invariants:
// - 3 ≤ k ≤ 32
// - All positions < sequence_length
// - K-mers stored in canonical form

impl KmerIndex {
    pub fn new(k: usize) -> Result<Self, Error>;
    pub fn index_sequence(&mut self, sequence: &[u8]) -> Result<(), Error>;
    pub fn query(&self, kmer: &[u8]) -> Vec<usize>;
    pub fn contains(&self, kmer: &[u8]) -> bool;
}

/// Aggregate: MinHash sketch for approximate similarity
pub struct MinHashSketch {
    k: usize,
    num_hashes: usize,
    signatures: Vec<u64>,
}

// Invariants:
// - num_hashes ≥ 1 (typically 128-1024)
// - signatures.len() == num_hashes
// - Signatures sorted in ascending order

impl MinHashSketch {
    pub fn new(k: usize, num_hashes: usize) -> Self;
    pub fn add_sequence(&mut self, sequence: &[u8]);
    pub fn jaccard_similarity(&self, other: &Self) -> f64;
}
```

### Entities

```rust
/// Entity: DNA sequence with metadata
#[derive(Debug, Clone)]
pub struct DnaSequence {
    pub id: String, // Identity
    pub sequence: Vec<u8>,
    pub quality_scores: Option<Vec<QualityScore>>,
    pub created_at: DateTime<Utc>,
}

// Invariants:
// - id must be unique
// - sequence contains only A, C, G, T, N
// - if quality_scores.is_some(), length must equal sequence length

impl DnaSequence {
    pub fn reverse_complement(&self) -> Self;
    pub fn gc_content(&self) -> f64;
    pub fn length(&self) -> usize;
}
```

### Value Objects

```rust
/// K-mer encoder configuration
#[derive(Debug, Clone, Copy)]
pub struct KmerConfig {
    pub k: usize,
    pub alphabet_size: usize,
}

// Invariants:
// - k ≥ 3
// - alphabet_size typically 4 (DNA) or 20 (protein)
```

### Domain Events

```rust
pub enum SequenceEvent {
    SequenceIndexed {
        sequence_id: String,
        kmer_count: usize,
        timestamp: DateTime<Utc>,
    },
    SimilarSequenceFound {
        query_id: String,
        match_id: String,
        similarity: f64,
        timestamp: DateTime<Utc>,
    },
}
```

## 2. Alignment Context Domain Model

### Aggregates

```rust
/// Aggregate Root: Attention-based sequence aligner
pub struct AttentionAligner {
    attention_service: Arc<AttentionService>,
    gap_penalty: f64,
    match_bonus: f64,
}

// Invariants:
// - gap_penalty < 0
// - match_bonus > 0
// - |gap_penalty| < match_bonus (gaps should be costly)

impl AttentionAligner {
    pub fn align(&self, query: &[u8], target: &[u8])
        -> Result<AlignmentResult, Error>;
    pub fn batch_align(&self, pairs: Vec<(&[u8], &[u8])>)
        -> Result<Vec<AlignmentResult>, Error>;
}

/// Aggregate: Motif scanner for regulatory elements
pub struct MotifScanner {
    attention_service: Arc<AttentionService>,
    min_score: f64,
    known_motifs: Vec<MotifPattern>,
}

// Invariants:
// - 0.0 ≤ min_score ≤ 1.0
// - All motif patterns valid (length ≥ 4)

impl MotifScanner {
    pub fn scan(&self, sequence: &[u8]) -> Vec<MotifMatch>;
    pub fn add_motif(&mut self, pattern: MotifPattern);
}
```

### Value Objects

```rust
/// Alignment result (immutable)
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    pub score: f64,
    pub aligned_query: String,
    pub aligned_target: String,
    pub attention_weights: Vec<Vec<f64>>,
    pub identity: f64, // % exact matches
    pub gaps: usize,
}

// Invariants:
// - aligned_query.len() == aligned_target.len()
// - 0.0 ≤ identity ≤ 1.0
// - attention_weights dimensions match alignment length

/// Motif pattern definition
#[derive(Debug, Clone)]
pub struct MotifPattern {
    pub name: String,
    pub consensus: String, // IUPAC nucleotide codes
    pub pwm: Vec<[f64; 4]>, // Position Weight Matrix
}

// Invariants:
// - consensus.len() == pwm.len()
// - Each PWM position sums to ~1.0
// - pwm.len() ≥ 4

/// Motif match location
#[derive(Debug, Clone)]
pub struct MotifMatch {
    pub motif_name: String,
    pub position: usize,
    pub score: f64,
    pub strand: Strand,
}

#[derive(Debug, Clone, Copy)]
pub enum Strand {
    Forward,
    Reverse,
}
```

### Domain Events

```rust
pub enum AlignmentEvent {
    AlignmentCompleted {
        query_id: String,
        target_id: String,
        score: f64,
        timestamp: DateTime<Utc>,
    },
    MotifDetected {
        sequence_id: String,
        motif: String,
        position: usize,
        score: f64,
        timestamp: DateTime<Utc>,
    },
}
```

## 3. Variant Context Domain Model

### Aggregates

```rust
/// Aggregate Root: Collection of genetic variants
pub struct VariantDatabase {
    variants: HashMap<GenomicPosition, Variant>,
    graph_index: Option<GraphIndex>,
    population_frequencies: HashMap<String, f64>,
}

// Aggregate boundary: Ensures variant consistency and relationships
// Invariants:
// - No duplicate positions
// - All frequencies 0.0 ≤ f ≤ 1.0
// - Graph index consistent with variant set

impl VariantDatabase {
    pub fn add_variant(&mut self, variant: Variant) -> Result<(), Error>;
    pub fn get_variant(&self, pos: &GenomicPosition) -> Option<&Variant>;
    pub fn variants_in_region(&self, region: &GenomicRegion) -> Vec<&Variant>;
    pub fn update_frequency(&mut self, pos: &GenomicPosition, freq: f64);
}

/// Service Aggregate: Variant calling engine
pub struct VariantCaller {
    min_quality: f64,
    min_depth: usize,
    gnn_service: Arc<GnnService>,
}

// Invariants:
// - min_quality ≥ 0
// - min_depth ≥ 1

impl VariantCaller {
    pub fn call_variants(&self, reads: &[Read], reference: &[u8])
        -> Result<Vec<Variant>, Error>;
    pub fn genotype(&self, variant: &Variant, reads: &[Read])
        -> Result<Genotype, Error>;
}
```

### Entities

```rust
/// Entity: Genetic variant with identity at genomic position
#[derive(Debug, Clone)]
pub struct Variant {
    pub position: GenomicPosition, // Identity (part of)
    pub reference: String,
    pub alternate: String,
    pub variant_type: VariantType,
    pub quality: f64,
    pub genotype: Genotype,
    pub depth: usize,
    pub allele_frequency: Option<f64>,
    pub annotations: Vec<Annotation>,
}

// Invariants:
// - reference != alternate
// - quality ≥ 0
// - depth ≥ 1
// - if allele_frequency.is_some(), 0.0 ≤ f ≤ 1.0
// - variant_type consistent with reference/alternate

impl Variant {
    pub fn is_snp(&self) -> bool;
    pub fn is_indel(&self) -> bool;
    pub fn is_coding(&self) -> bool;
    pub fn clinical_significance(&self) -> ClinicalSignificance;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantType {
    SNP,
    Insertion,
    Deletion,
    MNP, // Multi-nucleotide polymorphism
    Complex,
}
```

### Value Objects

```rust
/// Genotype representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Genotype {
    Homozygous(Allele),
    Heterozygous(Allele, Allele),
}

// Invariants:
// - Heterozygous alleles must differ
// - Alleles must match variant's reference/alternate

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allele {
    Reference,
    Alternate,
}

/// Variant annotation
#[derive(Debug, Clone)]
pub struct Annotation {
    pub gene: String,
    pub consequence: Consequence,
    pub impact: Impact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consequence {
    Synonymous,
    Missense,
    Nonsense,
    FrameShift,
    SpliceSite,
    Regulatory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impact {
    High,
    Moderate,
    Low,
    Modifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClinicalSignificance {
    Benign,
    LikelyBenign,
    VUS, // Variant of Uncertain Significance
    LikelyPathogenic,
    Pathogenic,
}
```

### Domain Events

```rust
pub enum VariantEvent {
    VariantCalled {
        position: GenomicPosition,
        variant: Variant,
        timestamp: DateTime<Utc>,
    },
    GenotypeUpdated {
        sample_id: String,
        position: GenomicPosition,
        genotype: Genotype,
        timestamp: DateTime<Utc>,
    },
    PopulationFrequencyCalculated {
        variant_id: String,
        frequency: f64,
        population: String,
        timestamp: DateTime<Utc>,
    },
}
```

## 4. Protein Context Domain Model

### Aggregates

```rust
/// Aggregate Root: Protein represented as graph
pub struct ProteinGraph {
    pub id: String,
    pub sequence: String, // Amino acid sequence
    pub nodes: Vec<Residue>,
    pub edges: Vec<Contact>,
    pub secondary_structure: Vec<SecondaryStructureElement>,
}

// Aggregate boundary: Manages all structural relationships
// Invariants:
// - nodes.len() == sequence.len()
// - All edge indices < nodes.len()
// - No duplicate contacts

impl ProteinGraph {
    pub fn from_sequence(sequence: String) -> Self;
    pub fn add_contact(&mut self, i: usize, j: usize, contact_type: ContactType);
    pub fn contact_map(&self) -> Vec<Vec<f64>>;
    pub fn fold_energy(&self) -> f64;
}

/// Service Aggregate: 3D contact prediction
pub struct ContactPredictor {
    gnn_service: Arc<GnnService>,
    attention_service: Arc<AttentionService>,
    distance_threshold: f64,
}

// Invariants:
// - distance_threshold > 0.0 (typically 8.0 Ångströms)

impl ContactPredictor {
    pub fn predict_contacts(&self, sequence: &str)
        -> Result<Vec<ContactPrediction>, Error>;
    pub fn predict_structure(&self, sequence: &str)
        -> Result<ProteinGraph, Error>;
}
```

### Entities

```rust
/// Entity: Amino acid residue in protein
#[derive(Debug, Clone)]
pub struct Residue {
    pub position: usize, // Identity
    pub amino_acid: AminoAcid,
    pub phi_angle: Option<f64>, // Backbone dihedral
    pub psi_angle: Option<f64>, // Backbone dihedral
    pub secondary_structure: Option<SecondaryStructure>,
}

// Invariants:
// - position ≥ 1
// - -180° ≤ phi, psi ≤ 180°
```

### Value Objects

```rust
/// Contact between residues
#[derive(Debug, Clone)]
pub struct Contact {
    pub residue_i: usize,
    pub residue_j: usize,
    pub contact_type: ContactType,
    pub distance: Option<f64>, // Ångströms
}

// Invariants:
// - residue_i < residue_j (ordered)
// - |residue_i - residue_j| ≥ 4 (exclude local contacts)
// - if distance.is_some(), distance > 0.0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactType {
    Backbone,
    SideChain,
    HydrogenBond,
    DisulfideBridge,
}

/// Contact prediction with confidence
#[derive(Debug, Clone)]
pub struct ContactPrediction {
    pub residue_i: usize,
    pub residue_j: usize,
    pub probability: f64,
    pub distance: Option<f64>,
}

// Invariants:
// - 0.0 ≤ probability ≤ 1.0

/// Secondary structure element
#[derive(Debug, Clone)]
pub struct SecondaryStructureElement {
    pub start: usize,
    pub end: usize,
    pub structure_type: SecondaryStructure,
}

// Invariants:
// - start < end

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryStructure {
    Helix,      // α-helix
    Sheet,      // β-sheet
    Loop,       // Random coil
    Turn,       // β-turn
}

/// Protein mutation
#[derive(Debug, Clone)]
pub struct ProteinMutation {
    pub position: usize,
    pub reference_aa: AminoAcid,
    pub alternate_aa: AminoAcid,
    pub structural_impact: f64, // 0.0-1.0
}
```

### Domain Events

```rust
pub enum ProteinEvent {
    ProteinTranslated {
        gene_id: String,
        protein_sequence: String,
        timestamp: DateTime<Utc>,
    },
    StructurePredicted {
        protein_id: String,
        contact_count: usize,
        confidence: f64,
        timestamp: DateTime<Utc>,
    },
}
```

## 5. Epigenomic Context Domain Model

### Aggregates

```rust
/// Aggregate Root: Epigenetic modification index
pub struct EpigeneticIndex {
    cpg_sites: HashMap<GenomicPosition, CpGSite>,
    methylation_profile: MethylationProfile,
}

// Aggregate boundary: Manages methylation data consistency
// Invariants:
// - All CpG sites have valid genomic positions
// - Beta values 0.0 ≤ β ≤ 1.0

impl EpigeneticIndex {
    pub fn add_site(&mut self, site: CpGSite) -> Result<(), Error>;
    pub fn get_profile(&self) -> &MethylationProfile;
    pub fn differential_methylation(&self, other: &Self)
        -> Vec<DifferentialRegion>;
}

/// Service Aggregate: Epigenetic age calculator
pub struct HorvathClock {
    coefficients: HashMap<String, f64>,
    intercept: f64,
}

// Invariants:
// - At least 353 CpG sites (original Horvath model)
// - Coefficients normalized

impl HorvathClock {
    pub fn predict_age(&self, profile: &MethylationProfile)
        -> Result<EpigeneticAge, Error>;
}
```

### Entities

```rust
/// Entity: Methylation profile for sample
#[derive(Debug, Clone)]
pub struct MethylationProfile {
    pub sample_id: String, // Identity
    pub cpg_sites: HashMap<GenomicPosition, f64>,
    pub total_sites: usize,
    pub mean_methylation: f64,
    pub created_at: DateTime<Utc>,
}

// Invariants:
// - cpg_sites.len() ≤ total_sites
// - All beta values 0.0 ≤ β ≤ 1.0
// - mean_methylation = average of all beta values

impl MethylationProfile {
    pub fn global_methylation(&self) -> f64;
    pub fn region_methylation(&self, region: &GenomicRegion) -> f64;
}
```

### Value Objects

```rust
/// CpG methylation site
#[derive(Debug, Clone)]
pub struct CpGSite {
    pub position: GenomicPosition,
    pub beta_value: f64, // 0.0 = unmethylated, 1.0 = fully methylated
    pub coverage: usize,
    pub quality: QualityScore,
}

// Invariants:
// - 0.0 ≤ beta_value ≤ 1.0
// - coverage ≥ 1

/// Epigenetic age prediction
#[derive(Debug, Clone)]
pub struct EpigeneticAge {
    pub chronological_age: Option<f64>,
    pub predicted_age: f64,
    pub acceleration: f64, // predicted - chronological
    pub confidence_interval: (f64, f64),
}

// Invariants:
// - predicted_age ≥ 0.0
// - confidence_interval.0 < confidence_interval.1

/// Differentially methylated region
#[derive(Debug, Clone)]
pub struct DifferentialRegion {
    pub region: GenomicRegion,
    pub delta_beta: f64,
    pub p_value: f64,
}

// Invariants:
// - -1.0 ≤ delta_beta ≤ 1.0
// - 0.0 ≤ p_value ≤ 1.0
```

### Domain Events

```rust
pub enum EpigenomicEvent {
    MethylationProfileGenerated {
        sample_id: String,
        site_count: usize,
        timestamp: DateTime<Utc>,
    },
    EpigeneticAgeCalculated {
        sample_id: String,
        age: f64,
        acceleration: f64,
        timestamp: DateTime<Utc>,
    },
}
```

## 6. Pharmacogenomic Context Domain Model

### Aggregates

```rust
/// Aggregate Root: Drug-gene interaction network
pub struct DrugInteractionGraph {
    nodes: Vec<DrugGeneNode>,
    edges: Vec<Interaction>,
    phenotype_map: HashMap<Diplotype, MetabolizerPhenotype>,
}

// Aggregate boundary: Manages pharmacogenetic relationships
// Invariants:
// - All edge indices valid
// - All diplotypes map to phenotypes

impl DrugInteractionGraph {
    pub fn add_interaction(&mut self, interaction: Interaction);
    pub fn predict_response(&self, drug: &str, diplotype: &Diplotype)
        -> DrugResponse;
}

/// Service Aggregate: Star allele haplotype caller
pub struct StarAlleleCaller {
    gene_definitions: HashMap<String, GeneDefinition>,
    min_coverage: usize,
}

// Invariants:
// - min_coverage ≥ 1
// - All genes have valid definitions

impl StarAlleleCaller {
    pub fn call_alleles(&self, variants: &[Variant], gene: &str)
        -> Result<Diplotype, Error>;
}
```

### Entities

```rust
/// Entity: Star allele definition
#[derive(Debug, Clone)]
pub struct StarAllele {
    pub id: String, // Identity (e.g., "CYP2D6*4")
    pub gene: String,
    pub allele: String,
    pub variants: Vec<Variant>,
    pub function: AlleleFunction,
    pub activity_score: f64,
}

// Invariants:
// - id format: "{gene}*{allele_number}"
// - 0.0 ≤ activity_score ≤ 2.0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlleleFunction {
    Normal,
    Increased,
    Decreased,
    NoFunction,
}
```

### Value Objects

```rust
/// Diplotype (pair of haplotypes)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diplotype {
    pub allele1: String,
    pub allele2: String,
}

// Invariants:
// - Both alleles non-empty
// - Canonical ordering (allele1 ≤ allele2 lexicographically)

/// Metabolizer phenotype derived from diplotype
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetabolizerPhenotype {
    UltraRapid,   // Activity score > 2.0
    Rapid,        // Activity score 1.5-2.0
    Normal,       // Activity score 1.0-1.5
    Intermediate, // Activity score 0.5-1.0
    Poor,         // Activity score < 0.5
}

impl MetabolizerPhenotype {
    pub fn from_activity_score(score: f64) -> Self;
}

/// Drug response prediction
#[derive(Debug, Clone)]
pub struct DrugResponse {
    pub drug: String,
    pub diplotype: Diplotype,
    pub phenotype: MetabolizerPhenotype,
    pub recommendation: ClinicalRecommendation,
}

#[derive(Debug, Clone)]
pub struct ClinicalRecommendation {
    pub recommendation_type: RecommendationType,
    pub dosage_adjustment: Option<f64>, // Multiplier
    pub alternative_drug: Option<String>,
    pub cpic_level: CpicLevel, // CPIC evidence level
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationType {
    Standard,
    IncreaseDose,
    DecreaseDose,
    AlternativeDrug,
    Contraindicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpicLevel {
    A, // High evidence
    B, // Moderate evidence
    C, // Low evidence
    D, // Preclinical evidence
}

/// Drug-gene interaction
#[derive(Debug, Clone)]
pub struct Interaction {
    pub drug: String,
    pub gene: String,
    pub interaction_type: InteractionType,
    pub strength: f64, // 0.0-1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    Metabolism,
    Transport,
    Target,
    Toxicity,
}
```

### Domain Events

```rust
pub enum PharmacogenomicEvent {
    StarAlleleIdentified {
        gene: String,
        allele: String,
        diplotype: String,
        timestamp: DateTime<Utc>,
    },
    DrugResponsePredicted {
        drug: String,
        phenotype: MetabolizerPhenotype,
        recommendation: RecommendationType,
        timestamp: DateTime<Utc>,
    },
}
```

## 7. Pipeline Context Domain Model

### Aggregates

```rust
/// Aggregate Root: Complete genomic analysis workflow
pub struct GenomicPipeline {
    pub id: String,
    pub config: PipelineConfig,
    stages: Vec<PipelineStage>,
    state: PipelineState,
    results: AnalysisResult,
}

// Aggregate boundary: Orchestrates all analysis contexts
// Invariants:
// - Stages execute in dependency order
// - No stage runs until dependencies complete
// - Failed stage prevents downstream execution

impl GenomicPipeline {
    pub fn new(config: PipelineConfig) -> Self;
    pub fn run(&mut self, input: SequenceData) -> Result<AnalysisResult, Error>;
    pub fn run_stage(&mut self, stage: &str) -> Result<(), Error>;
    pub fn checkpoint(&self) -> Result<(), Error>;
    pub fn restore(checkpoint_id: &str) -> Result<Self, Error>;
}
```

### Value Objects

```rust
/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub k: usize,
    pub min_variant_quality: f64,
    pub min_coverage: usize,
    pub enable_protein_prediction: bool,
    pub enable_epigenetic_analysis: bool,
    pub enable_pharmacogenomics: bool,
}

/// Analysis stage definition
#[derive(Debug, Clone)]
pub struct PipelineStage {
    pub name: String,
    pub dependencies: Vec<String>,
    pub timeout: Duration,
    pub retries: usize,
}

/// Pipeline execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Idle,
    Running,
    Completed,
    Failed,
}

/// Complete analysis result
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub sequence_stats: SequenceStats,
    pub variants: Vec<Variant>,
    pub protein_structures: Vec<ProteinGraph>,
    pub methylation_profile: Option<MethylationProfile>,
    pub drug_responses: Vec<DrugResponse>,
    pub execution_time: Duration,
}

#[derive(Debug, Clone)]
pub struct SequenceStats {
    pub total_length: usize,
    pub gc_content: f64,
    pub n_count: usize,
    pub quality_mean: f64,
}
```

### Domain Events

```rust
pub enum PipelineEvent {
    PipelineStarted {
        pipeline_id: String,
        stages: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    StageCompleted {
        pipeline_id: String,
        stage: String,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    PipelineCompleted {
        pipeline_id: String,
        total_duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    PipelineFailed {
        pipeline_id: String,
        stage: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
}
```

## Business Rules Summary

### Cross-Cutting Rules

1. **Quality Thresholds**: All data must meet minimum quality scores
2. **Validation**: Input data validated at bounded context entry points
3. **Traceability**: All results traceable to source data and parameters
4. **Consistency**: Aggregates maintain internal consistency invariants

### Context-Specific Rules

**Sequence Context**:
- K-mer indices use canonical (lexicographically minimal) representation
- MinHash signatures maintain cardinality for accurate similarity

**Alignment Context**:
- Gap penalties never exceed match bonuses
- Motif matches require minimum conservation score

**Variant Context**:
- Variants only called above quality and coverage thresholds
- Population frequencies sum to 1.0 across all samples
- Clinical significance based on ClinVar/evidence database

**Protein Context**:
- Contacts only between residues separated by ≥4 positions
- Secondary structure assignments mutually exclusive

**Epigenomic Context**:
- Beta values strictly bounded [0.0, 1.0]
- Epigenetic age non-negative

**Pharmacogenomic Context**:
- Diplotypes sorted in canonical order
- Phenotypes deterministically derived from diplotype activity scores
- CPIC recommendations follow evidence-based guidelines

**Pipeline Context**:
- Stage execution respects dependency DAG
- Checkpoints enable recovery from failures
- Configuration immutable during pipeline run

## Aggregate Invariants

Each aggregate root enforces these invariants:

1. **Identity**: Unique identifier within bounded context
2. **Completeness**: All required fields populated
3. **Consistency**: Related entities maintain referential integrity
4. **Validity**: All values within acceptable ranges
5. **Atomicity**: Changes commit or rollback as unit

These invariants ensure domain model correctness across all bounded contexts.
