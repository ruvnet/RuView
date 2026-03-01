//! CSV/TSV importer for RVF stores.
//!
//! Expects a CSV where one column contains the vector ID and a contiguous
//! range of columns holds the vector components (as f32).
//!
//! Example CSV (id_column=0, vector_start=1, dimension=3):
//! ```text
//! id,x0,x1,x2
//! 1,0.1,0.2,0.3
//! 2,0.4,0.5,0.6
//! ```

use crate::VectorRecord;
use std::io::Read;
use std::path::Path;

/// Configuration for CSV parsing.
#[derive(Clone, Debug)]
pub struct CsvConfig {
    /// Column index (0-based) that holds the vector ID.
    pub id_column: usize,
    /// Column index (0-based) where vector components begin.
    pub vector_start: usize,
    /// Expected vector dimensionality. If `None`, it is inferred from the
    /// first data row as `num_columns - vector_start`.
    pub dimension: Option<usize>,
    /// Field delimiter. Defaults to `,`.
    pub delimiter: u8,
    /// Whether the first row is a header row (skipped).
    pub has_header: bool,
}

impl Default for CsvConfig {
    fn default() -> Self {
        Self {
            id_column: 0,
            vector_start: 1,
            dimension: None,
            delimiter: b',',
            has_header: true,
        }
    }
}

/// Parse CSV from a reader with the given config.
pub fn parse_csv<R: Read>(reader: R, config: &CsvConfig) -> Result<Vec<VectorRecord>, String> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(config.delimiter)
        .has_headers(config.has_header)
        .from_reader(reader);

    let mut records = Vec::new();
    let mut inferred_dim: Option<usize> = config.dimension;

    for (row_idx, result) in csv_reader.records().enumerate() {
        let record = result.map_err(|e| format!("CSV row {}: {e}", row_idx + 1))?;

        let id: u64 = record
            .get(config.id_column)
            .ok_or_else(|| {
                format!(
                    "row {}: missing id column {}",
                    row_idx + 1,
                    config.id_column
                )
            })?
            .trim()
            .parse()
            .map_err(|e| format!("row {}: bad id: {e}", row_idx + 1))?;

        let dim = match inferred_dim {
            Some(d) => d,
            None => {
                let d = record.len().saturating_sub(config.vector_start);
                if d == 0 {
                    return Err(format!(
                        "row {}: no vector columns after index {}",
                        row_idx + 1,
                        config.vector_start
                    ));
                }
                inferred_dim = Some(d);
                d
            }
        };

        let end = config.vector_start + dim;
        if record.len() < end {
            return Err(format!(
                "row {}: expected {} columns for vector, got {}",
                row_idx + 1,
                dim,
                record.len().saturating_sub(config.vector_start)
            ));
        }

        let mut vector = Vec::with_capacity(dim);
        for col in config.vector_start..end {
            let val: f32 = record
                .get(col)
                .ok_or_else(|| format!("row {}: missing column {col}", row_idx + 1))?
                .trim()
                .parse()
                .map_err(|e| format!("row {}, col {col}: bad float: {e}", row_idx + 1))?;
            vector.push(val);
        }

        records.push(VectorRecord {
            id,
            vector,
            metadata: Vec::new(),
        });
    }

    Ok(records)
}

/// Parse CSV from a file path.
pub fn parse_csv_file(path: &Path, config: &CsvConfig) -> Result<Vec<VectorRecord>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let reader = std::io::BufReader::new(file);
    parse_csv(reader, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_csv() {
        let data = "id,x0,x1,x2\n1,0.1,0.2,0.3\n2,0.4,0.5,0.6\n";
        let config = CsvConfig::default();
        let records = parse_csv(data.as_bytes(), &config).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 1);
        assert_eq!(records[0].vector, vec![0.1, 0.2, 0.3]);
        assert_eq!(records[1].id, 2);
        assert_eq!(records[1].vector, vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn parse_tsv() {
        let data = "id\tx0\tx1\n10\t1.0\t2.0\n20\t3.0\t4.0\n";
        let config = CsvConfig {
            delimiter: b'\t',
            ..Default::default()
        };
        let records = parse_csv(data.as_bytes(), &config).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 10);
        assert_eq!(records[0].vector, vec![1.0, 2.0]);
    }

    #[test]
    fn parse_no_header() {
        let data = "1,0.1,0.2\n2,0.3,0.4\n";
        let config = CsvConfig {
            has_header: false,
            dimension: Some(2),
            ..Default::default()
        };
        let records = parse_csv(data.as_bytes(), &config).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn parse_custom_columns() {
        let data = "x0,x1,id\n0.1,0.2,100\n0.3,0.4,200\n";
        let config = CsvConfig {
            id_column: 2,
            vector_start: 0,
            dimension: Some(2),
            ..Default::default()
        };
        let records = parse_csv(data.as_bytes(), &config).unwrap();
        assert_eq!(records[0].id, 100);
        assert_eq!(records[0].vector, vec![0.1, 0.2]);
    }

    #[test]
    fn parse_empty_csv() {
        let data = "id,x0\n";
        let config = CsvConfig::default();
        let records = parse_csv(data.as_bytes(), &config).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn bad_float_gives_error() {
        let data = "id,x0\n1,notanumber\n";
        let config = CsvConfig::default();
        let result = parse_csv(data.as_bytes(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad float"));
    }

    #[test]
    fn infer_dimension_from_first_row() {
        let data = "id,a,b,c,d\n1,0.1,0.2,0.3,0.4\n2,0.5,0.6,0.7,0.8\n";
        let config = CsvConfig {
            dimension: None,
            ..Default::default()
        };
        let records = parse_csv(data.as_bytes(), &config).unwrap();
        assert_eq!(records[0].vector.len(), 4);
        assert_eq!(records[1].vector.len(), 4);
    }
}
