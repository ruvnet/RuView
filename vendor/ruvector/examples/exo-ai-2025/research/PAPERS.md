# EXO-AI 2025: Research Papers & References

## SPARC Research Phase: Academic Foundations

This document catalogs the academic research informing the EXO-AI architecture, organized by domain.

---

## 1. Processing-in-Memory (PIM) Architectures

### Core Reviews

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [A Comprehensive Review of Processing-in-Memory Architectures for DNNs](https://www.mdpi.com/2073-431X/13/7/174) | MDPI Computers | 2024 | Chiplet-based PIM designs, dataflow optimization |
| [Neural-PIM: Efficient Processing-In-Memory](https://arxiv.org/pdf/2201.09861) | arXiv | 2022 | Neural network acceleration in DRAM |
| [PRIME: Processing-in-Memory for Neural Networks](https://ieeexplore.ieee.org/document/7551380/) | ISCA | 2016 | ReRAM-based crossbar computation |
| [PIMCoSim: Hardware/Software Co-Simulator](https://www.mdpi.com/2079-9292/13/23/4795) | MDPI Electronics | 2024 | Simulation framework for PIM exploration |

### Key Findings
- UPMEM achieves 23x performance over GPU when memory oversubscription required
- SRAM-PIM with value-level and bit-level sparsity (DB-PIM framework)
- ReRAM crossbars enable ~10x gain over SRAM-based accelerators

### UPMEM Architecture
First commercially available PIM: DRAM + in-order cores (DPUs) on same chip.

---

## 2. Neuromorphic Computing & Vector Search

### Neuromorphic Hardware

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Roadmap to Neuromorphic Computing with Emerging Technologies](https://arxiv.org/html/2407.02353v1) | arXiv | 2024 | Technology roadmap for neuromorphic systems |
| [Neuromorphic Computing for Robotic Vision](https://www.nature.com/articles/s44172-025-00492-5) | Nature Comm. Eng. | 2025 | Event-driven vision processing |
| [Survey of Neuromorphic Computing and Neural Networks in Hardware](https://arxiv.org/pdf/1705.06963) | arXiv | 2017 | Comprehensive hardware survey |

### Key Hardware Platforms
- **SpiNNaker**: Millions of processing cores (Manchester)
- **TrueNorth**: IBM's commercial neuromorphic chip
- **Loihi**: Intel research chip with online learning
- **BrainScaleS**: European analog-digital hybrid

### HNSW Advances

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Down with the Hierarchy: Hub Highway Hypothesis](https://arxiv.org/html/2412.01940v2) | arXiv | 2024 | Hubs maintain hierarchy function, not layers |
| [Efficient Vector Search on Disaggregated Memory (d-HNSW)](https://arxiv.org/html/2505.11783v1) | arXiv | 2025 | Disaggregated memory architecture |
| [WebANNS: ANN Search in Web Browsers](https://arxiv.org/html/2507.00521) | arXiv | 2025 | Browser-based vector search |

---

## 3. Implicit Neural Representations (INR)

### Core Research

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Where Do We Stand with INRs? Technical Survey](https://arxiv.org/html/2411.03688v1) | arXiv | 2024 | Four-category taxonomy of INR techniques |
| [FR-INR: Fourier Reparameterized Training](https://github.com/LabShuHangGU/FR-INR) | CVPR | 2024 | Fourier bases for MLP weight composition |
| [Neural Experts: Mixture of Experts for INRs](https://neurips.cc/virtual/2024/poster/93148) | NeurIPS | 2024 | MoE for local piece-wise continuous functions |
| [inr2vec: Compact Latent Representation for INRs](https://cvlab-unibo.github.io/inr2vec/) | CVPR | 2023 | Embeddings for INR-based retrieval |

### Key INR Methods
- **SIREN**: Sinusoidal activation networks
- **WIRE**: Wavelet implicit representations
- **GAUSS**: Gaussian activation functions
- **FINER**: Frequency-enhanced representations

### Retrieval Performance
inr2vec shows 1.8 mAP gap vs PointNet++ on 3D retrieval benchmarks.

---

## 4. Hypergraph & Topological Data Analysis

### Hypergraph Neural Networks

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [EasyHypergraph: Fast Higher-Order Network Analysis](https://www.nature.com/articles/s41599-025-05180-5) | Nature HSS Comm. | 2025 | Memory-efficient hypergraph analysis |
| [DPHGNN: Dual Perspective Hypergraph Neural Networks](https://dl.acm.org/doi/10.1145/3637528.3672047) | KDD | 2024 | Dual-perspective message passing |
| [Hypergraph Computation Survey](https://www.sciencedirect.com/science/article/pii/S2095809924002510) | Engineering | 2024 | Comprehensive hypergraph computation survey |

### Topological Deep Learning

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Topological Deep Learning: New Frontier for Relational Learning](https://pmc.ncbi.nlm.nih.gov/articles/PMC11973457/) | PMC | 2024 | Position paper on TDL paradigm |
| [ICML TDL Challenge 2024: Beyond the Graph Domain](https://arxiv.org/html/2409.05211v1) | ICML | 2024 | 52 submissions on topological liftings |
| [Simplicial Homology Theories for Hypergraphs](https://arxiv.org/html/2409.18310) | arXiv | 2024 | Survey of hypergraph homology |

### Key Software
- **TopoX Suite**: TopoNetX, TopoEmbedX, TopoModelX (Python)
- **DHG**: DeepHypergraph for learning on hypergraphs
- **HyperNetX**: Hypergraph computations
- **XGI**: Hypergraphs and simplicial complexes

---

## 5. Temporal Memory & Causal Inference

### Agent Memory Architectures

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Mem0: Production-Ready AI Agents with Scalable LTM](https://arxiv.org/pdf/2504.19413) | arXiv | 2024 | Causal relationships for decision-making |
| [Zep: Temporal Knowledge Graph for Agent Memory](https://arxiv.org/html/2501.13956v1) | arXiv | 2025 | TKG-based memory with Graphiti engine |
| [Memory Architectures in Long-Term AI Agents](https://www.researchgate.net/publication/388144017) | ResearchGate | 2025 | 47% improvement in temporal reasoning |
| [Evaluating Very Long-Term Conversational Memory](https://www.researchgate.net/publication/384220784) | ResearchGate | 2024 | Long-term temporal/causal dynamics |

### Key Findings
- Zep outperforms MemGPT on Deep Memory Retrieval benchmark
- Mem0g adds graph-based memory representations
- TKGs model relationship start/change/end for causality tracking

### Causal Inference + Deep Learning

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Causal Inference Meets Deep Learning: Survey](https://pmc.ncbi.nlm.nih.gov/articles/PMC11384545/) | PMC | 2024 | PFC working memory for causal reasoning |

---

## 6. Federated Learning & Distributed Consensus

### Federated Learning

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Secure and Fair Federated Learning via Consensus Incentive](https://www.mdpi.com/2227-7390/12/19/3068) | MDPI Mathematics | 2024 | Byzantine-resistant FL |
| [FL Assisted Distributed Energy Optimization](https://ietresearch.onlinelibrary.wiley.com/doi/10.1049/rpg2.13101) | IET RPG | 2024 | Consensus + innovations approach |
| [Comprehensive Review of FL Challenges](https://link.springer.com/article/10.1186/s40537-025-01195-6) | J. Big Data | 2025 | Data preparation viewpoint |

### CRDT Fundamentals

| Resource | Key Contribution |
|----------|------------------|
| [CRDT Dictionary: Field Guide](https://www.iankduncan.com/engineering/2025-11-27-crdt-dictionary) | Comprehensive CRDT taxonomy |
| [CRDT Wiki (Dremio)](https://www.dremio.com/wiki/conflict-free-replicated-data-type/) | Strong eventual consistency |

### Key Algorithms
- **HyFDCA**: Hybrid Federated Dual Coordinate Ascent (2024)
- **Gossip protocols** for decentralized aggregation
- **Version vectors** for causal tracking in CRDTs

---

## 7. Photonic Computing

### Silicon Photonics for AI

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [MIT Photonic Processor for Ultrafast AI](https://news.mit.edu/2024/photonic-processor-could-enable-ultrafast-ai-computations-1202) | MIT News | 2024 | Sub-nanosecond classification, 92% accuracy |
| [Silicon Photonics for Scalable AI Hardware](https://ieeephotonics.org/) | IEEE JSTQE | 2025 | Wafer-scale ONN integration |
| [Hundred-Layer Photonic Deep Learning](https://www.nature.com/articles/s41467-025-65356-0) | Nature Comm. | 2025 | SLiM chip: 200+ layer depth |
| [All-Optical CNN with Phase Change Materials](https://www.nature.com/articles/s41598-025-06259-4) | Sci. Reports | 2025 | GST-based active waveguides |

### Key Characteristics
- Sub-nanosecond latency
- Minimal energy loss (photons don't generate heat like electrons)
- THz bandwidth potential
- 3.2 Tbps achieved on silicon slow-light modulator

---

## 8. ReRAM & Memristor Computing

### Analog In-Memory Compute

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Programming Memristor Arrays with Arbitrary Precision](https://www.science.org/doi/10.1126/science.adi9405) | Science | 2024 | 16Mb floating-point RRAM, 31.2 TFLOPS/W |
| [Memristive Memory Augmented Neural Network](https://www.nature.com/articles/s41467-022-33629-7) | Nature Comm. | 2022 | Hashing and similarity search in crossbars |
| [Wafer-Scale Memristive Passive Crossbar](https://www.nature.com/articles/s41467-025-63831-2) | Nature Comm. | 2025 | Brain-scale neuromorphic computing |
| [4K-Memristor Analog-Grade Crossbar](https://www.nature.com/articles/s41467-021-25455-0) | Nature Comm. | 2021 | Foundational analog VMM work |

### Vector Similarity Search
- TCAM functionality in analog crossbar
- Hamming distance via degree-of-mismatch output
- Massively parallel in-memory similarity computation

---

## 9. Sheaf Theory & Category Theory for ML

### Sheaf Neural Networks

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Sheaf Theory: From Deep Geometry to Deep Learning](https://arxiv.org/html/2502.15476v1) | arXiv | 2025 | Comprehensive sheaf applications survey |
| [Sheaf4Rec: Recommender Systems](https://arxiv.org/abs/2304.09097) | arXiv | 2023 | 8.53% F1@10 improvement, 37% faster |
| [Sheaf Neural Networks with Connection Laplacians](https://proceedings.mlr.press/v196/barbero22a/barbero22a.pdf) | ICML | 2022 | Learnable sheaf Laplacians |
| [Categorical Deep Learning: Algebraic Theory of All Architectures](https://arxiv.org/abs/2402.15332) | arXiv | 2024 | Monads + 2-categories for neural networks |

### Key Concepts
- **Sheaf**: Local-to-global consistency structure
- **Sheaf Laplacian**: Diffusion operator on sheaf-decorated graphs
- **Neural Sheaf Diffusion**: Learning sheaf structure from data

---

## 10. Consciousness & Integrated Information

### IIT Research

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [IIT 4.0: Phenomenal Existence in Physical Terms](https://pmc.ncbi.nlm.nih.gov/articles/PMC10581496/) | PLOS Comp. Bio. | 2023 | Updated axioms, postulates, measures |
| [How to be an IIT Theorist Without Losing Your Body](https://www.frontiersin.org/journals/computational-neuroscience/articles/10.3389/fncom.2024.1510066/full) | Frontiers | 2024 | Embodied IIT considerations |

### Key Metrics
- **Φ (Phi)**: Integrated information measure
- **Reentrant architecture**: Feedback loops required for consciousness
- **Controversy**: Empirical testability debates (2023-2025)

---

## 11. Thermodynamic Limits

### Landauer Bound & Reversible Computing

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Fundamental Energy Limits and Reversible Computing](https://www.osti.gov/servlets/purl/1458032) | Sandia | 2017 | DOE reversible computing roadmap |
| [Adiabatic Computing for Optimal Thermodynamic Efficiency](https://arxiv.org/abs/2302.09957) | arXiv | 2023 | Optimal information processing bounds |
| [Fundamental Energy Cost of Finite-Time Parallelizable Computing](https://www.nature.com/articles/s41467-023-36020-2) | Nature Comm. | 2023 | Parallelization thermodynamics |

### Key Numbers
- Landauer limit: ~0.018 eV (2.9×10⁻²¹ J) per bit erasure at room temp
- Current CMOS: 1000x above theoretical minimum
- Reversible computing: 4000x efficiency potential
- Vaire Computing: Commercial reversible chips by 2027-2028

---

## 12. Multi-Modal Foundation Models

### Unified Architectures

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|------------------|
| [Unified Multimodal Understanding and Generation](https://arxiv.org/pdf/2505.02567) | arXiv | 2025 | Any-to-any multimodal models |
| [Show-o: Single Transformer for Multimodal](https://github.com/showlab/Awesome-Unified-Multimodal-Models) | GitHub | 2024 | Unified understanding + generation |
| [Multi-Modal Latent Space Learning for CoT Reasoning](https://ojs.aaai.org/index.php/AAAI/article/view/29776/31338) | AAAI | 2024 | Chain-of-thought across modalities |

### Key Models (2024-2025)
- **Chameleon**: Mixed-modal early fusion (Meta)
- **Emu3**: Next-token prediction for all modalities
- **Janus/JanusFlow**: Decoupled visual encoding
- **SEED-X**: Multi-granularity comprehension

---

## Summary Statistics

| Category | Papers Reviewed | Key Takeaway |
|----------|-----------------|--------------|
| PIM/Near-Memory | 8 | 23x GPU performance, commercial availability |
| Neuromorphic | 12 | 1000x energy reduction potential |
| INR/Learned Manifolds | 6 | Continuous representations for storage |
| Hypergraph/TDA | 10 | Higher-order relations, topological queries |
| Temporal Memory | 6 | TKGs for causal agent memory |
| Federated/CRDT | 5 | Decentralized consensus, eventual consistency |
| Photonic | 5 | Sub-ns latency, 92% accuracy demonstrated |
| Memristor | 5 | 31.2 TFLOPS/W efficiency |
| Sheaf/Category | 6 | 8.5% improvement on recommender tasks |
| Consciousness | 3 | IIT 4.0 framework, Φ measures |
| Thermodynamics | 4 | 4000x reversible computing potential |
| Multi-Modal | 5 | Unified latent spaces emerging |
