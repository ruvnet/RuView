//! Exotic Capability: Cross-Modal Search (Text + Image)
//!
//! Demonstrates RVF for multi-modal embedding search where text and image
//! embeddings coexist in the same vector space (e.g., CLIP-style).
//! Cross-modal queries find matching embeddings across modalities.
//!
//! Features:
//!   - 200 text embeddings + 200 image embeddings in a shared space
//!   - Cross-modal search: query with text, find matching images
//!   - Same-modal search: query text, find similar text
//!   - Filter by modality to restrict search domain
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example multimodal_fusion

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Create a "paired" vector that is close to a source vector in embedding space.
/// Demonstrates how CLIP maps matching text/image pairs to nearby points.
fn paired_vector(source: &[f32], pair_seed: u64, noise_scale: f32) -> Vec<f32> {
    let dim = source.len();
    let noise = random_vector(dim, pair_seed);
    source
        .iter()
        .zip(noise.iter())
        .map(|(s, n)| s + n * noise_scale)
        .collect()
}

fn main() {
    println!("=== Multi-Modal Fusion: Cross-Modal Search ===\n");

    let dim = 512;
    let num_text = 200;
    let num_image = 200;
    let total = num_text + num_image;

    let text_content_types = ["caption", "description", "title", "abstract", "review"];
    let image_content_types = ["photo", "diagram", "chart", "sketch", "screenshot"];

    // ====================================================================
    // 1. Create store
    // ====================================================================
    println!("--- 1. Create Multi-Modal Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("multimodal.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store: {} dims, L2 metric", dim);

    // ====================================================================
    // 2. Generate and insert text embeddings
    // ====================================================================
    println!("\n--- 2. Ingest Text Embeddings ---");

    let text_vectors: Vec<Vec<f32>> = (0..num_text)
        .map(|i| random_vector(dim, i as u64 * 100))
        .collect();
    let text_refs: Vec<&[f32]> = text_vectors.iter().map(|v| v.as_slice()).collect();
    let text_ids: Vec<u64> = (0..num_text as u64).collect();

    // Text metadata: modality (0), content_type (1)
    let mut text_meta = Vec::with_capacity(num_text * 2);
    for i in 0..num_text {
        text_meta.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String("text".to_string()),
        });
        text_meta.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(
                text_content_types[i % text_content_types.len()].to_string(),
            ),
        });
    }

    let text_ingest = store
        .ingest_batch(&text_refs, &text_ids, Some(&text_meta))
        .expect("text ingest failed");
    println!("  Ingested {} text embeddings", text_ingest.accepted);

    // ====================================================================
    // 3. Generate and insert image embeddings
    // ====================================================================
    println!("\n--- 3. Ingest Image Embeddings ---");

    // Some image embeddings are "paired" with text (representing CLIP alignment)
    // For the first 50 images, create embeddings close to corresponding text
    let image_vectors: Vec<Vec<f32>> = (0..num_image)
        .map(|i| {
            if i < 50 {
                // Paired with text embedding i — close in space
                paired_vector(&text_vectors[i], (num_text + i) as u64 * 100 + 7, 0.1)
            } else {
                // Independent image embedding
                random_vector(dim, (num_text + i) as u64 * 100 + 7)
            }
        })
        .collect();
    let image_refs: Vec<&[f32]> = image_vectors.iter().map(|v| v.as_slice()).collect();
    let image_ids: Vec<u64> = (num_text as u64..(num_text + num_image) as u64).collect();

    // Image metadata: modality (0), content_type (1)
    let mut image_meta = Vec::with_capacity(num_image * 2);
    for i in 0..num_image {
        image_meta.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String("image".to_string()),
        });
        image_meta.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(
                image_content_types[i % image_content_types.len()].to_string(),
            ),
        });
    }

    let image_ingest = store
        .ingest_batch(&image_refs, &image_ids, Some(&image_meta))
        .expect("image ingest failed");
    println!("  Ingested {} image embeddings", image_ingest.accepted);
    println!("  ({} paired with text, {} independent)", 50, num_image - 50);

    let status = store.status();
    println!("\n  Total vectors: {} ({} text + {} image)", status.total_vectors, num_text, num_image);

    // ====================================================================
    // 4. Unfiltered search (both modalities)
    // ====================================================================
    println!("\n--- 4. Unfiltered Search (Both Modalities) ---");

    // Query with a text embedding (ID 10)
    let query_text = &text_vectors[10];
    let k = 10;

    let results_all = store
        .query(query_text, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Query: text embedding #10 -> top-{}:", k);
    print_modal_results(&results_all, num_text);

    let text_in_results = results_all.iter().filter(|r| (r.id as usize) < num_text).count();
    let image_in_results = results_all.iter().filter(|r| (r.id as usize) >= num_text).count();
    println!("  Mix: {} text, {} image", text_in_results, image_in_results);

    // ====================================================================
    // 5. Cross-modal: query text, find images only
    // ====================================================================
    println!("\n--- 5. Cross-Modal Search (Text -> Image) ---");

    let filter_image = FilterExpr::Eq(0, FilterValue::String("image".to_string()));
    let opts_image = QueryOptions {
        filter: Some(filter_image),
        ..Default::default()
    };
    let results_cross = store
        .query(query_text, k, &opts_image)
        .expect("filtered query failed");

    println!("  Query: text #10 -> images only (top-{}):", k);
    print_modal_results(&results_cross, num_text);

    for r in &results_cross {
        assert!(r.id as usize >= num_text, "expected image, got text ID {}", r.id);
    }
    println!("  All results verified: modality == image.");

    // Check if the paired image (ID = num_text + 10) appears in results
    let paired_id = (num_text + 10) as u64;
    let paired_found = results_cross.iter().any(|r| r.id == paired_id);
    println!(
        "  Paired image (ID {}): {}",
        paired_id,
        if paired_found { "FOUND (cross-modal alignment works)" } else { "not in top-k" }
    );

    // ====================================================================
    // 6. Same-modal: query text, find text only
    // ====================================================================
    println!("\n--- 6. Same-Modal Search (Text -> Text) ---");

    let filter_text = FilterExpr::Eq(0, FilterValue::String("text".to_string()));
    let opts_text = QueryOptions {
        filter: Some(filter_text),
        ..Default::default()
    };
    let results_same = store
        .query(query_text, k, &opts_text)
        .expect("filtered query failed");

    println!("  Query: text #10 -> text only (top-{}):", k);
    print_modal_results(&results_same, num_text);

    for r in &results_same {
        assert!((r.id as usize) < num_text, "expected text, got image ID {}", r.id);
    }
    println!("  All results verified: modality == text.");

    // ====================================================================
    // 7. Cross-modal from image side
    // ====================================================================
    println!("\n--- 7. Cross-Modal Search (Image -> Text) ---");

    // Query with a paired image embedding to find matching text
    let query_image = &image_vectors[10]; // This is paired with text #10
    let filter_text2 = FilterExpr::Eq(0, FilterValue::String("text".to_string()));
    let opts_text2 = QueryOptions {
        filter: Some(filter_text2),
        ..Default::default()
    };
    let results_img2txt = store
        .query(query_image, k, &opts_text2)
        .expect("query failed");

    println!("  Query: paired image #{} -> text only (top-{}):", num_text + 10, k);
    print_modal_results(&results_img2txt, num_text);

    let paired_text_found = results_img2txt.iter().any(|r| r.id == 10);
    println!(
        "  Paired text (ID 10): {}",
        if paired_text_found { "FOUND (bidirectional alignment)" } else { "not in top-k" }
    );

    // ====================================================================
    // 8. Filter by content type
    // ====================================================================
    println!("\n--- 8. Content Type Filter ---");

    let filter_photo = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("image".to_string())),
        FilterExpr::Eq(1, FilterValue::String("photo".to_string())),
    ]);
    let opts_photo = QueryOptions {
        filter: Some(filter_photo),
        ..Default::default()
    };
    let results_photo = store
        .query(query_text, k, &opts_photo)
        .expect("query failed");

    println!("  Photos matching text #10: {}", results_photo.len());

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Multi-Modal Fusion Summary ===\n");
    println!("  Total embeddings:     {} ({} text + {} image)", total, num_text, num_image);
    println!("  Paired embeddings:    50 (CLIP-style alignment)");
    println!("  Embedding dims:       {}", dim);
    println!("  Unfiltered results:   {} ({} text, {} image)", results_all.len(), text_in_results, image_in_results);
    println!("  Cross-modal (T->I):   {} results", results_cross.len());
    println!("  Same-modal (T->T):    {} results", results_same.len());
    println!("  Cross-modal (I->T):   {} results", results_img2txt.len());
    println!("  Content-filtered:     {} results", results_photo.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_modal_results(results: &[SearchResult], text_count: usize) {
    let text_content_types = ["caption", "description", "title", "abstract", "review"];
    let image_content_types = ["photo", "diagram", "chart", "sketch", "screenshot"];

    println!(
        "    {:>6}  {:>12}  {:>8}  {:>12}",
        "ID", "Distance", "Modality", "ContentType"
    );
    println!("    {:->6}  {:->12}  {:->8}  {:->12}", "", "", "", "");
    for r in results {
        let idx = r.id as usize;
        let (modality, content_type) = if idx < text_count {
            ("text", text_content_types[idx % text_content_types.len()])
        } else {
            let img_idx = idx - text_count;
            ("image", image_content_types[img_idx % image_content_types.len()])
        };
        println!(
            "    {:>6}  {:>12.6}  {:>8}  {:>12}",
            r.id, r.distance, modality, content_type
        );
    }
}
