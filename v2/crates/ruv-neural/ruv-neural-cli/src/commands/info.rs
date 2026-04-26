//! Display system info and capabilities.

/// Run the info command.
pub fn run() {
    let version = env!("CARGO_PKG_VERSION");

    println!("=== rUv Neural — System Information ===");
    println!();
    println!("  Version:     {version}");
    println!("  Binary:      ruv-neural");
    println!();
    println!("  Crate Versions:");
    println!("    ruv-neural-core     {version}");
    println!("    ruv-neural-sensor   {version}");
    println!("    ruv-neural-signal   {version}");
    println!("    ruv-neural-graph    {version}");
    println!("    ruv-neural-mincut   {version}");
    println!("    ruv-neural-embed    {version}");
    println!("    ruv-neural-memory   {version}");
    println!("    ruv-neural-decoder  {version}");
    println!("    ruv-neural-viz      {version}");
    println!("    ruv-neural-cli      {version}");
    println!();
    println!("  Features:");
    println!("    Sensor simulation       [available]");
    println!("    Signal processing       [available]");
    println!("    Bandpass filtering       [available]  (Butterworth IIR, SOS form)");
    println!("    Artifact rejection       [available]  (eye blink, muscle, cardiac)");
    println!("    PLV connectivity         [available]  (phase locking value)");
    println!("    Coherence metrics        [available]  (coherence, imaginary coherence)");
    println!("    Stoer-Wagner mincut      [available]  (global minimum cut)");
    println!("    Normalized cut           [available]  (Shi-Malik spectral bisection)");
    println!("    Multi-way cut            [available]  (recursive normalized cut)");
    println!("    Spectral embedding       [available]  (Laplacian eigenvector encoding)");
    println!("    Topology embedding       [available]  (hand-crafted topological features)");
    println!("    Node2Vec embedding       [available]  (random walk co-occurrence)");
    println!("    Threshold decoder        [available]  (rule-based cognitive state)");
    println!("    KNN decoder              [available]  (k-nearest neighbor classifier)");
    println!("    Force-directed layout    [available]  (Fruchterman-Reingold)");
    println!("    Anatomical layout        [available]  (MNI coordinate-based)");
    println!();
    println!("  Export Formats:");
    println!("    D3.js JSON              [available]");
    println!("    Graphviz DOT            [available]");
    println!("    GEXF (Graph Exchange)   [available]");
    println!("    CSV edge list           [available]");
    println!("    RVF (RuVector File)     [available]");
    println!();
    println!("  Pipeline:");
    println!("    simulate -> filter -> PLV graph -> mincut -> embed -> decode");
    println!();
    println!("  Platform:");
    println!("    OS:           {}", std::env::consts::OS);
    println!("    Arch:         {}", std::env::consts::ARCH);
    println!("    Family:       {}", std::env::consts::FAMILY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_runs_without_panic() {
        run();
    }
}
