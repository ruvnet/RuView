/// Example demonstrating the pattern detection APIs in temporal-compare
///
/// This example shows how to use:
/// 1. find_similar() - Find similar patterns in time series
/// 2. detect_pattern() - Detect if a pattern exists
/// 3. Advanced APIs for recurring and fuzzy pattern detection

use midstreamer_temporal_compare::{TemporalComparator, Pattern};

fn main() {
    println!("=== Temporal-Compare Pattern Detection Demo ===\n");

    // Create a comparator with cache size 100 and max sequence length 1000
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    // Example 1: find_similar() - Find exact matches
    println!("Example 1: Finding similar patterns with find_similar()");
    println!("---------------------------------------------------");
    let series1 = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 4.0, 5.0, 6.0];
    let pattern1 = vec![3.0, 4.0, 5.0];

    println!("Series: {:?}", series1);
    println!("Pattern: {:?}", pattern1);

    let matches = comparator.find_similar(&series1, &pattern1, 0.5);
    println!("Found {} matches:", matches.len());
    for (idx, distance) in &matches {
        println!("  - Index {}: distance = {:.4}", idx, distance);
    }
    println!();

    // Example 2: detect_pattern() - Simple boolean detection
    println!("Example 2: Detecting pattern existence with detect_pattern()");
    println!("-------------------------------------------------------------");
    let series2 = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    let pattern2a = vec![30.0, 40.0, 50.0];
    let pattern2b = vec![100.0, 200.0, 300.0];

    println!("Series: {:?}", series2);
    println!("Pattern A: {:?}", pattern2a);
    let found_a = comparator.detect_pattern(&series2, &pattern2a, 1.0);
    println!("Pattern A detected: {}", found_a);

    println!("Pattern B: {:?}", pattern2b);
    let found_b = comparator.detect_pattern(&series2, &pattern2b, 1.0);
    println!("Pattern B detected: {}", found_b);
    println!();

    // Example 3: Approximate matching with threshold
    println!("Example 3: Approximate matching with different thresholds");
    println!("----------------------------------------------------------");
    let series3 = vec![1.0, 2.0, 3.1, 4.2, 4.9, 6.0];
    let pattern3 = vec![3.0, 4.0, 5.0];

    println!("Series: {:?}", series3);
    println!("Pattern: {:?}", pattern3);

    // Strict threshold
    let strict_matches = comparator.find_similar(&series3, &pattern3, 0.5);
    println!("Strict threshold (0.5): {} matches", strict_matches.len());

    // Loose threshold
    let loose_matches = comparator.find_similar(&series3, &pattern3, 2.0);
    println!("Loose threshold (2.0): {} matches", loose_matches.len());
    for (idx, distance) in &loose_matches {
        println!("  - Index {}: distance = {:.4}", idx, distance);
    }
    println!();

    // Example 4: Generic API with integers
    println!("Example 4: Generic API with integer sequences");
    println!("----------------------------------------------");
    let comparator_int: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

    let haystack = vec![1, 2, 3, 4, 5, 3, 4, 5, 6, 7];
    let needle = vec![3, 4, 5];

    println!("Haystack: {:?}", haystack);
    println!("Needle: {:?}", needle);

    let matches = comparator_int.find_similar_generic(&haystack, &needle, 0.1).unwrap();
    println!("Found {} matches:", matches.len());
    for m in &matches {
        println!("  - Index {}: similarity = {:.4}, distance = {:.4}",
                 m.start_index, m.similarity, m.distance);
    }
    println!();

    // Example 5: Detect recurring patterns
    println!("Example 5: Automatic recurring pattern detection");
    println!("------------------------------------------------");
    let comparator_char: TemporalComparator<char> = TemporalComparator::new(100, 1000);

    let sequence = vec!['a', 'b', 'c', 'a', 'b', 'c', 'd', 'e', 'd', 'e'];

    println!("Sequence: {:?}", sequence);

    let patterns = comparator_char.detect_recurring_patterns(&sequence, 2, 3).unwrap();
    println!("Found {} recurring patterns:", patterns.len());
    for (i, pattern) in patterns.iter().enumerate() {
        println!("  Pattern {}: {:?}", i + 1, pattern.sequence);
        println!("    Frequency: {}", pattern.frequency());
        println!("    Confidence: {:.4}", pattern.confidence);
        println!("    Occurrences at: {:?}", pattern.occurrences);
    }
    println!();

    // Example 6: Fuzzy pattern detection
    println!("Example 6: Fuzzy pattern detection (groups similar patterns)");
    println!("-------------------------------------------------------------");
    let comparator_fuzzy: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

    let sequence = vec![1, 2, 3, 1, 2, 4, 1, 2, 3, 5, 6, 7, 5, 6, 8];

    println!("Sequence: {:?}", sequence);

    let fuzzy_patterns = comparator_fuzzy.detect_fuzzy_patterns(&sequence, 3, 3, 0.7).unwrap();
    println!("Found {} fuzzy pattern groups:", fuzzy_patterns.len());
    for (i, pattern) in fuzzy_patterns.iter().enumerate() {
        println!("  Pattern Group {}: {:?}", i + 1, pattern.sequence);
        println!("    Frequency: {} (includes variations)", pattern.frequency());
        println!("    Confidence: {:.4}", pattern.confidence);
    }
    println!();

    // Example 7: Cache performance
    println!("Example 7: Cache performance demonstration");
    println!("------------------------------------------");

    // Clear cache first
    comparator.clear_cache();

    // Run same query multiple times
    for i in 1..=5 {
        let _ = comparator.find_similar(&series1, &pattern1, 0.5);
        let stats = comparator.cache_stats();
        println!("  Iteration {}: hits = {}, misses = {}, hit rate = {:.2}%",
                 i, stats.hits, stats.misses, stats.hit_rate() * 100.0);
    }
    println!();

    println!("=== Demo Complete ===");
    println!("\nKey Takeaways:");
    println!("1. find_similar() uses DTW to find pattern matches with configurable threshold");
    println!("2. detect_pattern() provides simple boolean detection");
    println!("3. Generic APIs work with any comparable type (f64, i32, char, etc.)");
    println!("4. Automatic pattern discovery finds recurring patterns");
    println!("5. Fuzzy matching groups similar pattern variations");
    println!("6. Built-in caching improves performance for repeated queries");
}
