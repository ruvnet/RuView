# SPARC Pseudocode: Ruvector-Scipix OCR & Math Recognition Pipeline

## Document Overview

This document provides algorithmic pseudocode for the core components of the ruvector-scipix OCR and mathematical expression recognition system. All algorithms use Rust-like syntax and include complexity analysis.

---

## 1. Image Preprocessing Pipeline

### 1.1 Main Preprocessing Algorithm

```
ALGORITHM: PreprocessImage
INPUT: imageBytes (Vec<u8>), config (PreprocessConfig)
OUTPUT: Result<ProcessedImage, Error>

CONSTANTS:
    MAX_IMAGE_SIZE = 4096 × 4096 pixels
    MIN_DPI = 150
    TARGET_DPI = 300
    NOISE_THRESHOLD = 0.15

DATA STRUCTURES:
    ProcessedImage {
        data: Vec<u8>,
        width: u32,
        height: u32,
        channels: u8,
        metadata: ImageMetadata,
        regions: Vec<TextRegion>
    }

    ImageMetadata {
        dpi: u32,
        rotation: f32,
        quality_score: f32,
        has_math: bool
    }

    TextRegion {
        bbox: BoundingBox,
        confidence: f32,
        region_type: RegionType  // Text, Math, Diagram
    }

BEGIN
    // Phase 1: Image Loading and Validation
    rawImage ← DecodeImage(imageBytes)
    IF rawImage.is_error() THEN
        RETURN Error("Failed to decode image")
    END IF

    IF rawImage.width > MAX_IMAGE_SIZE OR rawImage.height > MAX_IMAGE_SIZE THEN
        rawImage ← ResizeImage(rawImage, MAX_IMAGE_SIZE)
    END IF

    // Phase 2: Rotation Detection and Correction
    rotationAngle ← DetectRotation(rawImage)
    IF ABS(rotationAngle) > 0.5 THEN
        rawImage ← RotateImage(rawImage, -rotationAngle)
    END IF

    // Phase 3: DPI Normalization
    currentDPI ← EstimateDPI(rawImage)
    IF currentDPI < MIN_DPI THEN
        RETURN Error("Image resolution too low")
    END IF

    IF currentDPI != TARGET_DPI THEN
        scaleFactor ← TARGET_DPI / currentDPI
        rawImage ← ResizeImage(rawImage, scaleFactor)
    END IF

    // Phase 4: Noise Reduction
    noiseLevel ← EstimateNoise(rawImage)
    IF noiseLevel > NOISE_THRESHOLD THEN
        rawImage ← ApplyBilateralFilter(rawImage, sigma: 2.0, radius: 3)
    END IF

    // Phase 5: Contrast Enhancement
    enhancedImage ← AdaptiveHistogramEqualization(rawImage, clip_limit: 2.0)

    // Phase 6: Text Region Detection
    regions ← DetectTextRegions(enhancedImage)

    // Phase 7: Quality Assessment
    qualityScore ← AssessQuality(enhancedImage, regions)

    metadata ← ImageMetadata {
        dpi: TARGET_DPI,
        rotation: rotationAngle,
        quality_score: qualityScore,
        has_math: ContainsMathRegions(regions)
    }

    RETURN Ok(ProcessedImage {
        data: enhancedImage.to_bytes(),
        width: enhancedImage.width,
        height: enhancedImage.height,
        channels: enhancedImage.channels,
        metadata: metadata,
        regions: regions
    })
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - Image decoding: O(n) where n = pixel count
        - Rotation detection: O(n log n) using Hough transform
        - Image rotation: O(n)
        - DPI scaling: O(n)
        - Bilateral filter: O(n × r²) where r = radius
        - CLAHE: O(n)
        - Region detection: O(n log n)
        Total: O(n log n)

    Space Complexity:
        - Raw image buffer: O(n)
        - Intermediate buffers: O(n)
        - Region storage: O(k) where k = region count
        Total: O(n)
```

### 1.2 Rotation Detection Algorithm

```
ALGORITHM: DetectRotation
INPUT: image (Image)
OUTPUT: angle (f32)

BEGIN
    // Convert to grayscale if needed
    grayImage ← ToGrayscale(image)

    // Apply edge detection
    edges ← CannyEdgeDetection(grayImage, low: 50, high: 150)

    // Use Hough Line Transform to detect dominant lines
    lines ← HoughLineTransform(edges, rho: 1.0, theta: PI/180, threshold: 100)

    IF lines.is_empty() THEN
        RETURN 0.0
    END IF

    // Cluster angles into dominant orientations
    angles ← []
    FOR EACH line IN lines DO
        angle ← line.theta * 180 / PI
        // Normalize to [-45, 45] range
        WHILE angle > 45 DO
            angle ← angle - 90
        END WHILE
        WHILE angle < -45 DO
            angle ← angle + 90
        END WHILE
        angles.push(angle)
    END FOR

    // Use median for robustness
    angles.sort()
    medianAngle ← angles[angles.len() / 2]

    RETURN medianAngle
END

COMPLEXITY ANALYSIS:
    Time: O(n log n) for Hough transform
    Space: O(n) for edge map
```

### 1.3 Text Region Detection

```
ALGORITHM: DetectTextRegions
INPUT: image (Image)
OUTPUT: regions (Vec<TextRegion>)

DATA STRUCTURES:
    Component {
        pixels: Vec<Point>,
        bbox: BoundingBox,
        area: u32
    }

BEGIN
    // Use MSER (Maximally Stable Extremal Regions)
    binaryImage ← AdaptiveThreshold(image, window: 15)

    components ← FindConnectedComponents(binaryImage)

    regions ← []
    FOR EACH comp IN components DO
        // Filter by geometric properties
        aspectRatio ← comp.bbox.width / comp.bbox.height
        density ← comp.area / (comp.bbox.width * comp.bbox.height)

        IF aspectRatio > 0.1 AND aspectRatio < 10.0 AND density > 0.3 THEN
            // Classify region type
            features ← ExtractRegionFeatures(comp, image)
            regionType ← ClassifyRegion(features)

            region ← TextRegion {
                bbox: comp.bbox,
                confidence: features.confidence,
                region_type: regionType
            }
            regions.push(region)
        END IF
    END FOR

    // Merge nearby regions
    mergedRegions ← MergeOverlappingRegions(regions, iou_threshold: 0.3)

    RETURN mergedRegions
END

COMPLEXITY ANALYSIS:
    Time: O(n × α(n)) where α is inverse Ackermann (connected components)
    Space: O(k) where k = component count
```

---

## 2. OCR Engine Core

### 2.1 Main OCR Pipeline

```
ALGORITHM: RecognizeText
INPUT: image (ProcessedImage), model (VisionTransformer)
OUTPUT: Result<RecognitionResult, Error>

DATA STRUCTURES:
    RecognitionResult {
        lines: Vec<TextLine>,
        confidence: f32,
        processing_time_ms: u64
    }

    TextLine {
        text: String,
        bbox: BoundingBox,
        words: Vec<Word>,
        confidence: f32
    }

    Word {
        text: String,
        bbox: BoundingBox,
        chars: Vec<Character>,
        confidence: f32
    }

    Character {
        char: char,
        bbox: BoundingBox,
        confidence: f32,
        alternatives: Vec<(char, f32)>
    }

BEGIN
    startTime ← GetCurrentTime()

    // Phase 1: Vision Transformer Encoding
    encodedFeatures ← EncodeImageFeatures(image, model)

    // Phase 2: Text Line Detection
    textLines ← DetectTextLines(encodedFeatures, image.regions)

    // Phase 3: Character Recognition
    recognizedLines ← []
    totalConfidence ← 0.0

    FOR EACH lineRegion IN textLines DO
        lineImage ← CropRegion(image, lineRegion.bbox)

        // Run sequence-to-sequence recognition
        words ← RecognizeLineSequence(lineImage, model, encodedFeatures)

        lineText ← words.map(|w| w.text).join(" ")
        lineConfidence ← ComputeLineConfidence(words)

        textLine ← TextLine {
            text: lineText,
            bbox: lineRegion.bbox,
            words: words,
            confidence: lineConfidence
        }

        recognizedLines.push(textLine)
        totalConfidence ← totalConfidence + lineConfidence
    END FOR

    avgConfidence ← totalConfidence / recognizedLines.len()
    processingTime ← GetCurrentTime() - startTime

    RETURN Ok(RecognitionResult {
        lines: recognizedLines,
        confidence: avgConfidence,
        processing_time_ms: processingTime
    })
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - Vision Transformer encoding: O(n² × d) where d = embedding dim
        - Line detection: O(k × log k) where k = regions
        - Character recognition per line: O(m × d²) where m = line length
        - Total lines L: O(L × m × d²)
        Overall: O(n² × d + L × m × d²)

    Space Complexity:
        - Feature maps: O(n × d)
        - Attention maps: O(n² × h) where h = attention heads
        - Output storage: O(L × m)
        Total: O(n² × h + n × d)
```

### 2.2 Vision Transformer Encoding

```
ALGORITHM: EncodeImageFeatures
INPUT: image (ProcessedImage), model (VisionTransformer)
OUTPUT: features (FeatureMap)

DATA STRUCTURES:
    FeatureMap {
        embeddings: Tensor<f32>,  // Shape: [seq_len, embed_dim]
        attention_weights: Tensor<f32>,  // Shape: [heads, seq_len, seq_len]
        positions: Vec<Point>
    }

    VisionTransformer {
        patch_size: u32,
        embed_dim: u32,
        num_heads: u32,
        num_layers: u32,
        weights: ModelWeights
    }

BEGIN
    // Phase 1: Patch Extraction
    patchSize ← model.patch_size
    numPatchesH ← image.height / patchSize
    numPatchesW ← image.width / patchSize

    patches ← []
    positions ← []

    FOR h IN 0..numPatchesH DO
        FOR w IN 0..numPatchesW DO
            y ← h * patchSize
            x ← w * patchSize
            patch ← ExtractPatch(image, x, y, patchSize)
            patches.push(patch)
            positions.push(Point{x, y})
        END FOR
    END FOR

    // Phase 2: Patch Embedding
    embeddings ← []
    FOR EACH patch IN patches DO
        // Linear projection of flattened patch
        flatPatch ← Flatten(patch)
        embedding ← MatMul(model.weights.patch_projection, flatPatch)
        embeddings.push(embedding)
    END FOR

    // Phase 3: Positional Encoding
    FOR i IN 0..embeddings.len() DO
        posEncoding ← ComputePositionalEncoding(i, model.embed_dim)
        embeddings[i] ← embeddings[i] + posEncoding
    END FOR

    // Add [CLS] token
    clsToken ← model.weights.cls_token
    embeddings.insert(0, clsToken)

    // Phase 4: Transformer Layers
    x ← Tensor::from(embeddings)
    allAttentionWeights ← []

    FOR layer IN 0..model.num_layers DO
        // Multi-head self-attention
        (x, attentionWeights) ← MultiHeadAttention(
            x,
            model.weights.layers[layer],
            num_heads: model.num_heads
        )

        allAttentionWeights.push(attentionWeights)

        // Feed-forward network
        x ← FeedForward(x, model.weights.layers[layer])

        // Layer normalization
        x ← LayerNorm(x, model.weights.layers[layer])
    END FOR

    RETURN FeatureMap {
        embeddings: x,
        attention_weights: Stack(allAttentionWeights),
        positions: positions
    }
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - Patch extraction: O(n) where n = pixels
        - Patch embedding: O(p × d²) where p = patches, d = embed_dim
        - Attention per layer: O(p² × d)
        - Total layers L: O(L × p² × d)
        Overall: O(L × p² × d)

    Space Complexity:
        - Embeddings: O(p × d)
        - Attention matrices: O(L × h × p²) where h = heads
        Total: O(L × h × p² + p × d)
```

### 2.3 Character Recognition Sequence

```
ALGORITHM: RecognizeLineSequence
INPUT: lineImage (Image), model (VisionTransformer), features (FeatureMap)
OUTPUT: words (Vec<Word>)

DATA STRUCTURES:
    BeamSearchState {
        sequence: Vec<char>,
        score: f32,
        hidden_state: Tensor<f32>
    }

CONSTANTS:
    BEAM_WIDTH = 5
    MAX_SEQUENCE_LENGTH = 256
    END_TOKEN = '<END>'
    SPACE_TOKEN = '<SPACE>'

BEGIN
    // Initialize beam search
    initialState ← BeamSearchState {
        sequence: [],
        score: 0.0,
        hidden_state: features.embeddings[0]  // CLS token
    }

    beams ← [initialState]

    // Beam search decoding
    FOR step IN 0..MAX_SEQUENCE_LENGTH DO
        candidates ← []

        FOR EACH beam IN beams DO
            IF beam.sequence.last() == END_TOKEN THEN
                candidates.push(beam)
                CONTINUE
            END IF

            // Get character probabilities from model
            (logits, newHiddenState) ← model.decode_step(
                beam.hidden_state,
                features.embeddings
            )

            probabilities ← Softmax(logits)

            // Get top-k characters
            topK ← GetTopK(probabilities, k: BEAM_WIDTH)

            FOR EACH (char, prob) IN topK DO
                newSequence ← beam.sequence.clone()
                newSequence.push(char)

                // Log probability for numerical stability
                newScore ← beam.score + LOG(prob)

                newBeam ← BeamSearchState {
                    sequence: newSequence,
                    score: newScore,
                    hidden_state: newHiddenState
                }

                candidates.push(newBeam)
            END FOR
        END FOR

        // Keep top BEAM_WIDTH candidates
        candidates.sort_by(|a, b| b.score.cmp(a.score))
        beams ← candidates[0..BEAM_WIDTH]

        // Check if all beams ended
        allEnded ← beams.all(|b| b.sequence.last() == END_TOKEN)
        IF allEnded THEN
            BREAK
        END IF
    END FOR

    // Take best beam
    bestBeam ← beams[0]

    // Split sequence into words
    words ← []
    currentWord ← []
    currentBBox ← BoundingBox::new()

    FOR i IN 0..bestBeam.sequence.len() DO
        char ← bestBeam.sequence[i]

        IF char == SPACE_TOKEN OR char == END_TOKEN THEN
            IF NOT currentWord.is_empty() THEN
                wordText ← currentWord.join("")
                word ← Word {
                    text: wordText,
                    bbox: currentBBox,
                    chars: currentWord.clone(),
                    confidence: EXP(bestBeam.score / bestBeam.sequence.len())
                }
                words.push(word)
                currentWord.clear()
            END IF
        ELSE
            currentWord.push(Character {
                char: char,
                bbox: EstimateCharBBox(lineImage, i),
                confidence: EXP(bestBeam.score / (i + 1)),
                alternatives: []
            })
        END IF
    END FOR

    RETURN words
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - Beam search steps: O(T × B × V) where:
            T = max sequence length
            B = beam width
            V = vocabulary size
        - Sorting per step: O(B × V × log(B × V))
        Overall: O(T × B × V × log(B × V))

    Space Complexity:
        - Beam storage: O(B × T × d) where d = hidden dim
        - Candidate buffer: O(B × V)
        Total: O(B × T × d)
```

---

## 3. Mathematical Expression Parser

### 3.1 Math Expression Recognition

```
ALGORITHM: RecognizeMathExpression
INPUT: region (TextRegion), image (ProcessedImage), model (MathModel)
OUTPUT: Result<MathExpression, Error>

DATA STRUCTURES:
    MathExpression {
        latex: String,
        tree: ExpressionTree,
        symbols: Vec<MathSymbol>,
        confidence: f32
    }

    ExpressionTree {
        root: Box<TreeNode>,
        height: u32
    }

    TreeNode {
        symbol: MathSymbol,
        relationship: SpatialRelation,
        children: Vec<Box<TreeNode>>
    }

    MathSymbol {
        symbol_type: SymbolType,  // Digit, Operator, Letter, Special
        value: String,
        bbox: BoundingBox,
        confidence: f32
    }

    SpatialRelation {
        relation_type: RelationType,  // Above, Below, Right, Superscript, Subscript
        distance: f32,
        alignment: f32
    }

BEGIN
    // Phase 1: Extract math region
    mathImage ← CropRegion(image, region.bbox)

    // Phase 2: Symbol Detection and Classification
    symbols ← DetectMathSymbols(mathImage, model)

    IF symbols.is_empty() THEN
        RETURN Error("No mathematical symbols detected")
    END IF

    // Phase 3: Spatial Relationship Analysis
    relationships ← AnalyzeSpatialRelationships(symbols)

    // Phase 4: Expression Tree Construction
    tree ← BuildExpressionTree(symbols, relationships)

    // Phase 5: LaTeX Generation
    latex ← GenerateLaTeX(tree)

    // Calculate overall confidence
    avgConfidence ← symbols.map(|s| s.confidence).average()

    RETURN Ok(MathExpression {
        latex: latex,
        tree: tree,
        symbols: symbols,
        confidence: avgConfidence
    })
END

COMPLEXITY ANALYSIS:
    Time: O(n² × log n) where n = symbol count
    Space: O(n × h) where h = tree height
```

### 3.2 Symbol Detection and Classification

```
ALGORITHM: DetectMathSymbols
INPUT: mathImage (Image), model (MathModel)
OUTPUT: symbols (Vec<MathSymbol>)

CONSTANTS:
    SYMBOL_MIN_SIZE = 8 pixels
    SYMBOL_MAX_SIZE = 128 pixels
    CONFIDENCE_THRESHOLD = 0.7

BEGIN
    // Phase 1: Connected Component Analysis
    binaryImage ← AdaptiveThreshold(mathImage, window: 11)
    components ← FindConnectedComponents(binaryImage)

    symbols ← []

    FOR EACH comp IN components DO
        // Filter by size
        width ← comp.bbox.width
        height ← comp.bbox.height

        IF width < SYMBOL_MIN_SIZE OR height < SYMBOL_MIN_SIZE THEN
            CONTINUE
        END IF

        IF width > SYMBOL_MAX_SIZE OR height > SYMBOL_MAX_SIZE THEN
            // Might be compound symbol, try to split
            subComponents ← SplitComponent(comp)
            FOR EACH subComp IN subComponents DO
                ProcessSymbol(subComp, mathImage, model, symbols)
            END FOR
        ELSE
            ProcessSymbol(comp, mathImage, model, symbols)
        END IF
    END FOR

    // Sort symbols left-to-right, top-to-bottom
    symbols.sort_by(|a, b| {
        IF ABS(a.bbox.y - b.bbox.y) < 10 THEN
            a.bbox.x.cmp(b.bbox.x)
        ELSE
            a.bbox.y.cmp(b.bbox.y)
        END IF
    })

    RETURN symbols
END

SUBROUTINE: ProcessSymbol
INPUT: component (Component), image (Image), model (MathModel), symbols (Vec<MathSymbol>)
OUTPUT: None (modifies symbols)

BEGIN
    // Extract symbol image
    symbolImage ← CropRegion(image, component.bbox)

    // Normalize to model input size
    normalizedSymbol ← ResizeImage(symbolImage, 64, 64)

    // Classify symbol
    (symbolClass, confidence) ← model.classify_symbol(normalizedSymbol)

    IF confidence >= CONFIDENCE_THRESHOLD THEN
        symbol ← MathSymbol {
            symbol_type: DetermineSymbolType(symbolClass),
            value: symbolClass.to_string(),
            bbox: component.bbox,
            confidence: confidence
        }

        symbols.push(symbol)
    END IF
END

COMPLEXITY ANALYSIS:
    Time: O(n × c) where n = components, c = classification time
    Space: O(n) for symbol storage
```

### 3.3 Spatial Relationship Analysis

```
ALGORITHM: AnalyzeSpatialRelationships
INPUT: symbols (Vec<MathSymbol>)
OUTPUT: relationships (Vec<(usize, usize, SpatialRelation)>)

DATA STRUCTURES:
    RelationFeatures {
        horizontal_distance: f32,
        vertical_distance: f32,
        size_ratio: f32,
        vertical_alignment: f32,
        horizontal_alignment: f32
    }

CONSTANTS:
    SUPERSCRIPT_Y_THRESHOLD = 0.6  // Relative to symbol height
    SUBSCRIPT_Y_THRESHOLD = 0.4
    FRACTION_ALIGNMENT_THRESHOLD = 0.8

BEGIN
    relationships ← []

    // Build spatial index for efficient queries
    spatialIndex ← BuildQuadTree(symbols)

    FOR i IN 0..symbols.len() DO
        symbolA ← symbols[i]

        // Find nearby symbols
        nearbySymbols ← spatialIndex.query_radius(
            symbolA.bbox.center(),
            radius: symbolA.bbox.width * 3
        )

        FOR EACH (j, symbolB) IN nearbySymbols DO
            IF i >= j THEN
                CONTINUE  // Avoid duplicate pairs
            END IF

            // Extract relationship features
            features ← ExtractRelationFeatures(symbolA, symbolB)

            // Classify relationship
            relation ← ClassifyRelation(features, symbolA, symbolB)

            IF relation.is_some() THEN
                relationships.push((i, j, relation.unwrap()))
            END IF
        END FOR
    END FOR

    RETURN relationships
END

SUBROUTINE: ClassifyRelation
INPUT: features (RelationFeatures), symbolA (MathSymbol), symbolB (MathSymbol)
OUTPUT: Option<SpatialRelation>

BEGIN
    centerA ← symbolA.bbox.center()
    centerB ← symbolB.bbox.center()

    deltaX ← centerB.x - centerA.x
    deltaY ← centerB.y - centerA.y

    // Determine dominant relationship

    // Superscript/Subscript detection
    IF deltaX > 0 AND deltaX < symbolA.bbox.width * 1.5 THEN
        relativeY ← deltaY / symbolA.bbox.height

        IF relativeY < -SUPERSCRIPT_Y_THRESHOLD THEN
            RETURN Some(SpatialRelation {
                relation_type: Superscript,
                distance: SQRT(deltaX² + deltaY²),
                alignment: features.horizontal_alignment
            })
        ELSE IF relativeY > SUBSCRIPT_Y_THRESHOLD THEN
            RETURN Some(SpatialRelation {
                relation_type: Subscript,
                distance: SQRT(deltaX² + deltaY²),
                alignment: features.horizontal_alignment
            })
        END IF
    END IF

    // Fraction detection (vertical alignment)
    IF features.vertical_alignment > FRACTION_ALIGNMENT_THRESHOLD THEN
        IF deltaY < 0 THEN
            RETURN Some(SpatialRelation {
                relation_type: Above,
                distance: ABS(deltaY),
                alignment: features.vertical_alignment
            })
        ELSE IF deltaY > 0 THEN
            RETURN Some(SpatialRelation {
                relation_type: Below,
                distance: ABS(deltaY),
                alignment: features.vertical_alignment
            })
        END IF
    END IF

    // Horizontal sequence (default)
    IF deltaX > 0 AND ABS(deltaY) < symbolA.bbox.height * 0.3 THEN
        RETURN Some(SpatialRelation {
            relation_type: Right,
            distance: deltaX,
            alignment: features.horizontal_alignment
        })
    END IF

    RETURN None
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - QuadTree construction: O(n log n)
        - For each symbol, query nearby: O(log n + k) where k = nearby count
        - Total: O(n × (log n + k))
        Average case: O(n log n) if k is constant

    Space Complexity:
        - QuadTree: O(n)
        - Relationships: O(n²) worst case, O(n) average
        Total: O(n²) worst case
```

### 3.4 Expression Tree Construction

```
ALGORITHM: BuildExpressionTree
INPUT: symbols (Vec<MathSymbol>), relationships (Vec<(usize, usize, SpatialRelation)>)
OUTPUT: tree (ExpressionTree)

DATA STRUCTURES:
    TreeBuilder {
        nodes: Vec<Box<TreeNode>>,
        parent_map: HashMap<usize, usize>,
        relation_graph: AdjacencyList
    }

BEGIN
    // Phase 1: Build relationship graph
    graph ← BuildRelationGraph(symbols, relationships)

    // Phase 2: Identify root candidates (symbols with no parents)
    rootCandidates ← []
    FOR i IN 0..symbols.len() DO
        IF NOT HasIncomingEdge(graph, i, excludeRight: true) THEN
            rootCandidates.push(i)
        END IF
    END FOR

    // Phase 3: Build tree from leftmost root
    rootCandidates.sort_by(|a, b| {
        symbols[*a].bbox.x.cmp(&symbols[*b].bbox.x)
    })

    rootIdx ← rootCandidates[0]

    // Phase 4: Recursive tree construction
    root ← BuildSubtree(rootIdx, symbols, graph, visited: Set::new())

    // Phase 5: Calculate tree height
    height ← CalculateHeight(root)

    RETURN ExpressionTree {
        root: root,
        height: height
    }
END

SUBROUTINE: BuildSubtree
INPUT: nodeIdx (usize), symbols (Vec<MathSymbol>), graph (AdjacencyList), visited (Set<usize>)
OUTPUT: node (Box<TreeNode>)

BEGIN
    IF visited.contains(nodeIdx) THEN
        RETURN Error("Cycle detected in expression tree")
    END IF

    visited.insert(nodeIdx)

    symbol ← symbols[nodeIdx]
    children ← []

    // Get all outgoing edges sorted by relationship priority
    edges ← graph.get_outgoing(nodeIdx)
    edges.sort_by(|a, b| {
        // Priority: Superscript > Subscript > Above > Below > Right
        GetRelationPriority(a.relation).cmp(GetRelationPriority(b.relation))
    })

    FOR EACH edge IN edges DO
        IF NOT visited.contains(edge.target) THEN
            childNode ← BuildSubtree(edge.target, symbols, graph, visited)
            childNode.relationship ← edge.relation
            children.push(childNode)
        END IF
    END FOR

    node ← TreeNode {
        symbol: symbol.clone(),
        relationship: SpatialRelation::default(),
        children: children
    }

    RETURN Box::new(node)
END

COMPLEXITY ANALYSIS:
    Time: O(n × log n) for graph construction and tree building
    Space: O(n × h) where h = average tree height
```

### 3.5 LaTeX Generation

```
ALGORITHM: GenerateLaTeX
INPUT: tree (ExpressionTree)
OUTPUT: latex (String)

BEGIN
    latex ← RecursiveGenerateLaTeX(tree.root)

    // Wrap in delimiters
    latex ← "\\(" + latex + "\\)"

    RETURN latex
END

SUBROUTINE: RecursiveGenerateLaTeX
INPUT: node (Box<TreeNode>)
OUTPUT: latex (String)

BEGIN
    symbol ← node.symbol
    baseLatex ← SymbolToLatex(symbol)

    // Group children by relationship type
    superscripts ← []
    subscripts ← []
    numerator ← None
    denominator ← None
    rightChildren ← []

    FOR EACH child IN node.children DO
        MATCH child.relationship.relation_type:
            Superscript → superscripts.push(child)
            Subscript → subscripts.push(child)
            Above → numerator ← Some(child)
            Below → denominator ← Some(child)
            Right → rightChildren.push(child)
        END MATCH
    END FOR

    // Build LaTeX string
    result ← baseLatex

    // Handle fractions
    IF numerator.is_some() AND denominator.is_some() THEN
        numLatex ← RecursiveGenerateLaTeX(numerator.unwrap())
        denomLatex ← RecursiveGenerateLaTeX(denominator.unwrap())
        result ← "\\frac{" + numLatex + "}{" + denomLatex + "}"
    END IF

    // Handle superscripts
    IF NOT superscripts.is_empty() THEN
        superLatex ← superscripts
            .map(|c| RecursiveGenerateLaTeX(c))
            .join("")
        result ← result + "^{" + superLatex + "}"
    END IF

    // Handle subscripts
    IF NOT subscripts.is_empty() THEN
        subLatex ← subscripts
            .map(|c| RecursiveGenerateLaTeX(c))
            .join("")
        result ← result + "_{" + subLatex + "}"
    END IF

    // Handle right children (sequential)
    FOR EACH child IN rightChildren DO
        childLatex ← RecursiveGenerateLaTeX(child)

        // Add spacing for operators
        IF IsOperator(child.symbol) THEN
            result ← result + " " + childLatex + " "
        ELSE
            result ← result + childLatex
        END IF
    END FOR

    RETURN result
END

SUBROUTINE: SymbolToLatex
INPUT: symbol (MathSymbol)
OUTPUT: latex (String)

BEGIN
    MATCH symbol.symbol_type:
        Digit → RETURN symbol.value
        Letter → RETURN symbol.value
        Operator → RETURN OperatorToLatex(symbol.value)
        Special → RETURN SpecialToLatex(symbol.value)
    END MATCH

    RETURN symbol.value
END

COMPLEXITY ANALYSIS:
    Time: O(n) where n = nodes in tree
    Space: O(h) for recursion stack where h = tree height
```

---

## 4. Output Format Conversion

### 4.1 Multi-Format Generation

```
ALGORITHM: ConvertToFormats
INPUT: mathExpr (MathExpression), formats (Vec<OutputFormat>)
OUTPUT: Result<HashMap<OutputFormat, String>, Error>

DATA STRUCTURES:
    OutputFormat {
        MMD,      // Markdown with delimiters
        LaTeXStyled,  // Standalone LaTeX
        MathML,   // MathML XML
        HTML      // Rendered HTML
    }

BEGIN
    results ← HashMap::new()

    FOR EACH format IN formats DO
        output ← MATCH format:
            MMD → GenerateMMD(mathExpr)
            LaTeXStyled → GenerateStyledLaTeX(mathExpr)
            MathML → GenerateMathML(mathExpr.tree)
            HTML → GenerateHTML(mathExpr)
        END MATCH

        results.insert(format, output)
    END FOR

    RETURN Ok(results)
END

COMPLEXITY ANALYSIS:
    Time: O(f × n) where f = format count, n = expression size
    Space: O(f × n) for storing all formats
```

### 4.2 MMD Generation

```
ALGORITHM: GenerateMMD
INPUT: mathExpr (MathExpression)
OUTPUT: mmd (String)

CONSTANTS:
    INLINE_DELIMITER = "$"
    DISPLAY_DELIMITER = "$$"

BEGIN
    latex ← mathExpr.latex

    // Determine if expression should be display or inline
    isDisplayMath ← ShouldBeDisplayMath(mathExpr)

    IF isDisplayMath THEN
        mmd ← DISPLAY_DELIMITER + "\n" + latex + "\n" + DISPLAY_DELIMITER
    ELSE
        mmd ← INLINE_DELIMITER + latex + INLINE_DELIMITER
    END IF

    RETURN mmd
END

SUBROUTINE: ShouldBeDisplayMath
INPUT: mathExpr (MathExpression)
OUTPUT: isDisplay (bool)

BEGIN
    // Display math if:
    // 1. Contains fractions or large operators
    // 2. Tree height > 2
    // 3. Width > threshold

    hasFractions ← mathExpr.latex.contains("\\frac")
    hasLargeOps ← mathExpr.latex.contains("\\sum") OR
                   mathExpr.latex.contains("\\int") OR
                   mathExpr.latex.contains("\\prod")

    isTall ← mathExpr.tree.height > 2
    isWide ← mathExpr.symbols.len() > 10

    RETURN hasFractions OR hasLargeOps OR isTall OR isWide
END

COMPLEXITY ANALYSIS:
    Time: O(n) where n = LaTeX string length
    Space: O(n) for output string
```

### 4.3 MathML Generation

```
ALGORITHM: GenerateMathML
INPUT: tree (ExpressionTree)
OUTPUT: mathml (String)

BEGIN
    xml ← XMLBuilder::new()
    xml.start_element("math", [("xmlns", "http://www.w3.org/1998/Math/MathML")])

    RecursiveGenerateMathML(tree.root, xml)

    xml.end_element("math")

    RETURN xml.to_string()
END

SUBROUTINE: RecursiveGenerateMathML
INPUT: node (Box<TreeNode>), xml (XMLBuilder)
OUTPUT: None (modifies xml)

BEGIN
    symbol ← node.symbol

    // Determine MathML element type
    MATCH symbol.symbol_type:
        Digit OR Letter →
            xml.element("mi", symbol.value)

        Operator →
            xml.element("mo", symbol.value)

        Special →
            HandleSpecialSymbol(symbol, xml)
    END MATCH

    // Handle relationships
    IF HasSuperscript(node) THEN
        xml.start_element("msup")
        RecursiveGenerateMathML(GetBase(node), xml)
        RecursiveGenerateMathML(GetSuperscript(node), xml)
        xml.end_element("msup")
    ELSE IF HasSubscript(node) THEN
        xml.start_element("msub")
        RecursiveGenerateMathML(GetBase(node), xml)
        RecursiveGenerateMathML(GetSubscript(node), xml)
        xml.end_element("msub")
    ELSE IF HasFraction(node) THEN
        xml.start_element("mfrac")
        RecursiveGenerateMathML(GetNumerator(node), xml)
        RecursiveGenerateMathML(GetDenominator(node), xml)
        xml.end_element("mfrac")
    END IF

    // Process right children
    FOR EACH child IN GetRightChildren(node) DO
        RecursiveGenerateMathML(child, xml)
    END FOR
END

COMPLEXITY ANALYSIS:
    Time: O(n) tree traversal
    Space: O(n) for XML string
```

### 4.4 HTML Rendering

```
ALGORITHM: GenerateHTML
INPUT: mathExpr (MathExpression)
OUTPUT: html (String)

BEGIN
    // Use KaTeX or MathJax for rendering
    latex ← mathExpr.latex

    html ← """
    <div class="math-expression" data-confidence="{mathExpr.confidence}">
        <script type="math/tex">
        {latex}
        </script>
    </div>
    """

    // Add accessibility attributes
    html ← AddAriaLabels(html, mathExpr)

    RETURN html
END

COMPLEXITY ANALYSIS:
    Time: O(n) string concatenation
    Space: O(n) output size
```

---

## 5. Batch Processing

### 5.1 Parallel Batch Processing

```
ALGORITHM: ProcessBatch
INPUT: inputs (Vec<InputSource>), config (ProcessConfig)
OUTPUT: Result<Vec<ProcessResult>, Error>

DATA STRUCTURES:
    InputSource {
        source_type: SourceType,  // Image, PDF, Directory
        path: PathBuf,
        page_range: Option<Range<u32>>
    }

    ProcessResult {
        input: InputSource,
        output: RecognitionResult,
        processing_time_ms: u64,
        status: ResultStatus
    }

    ProcessConfig {
        max_parallel: usize,
        timeout_ms: u64,
        cache_enabled: bool
    }

BEGIN
    // Phase 1: Expand inputs (handle PDFs and directories)
    expandedInputs ← []
    FOR EACH input IN inputs DO
        MATCH input.source_type:
            PDF →
                pages ← ExtractPDFPages(input.path, input.page_range)
                expandedInputs.extend(pages)
            Directory →
                images ← FindImagesInDirectory(input.path)
                expandedInputs.extend(images)
            Image →
                expandedInputs.push(input)
        END MATCH
    END FOR

    // Phase 2: Create processing queue
    queue ← WorkQueue::new(expandedInputs)
    results ← ConcurrentVec::new()

    // Phase 3: Parallel processing
    numWorkers ← MIN(config.max_parallel, CPU_COUNT)

    PARALLEL FOR worker IN 0..numWorkers DO
        LOOP
            input ← queue.pop()
            IF input.is_none() THEN
                BREAK
            END IF

            startTime ← GetCurrentTime()

            // Process single input
            result ← ProcessSingleInput(
                input.unwrap(),
                config,
                timeout: config.timeout_ms
            )

            processingTime ← GetCurrentTime() - startTime

            processResult ← ProcessResult {
                input: input.unwrap(),
                output: result,
                processing_time_ms: processingTime,
                status: DetermineStatus(result)
            }

            results.push(processResult)
        END LOOP
    END PARALLEL

    // Phase 4: Aggregate and return
    finalResults ← results.into_vec()
    finalResults.sort_by(|a, b| a.input.path.cmp(&b.input.path))

    RETURN Ok(finalResults)
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - With P workers, N inputs, T time per input
        - Parallel: O(N × T / P)
        - Sequential equivalent: O(N × T)
        - Speedup: ~P (linear with worker count)

    Space Complexity:
        - Queue: O(N)
        - Results: O(N × R) where R = result size
        - Worker memory: O(P × M) where M = model size
        Total: O(N × R + P × M)
```

### 5.2 PDF Page Extraction

```
ALGORITHM: ExtractPDFPages
INPUT: pdfPath (PathBuf), pageRange (Option<Range<u32>>)
OUTPUT: pages (Vec<InputSource>)

BEGIN
    // Load PDF document
    document ← PDFDocument::load(pdfPath)

    IF document.is_error() THEN
        RETURN Error("Failed to load PDF")
    END IF

    // Determine page range
    totalPages ← document.page_count()
    range ← pageRange.unwrap_or(0..totalPages)

    pages ← []

    FOR pageNum IN range DO
        IF pageNum >= totalPages THEN
            BREAK
        END IF

        // Render page to image
        page ← document.get_page(pageNum)

        // Render at high DPI for quality
        image ← page.render(dpi: 300)

        // Create temporary file
        tempPath ← CreateTempFile(format!("page_{}.png", pageNum))
        image.save(tempPath)

        inputSource ← InputSource {
            source_type: Image,
            path: tempPath,
            page_range: None
        }

        pages.push(inputSource)
    END FOR

    RETURN pages
END

COMPLEXITY ANALYSIS:
    Time: O(P × R) where P = pages, R = render time per page
    Space: O(P × S) where S = image size
```

### 5.3 Result Aggregation

```
ALGORITHM: AggregateResults
INPUT: results (Vec<ProcessResult>)
OUTPUT: aggregated (AggregatedResults)

DATA STRUCTURES:
    AggregatedResults {
        total_count: usize,
        success_count: usize,
        failure_count: usize,
        total_processing_time_ms: u64,
        average_confidence: f32,
        results_by_status: HashMap<ResultStatus, Vec<ProcessResult>>
    }

BEGIN
    totalCount ← results.len()
    successCount ← 0
    failureCount ← 0
    totalTime ← 0
    totalConfidence ← 0.0
    byStatus ← HashMap::new()

    FOR EACH result IN results DO
        totalTime ← totalTime + result.processing_time_ms

        MATCH result.status:
            Success →
                successCount ← successCount + 1
                totalConfidence ← totalConfidence + result.output.confidence
            Failure →
                failureCount ← failureCount + 1
        END MATCH

        // Group by status
        IF NOT byStatus.contains_key(result.status) THEN
            byStatus.insert(result.status, [])
        END IF
        byStatus.get_mut(result.status).push(result)
    END FOR

    avgConfidence ← IF successCount > 0 THEN
        totalConfidence / successCount
    ELSE
        0.0
    END IF

    RETURN AggregatedResults {
        total_count: totalCount,
        success_count: successCount,
        failure_count: failureCount,
        total_processing_time_ms: totalTime,
        average_confidence: avgConfidence,
        results_by_status: byStatus
    }
END

COMPLEXITY ANALYSIS:
    Time: O(n) single pass
    Space: O(n) for grouping
```

---

## 6. Caching and Memoization

### 6.1 Model Weight Caching

```
ALGORITHM: LoadModelWithCache
INPUT: modelPath (PathBuf), cacheConfig (CacheConfig)
OUTPUT: Result<Model, Error>

DATA STRUCTURES:
    CacheConfig {
        enabled: bool,
        cache_dir: PathBuf,
        max_cache_size_mb: u64,
        ttl_seconds: u64
    }

    CachedModel {
        weights: Vec<u8>,
        metadata: ModelMetadata,
        cached_at: Timestamp,
        access_count: u64
    }

BEGIN
    IF NOT cacheConfig.enabled THEN
        RETURN LoadModelDirect(modelPath)
    END IF

    // Generate cache key from model path and version
    cacheKey ← ComputeHash(modelPath, algorithm: SHA256)
    cachePath ← cacheConfig.cache_dir.join(cacheKey)

    // Check if cached version exists and is valid
    IF cachePath.exists() THEN
        cachedModel ← DeserializeCachedModel(cachePath)

        // Check TTL
        age ← GetCurrentTime() - cachedModel.cached_at
        IF age < cacheConfig.ttl_seconds THEN
            // Cache hit
            cachedModel.access_count ← cachedModel.access_count + 1
            UpdateCacheMetadata(cachePath, cachedModel.metadata)

            model ← DeserializeModel(cachedModel.weights)
            RETURN Ok(model)
        ELSE
            // Cache expired
            DeleteFile(cachePath)
        END IF
    END IF

    // Cache miss - load from disk
    model ← LoadModelDirect(modelPath)

    IF model.is_error() THEN
        RETURN model
    END IF

    // Serialize and cache
    serializedWeights ← SerializeModel(model.unwrap())

    cachedModel ← CachedModel {
        weights: serializedWeights,
        metadata: model.metadata,
        cached_at: GetCurrentTime(),
        access_count: 1
    }

    // Check cache size limit
    EnsureCacheSize(cacheConfig)

    // Write to cache
    WriteCachedModel(cachePath, cachedModel)

    RETURN model
END

SUBROUTINE: EnsureCacheSize
INPUT: cacheConfig (CacheConfig)
OUTPUT: None

BEGIN
    currentSize ← GetDirectorySize(cacheConfig.cache_dir)
    maxSize ← cacheConfig.max_cache_size_mb * 1024 * 1024

    IF currentSize <= maxSize THEN
        RETURN
    END IF

    // Evict least recently used models
    cachedFiles ← ListFiles(cacheConfig.cache_dir)

    // Sort by last access time
    cachedFiles.sort_by(|a, b| {
        a.metadata.accessed_at.cmp(&b.metadata.accessed_at)
    })

    freedSpace ← 0
    targetFree ← currentSize - maxSize

    FOR EACH file IN cachedFiles DO
        IF freedSpace >= targetFree THEN
            BREAK
        END IF

        fileSize ← GetFileSize(file)
        DeleteFile(file)
        freedSpace ← freedSpace + fileSize
    END FOR
END

COMPLEXITY ANALYSIS:
    Time Complexity:
        - Cache hit: O(1) for lookup + O(m) for deserialization
        - Cache miss: O(m) for model loading + O(m) for serialization
        - Eviction: O(k log k) where k = cached files

    Space Complexity:
        - Cached model: O(m) where m = model size
        - LRU tracking: O(k)
```

### 6.2 Result Caching with Ruvector

```
ALGORITHM: CacheResultWithVector
INPUT: imageHash (Hash), result (RecognitionResult), vectorStore (RuvectorStore)
OUTPUT: Result<(), Error>

DATA STRUCTURES:
    RuvectorStore {
        index: VectorIndex,
        metadata_db: HashMap<Hash, ResultMetadata>,
        config: VectorConfig
    }

    VectorConfig {
        embedding_dim: usize,
        similarity_threshold: f32,
        max_cache_entries: usize
    }

    ResultMetadata {
        result: RecognitionResult,
        image_hash: Hash,
        cached_at: Timestamp,
        hit_count: u64
    }

BEGIN
    // Phase 1: Generate perceptual hash
    perceptualHash ← ComputePerceptualHash(imageHash)

    // Phase 2: Check if already cached
    IF vectorStore.metadata_db.contains_key(perceptualHash) THEN
        // Update metadata
        metadata ← vectorStore.metadata_db.get_mut(perceptualHash)
        metadata.hit_count ← metadata.hit_count + 1
        RETURN Ok(())
    END IF

    // Phase 3: Generate embedding for the result
    embedding ← GenerateResultEmbedding(result)

    // Phase 4: Store in vector index
    vectorStore.index.insert(
        id: perceptualHash,
        vector: embedding
    )

    // Phase 5: Store metadata
    metadata ← ResultMetadata {
        result: result,
        image_hash: imageHash,
        cached_at: GetCurrentTime(),
        hit_count: 1
    }

    vectorStore.metadata_db.insert(perceptualHash, metadata)

    // Phase 6: Enforce cache size limit
    IF vectorStore.metadata_db.len() > vectorStore.config.max_cache_entries THEN
        EvictLeastUsedEntry(vectorStore)
    END IF

    RETURN Ok(())
END

ALGORITHM: QuerySimilarCachedResult
INPUT: imageHash (Hash), vectorStore (RuvectorStore)
OUTPUT: Option<RecognitionResult>

BEGIN
    // Generate perceptual hash
    perceptualHash ← ComputePerceptualHash(imageHash)

    // Exact match check
    IF vectorStore.metadata_db.contains_key(perceptualHash) THEN
        metadata ← vectorStore.metadata_db.get(perceptualHash)
        metadata.hit_count ← metadata.hit_count + 1
        RETURN Some(metadata.result.clone())
    END IF

    // Generate query embedding
    queryEmbedding ← GenerateImageEmbedding(imageHash)

    // Search for similar results
    results ← vectorStore.index.search(
        query: queryEmbedding,
        k: 1,
        threshold: vectorStore.config.similarity_threshold
    )

    IF results.is_empty() THEN
        RETURN None
    END IF

    bestMatch ← results[0]

    IF bestMatch.similarity >= vectorStore.config.similarity_threshold THEN
        metadata ← vectorStore.metadata_db.get(bestMatch.id)
        metadata.hit_count ← metadata.hit_count + 1
        RETURN Some(metadata.result.clone())
    END IF

    RETURN None
END

COMPLEXITY ANALYSIS:
    Caching:
        Time: O(d) for embedding + O(log n) for index insertion
        Space: O(n × d) where n = cached entries, d = embedding dim

    Querying:
        Time: O(d) for embedding + O(log n × d) for ANN search
        Space: O(k) for results where k = search parameter
```

### 6.3 Incremental Update Cache

```
ALGORITHM: UpdateCacheIncremental
INPUT: updates (Vec<CacheUpdate>), vectorStore (RuvectorStore)
OUTPUT: Result<(), Error>

DATA STRUCTURES:
    CacheUpdate {
        operation: UpdateOp,  // Insert, Update, Delete
        image_hash: Hash,
        result: Option<RecognitionResult>
    }

    UpdateOp {
        Insert,
        Update,
        Delete
    }

BEGIN
    // Batch updates for efficiency
    insertBatch ← []
    updateBatch ← []
    deleteBatch ← []

    FOR EACH update IN updates DO
        MATCH update.operation:
            Insert →
                insertBatch.push(update)
            Update →
                updateBatch.push(update)
            Delete →
                deleteBatch.push(update)
        END MATCH
    END FOR

    // Process deletes first
    FOR EACH update IN deleteBatch DO
        perceptualHash ← ComputePerceptualHash(update.image_hash)
        vectorStore.index.remove(perceptualHash)
        vectorStore.metadata_db.remove(perceptualHash)
    END FOR

    // Process updates
    FOR EACH update IN updateBatch DO
        perceptualHash ← ComputePerceptualHash(update.image_hash)

        IF vectorStore.metadata_db.contains_key(perceptualHash) THEN
            // Update existing entry
            embedding ← GenerateResultEmbedding(update.result.unwrap())
            vectorStore.index.update(perceptualHash, embedding)

            metadata ← vectorStore.metadata_db.get_mut(perceptualHash)
            metadata.result ← update.result.unwrap()
            metadata.cached_at ← GetCurrentTime()
        END IF
    END FOR

    // Process inserts in batch
    IF NOT insertBatch.is_empty() THEN
        embeddings ← []
        metadataList ← []

        FOR EACH update IN insertBatch DO
            embedding ← GenerateResultEmbedding(update.result.unwrap())
            embeddings.push(embedding)

            perceptualHash ← ComputePerceptualHash(update.image_hash)
            metadata ← ResultMetadata {
                result: update.result.unwrap(),
                image_hash: update.image_hash,
                cached_at: GetCurrentTime(),
                hit_count: 1
            }
            metadataList.push((perceptualHash, metadata))
        END FOR

        // Batch insert into vector index
        vectorStore.index.insert_batch(embeddings)

        // Batch insert metadata
        FOR EACH (hash, metadata) IN metadataList DO
            vectorStore.metadata_db.insert(hash, metadata)
        END FOR
    END IF

    RETURN Ok(())
END

COMPLEXITY ANALYSIS:
    Time: O(b × d) where b = batch size, d = embedding dim
    Space: O(b × d) for batch processing
```

---

## Summary: Complexity Analysis

### Overall System Complexity

| Component | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| Image Preprocessing | O(n log n) | O(n) |
| Vision Transformer | O(L × p² × d) | O(L × h × p²) |
| Text Recognition | O(T × B × V × log(BV)) | O(B × T × d) |
| Math Symbol Detection | O(s × c) | O(s) |
| Spatial Analysis | O(s log s) | O(s²) worst case |
| Tree Construction | O(s log s) | O(s × h) |
| LaTeX Generation | O(s) | O(h) |
| Batch Processing | O(N × T / P) | O(N × R + P × M) |
| Vector Caching | O(d + log n) | O(n × d) |

**Legend:**
- n = pixel count
- L = transformer layers
- p = number of patches
- d = embedding dimension
- h = attention heads
- T = sequence length
- B = beam width
- V = vocabulary size
- s = symbol count
- N = batch size
- P = parallel workers
- R = result size
- M = model size

### Optimization Opportunities

1. **Preprocessing**: Use GPU-accelerated image operations
2. **Transformer**: Implement efficient attention (FlashAttention)
3. **Beam Search**: Prune low-probability beams early
4. **Spatial Analysis**: Use spatial indexing (QuadTree/R-tree)
5. **Caching**: Implement tiered cache (L1: memory, L2: disk)
6. **Batch Processing**: Dynamic load balancing across workers
7. **Vector Search**: Use approximate nearest neighbor (HNSW)

---

## Design Patterns Used

1. **Pipeline Pattern**: Image preprocessing → OCR → Math parsing → Output
2. **Strategy Pattern**: Multiple output format generators
3. **Observer Pattern**: Progress tracking in batch processing
4. **Factory Pattern**: Model and cache instantiation
5. **Adapter Pattern**: Format conversion layers
6. **Repository Pattern**: Vector store abstraction
7. **Command Pattern**: Cache update operations
8. **Builder Pattern**: Expression tree and XML construction

---

*This pseudocode serves as the algorithmic blueprint for implementation in the Refinement phase.*
