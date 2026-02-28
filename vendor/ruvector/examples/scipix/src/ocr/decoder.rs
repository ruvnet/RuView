//! Output Decoding Module
//!
//! This module provides various decoding strategies for converting
//! model output logits into text strings.

use super::{OcrError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Decoder trait for converting logits to text
pub trait Decoder: Send + Sync {
    /// Decode logits to text
    fn decode(&self, logits: &[Vec<f32>]) -> Result<String>;

    /// Decode with confidence scores per character
    fn decode_with_confidence(&self, logits: &[Vec<f32>]) -> Result<(String, Vec<f32>)> {
        // Default implementation just returns uniform confidence
        let text = self.decode(logits)?;
        let confidences = vec![1.0; text.len()];
        Ok((text, confidences))
    }
}

/// Vocabulary mapping for character recognition
#[derive(Debug, Clone)]
pub struct Vocabulary {
    /// Index to character mapping
    idx_to_char: HashMap<usize, char>,
    /// Character to index mapping
    char_to_idx: HashMap<char, usize>,
    /// Blank token index for CTC
    blank_idx: usize,
}

impl Vocabulary {
    /// Create a new vocabulary
    pub fn new(chars: Vec<char>, blank_idx: usize) -> Self {
        let idx_to_char: HashMap<usize, char> =
            chars.iter().enumerate().map(|(i, &c)| (i, c)).collect();
        let char_to_idx: HashMap<char, usize> =
            chars.iter().enumerate().map(|(i, &c)| (c, i)).collect();

        Self {
            idx_to_char,
            char_to_idx,
            blank_idx,
        }
    }

    /// Get character by index
    pub fn get_char(&self, idx: usize) -> Option<char> {
        self.idx_to_char.get(&idx).copied()
    }

    /// Get index by character
    pub fn get_idx(&self, ch: char) -> Option<usize> {
        self.char_to_idx.get(&ch).copied()
    }

    /// Get blank token index
    pub fn blank_idx(&self) -> usize {
        self.blank_idx
    }

    /// Get vocabulary size
    pub fn size(&self) -> usize {
        self.idx_to_char.len()
    }
}

impl Default for Vocabulary {
    fn default() -> Self {
        // Default vocabulary: lowercase letters + digits + space + blank
        let mut chars = Vec::new();

        // Add lowercase letters
        for c in 'a'..='z' {
            chars.push(c);
        }

        // Add digits
        for c in '0'..='9' {
            chars.push(c);
        }

        // Add space
        chars.push(' ');

        // Blank token is at the end
        let blank_idx = chars.len();

        Self::new(chars, blank_idx)
    }
}

/// Greedy decoder - selects the character with highest probability at each step
pub struct GreedyDecoder {
    vocabulary: Arc<Vocabulary>,
}

impl GreedyDecoder {
    /// Create a new greedy decoder
    pub fn new(vocabulary: Arc<Vocabulary>) -> Self {
        Self { vocabulary }
    }

    /// Find the index with maximum value in a slice
    fn argmax(values: &[f32]) -> usize {
        values
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }
}

impl Decoder for GreedyDecoder {
    fn decode(&self, logits: &[Vec<f32>]) -> Result<String> {
        debug!("Greedy decoding {} frames", logits.len());

        let mut result = String::new();
        let mut prev_idx = None;

        for frame_logits in logits {
            let idx = Self::argmax(frame_logits);

            // Skip blank tokens and repeated characters
            if idx != self.vocabulary.blank_idx() && Some(idx) != prev_idx {
                if let Some(ch) = self.vocabulary.get_char(idx) {
                    result.push(ch);
                }
            }

            prev_idx = Some(idx);
        }

        Ok(result)
    }

    fn decode_with_confidence(&self, logits: &[Vec<f32>]) -> Result<(String, Vec<f32>)> {
        let mut result = String::new();
        let mut confidences = Vec::new();
        let mut prev_idx = None;

        for frame_logits in logits {
            let idx = Self::argmax(frame_logits);
            let confidence = softmax_max(frame_logits);

            // Skip blank tokens and repeated characters
            if idx != self.vocabulary.blank_idx() && Some(idx) != prev_idx {
                if let Some(ch) = self.vocabulary.get_char(idx) {
                    result.push(ch);
                    confidences.push(confidence);
                }
            }

            prev_idx = Some(idx);
        }

        Ok((result, confidences))
    }
}

/// Beam search decoder - maintains top-k hypotheses for better accuracy
pub struct BeamSearchDecoder {
    vocabulary: Arc<Vocabulary>,
    beam_width: usize,
}

impl BeamSearchDecoder {
    /// Create a new beam search decoder
    pub fn new(vocabulary: Arc<Vocabulary>, beam_width: usize) -> Self {
        Self {
            vocabulary,
            beam_width: beam_width.max(1),
        }
    }

    /// Get beam width
    pub fn beam_width(&self) -> usize {
        self.beam_width
    }
}

impl Decoder for BeamSearchDecoder {
    fn decode(&self, logits: &[Vec<f32>]) -> Result<String> {
        debug!(
            "Beam search decoding {} frames (beam_width: {})",
            logits.len(),
            self.beam_width
        );

        if logits.is_empty() {
            return Ok(String::new());
        }

        // Initialize beams: (text, score, last_idx)
        let mut beams: Vec<(String, f32, Option<usize>)> = vec![(String::new(), 0.0, None)];

        for frame_logits in logits {
            let mut new_beams = Vec::new();

            for (text, score, last_idx) in &beams {
                // Get top-k predictions for this frame
                let mut indexed_logits: Vec<(usize, f32)> = frame_logits
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (i, v))
                    .collect();
                indexed_logits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                // Expand each beam with top-k predictions
                for (idx, logit) in indexed_logits.iter().take(self.beam_width) {
                    let new_score = score + logit;

                    // Skip blank tokens
                    if *idx == self.vocabulary.blank_idx() {
                        new_beams.push((text.clone(), new_score, Some(*idx)));
                        continue;
                    }

                    // Skip repeated characters (CTC collapse)
                    if Some(*idx) == *last_idx {
                        new_beams.push((text.clone(), new_score, Some(*idx)));
                        continue;
                    }

                    // Add character to beam
                    if let Some(ch) = self.vocabulary.get_char(*idx) {
                        let mut new_text = text.clone();
                        new_text.push(ch);
                        new_beams.push((new_text, new_score, Some(*idx)));
                    }
                }
            }

            // Keep top beam_width beams
            new_beams.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            new_beams.truncate(self.beam_width);
            beams = new_beams;
        }

        // Return the best beam
        Ok(beams
            .first()
            .map(|(text, _, _)| text.clone())
            .unwrap_or_default())
    }
}

/// CTC (Connectionist Temporal Classification) decoder
pub struct CTCDecoder {
    vocabulary: Arc<Vocabulary>,
}

impl CTCDecoder {
    /// Create a new CTC decoder
    pub fn new(vocabulary: Arc<Vocabulary>) -> Self {
        Self { vocabulary }
    }

    /// Collapse repeated characters and remove blanks
    fn collapse_repeats(&self, indices: &[usize]) -> Vec<usize> {
        let mut result = Vec::new();
        let mut prev_idx = None;

        for &idx in indices {
            // Skip blanks
            if idx == self.vocabulary.blank_idx() {
                prev_idx = Some(idx);
                continue;
            }

            // Skip repeats
            if Some(idx) != prev_idx {
                result.push(idx);
            }

            prev_idx = Some(idx);
        }

        result
    }
}

impl Decoder for CTCDecoder {
    fn decode(&self, logits: &[Vec<f32>]) -> Result<String> {
        debug!("CTC decoding {} frames", logits.len());

        // Get best path (greedy)
        let indices: Vec<usize> = logits
            .iter()
            .map(|frame| GreedyDecoder::argmax(frame))
            .collect();

        // Collapse repeats and remove blanks
        let collapsed = self.collapse_repeats(&indices);

        // Convert to text
        let text: String = collapsed
            .iter()
            .filter_map(|&idx| self.vocabulary.get_char(idx))
            .collect();

        Ok(text)
    }

    fn decode_with_confidence(&self, logits: &[Vec<f32>]) -> Result<(String, Vec<f32>)> {
        let indices: Vec<usize> = logits
            .iter()
            .map(|frame| GreedyDecoder::argmax(frame))
            .collect();
        let confidences: Vec<f32> = logits.iter().map(|frame| softmax_max(frame)).collect();

        let collapsed = self.collapse_repeats(&indices);

        let text: String = collapsed
            .iter()
            .filter_map(|&idx| self.vocabulary.get_char(idx))
            .collect();

        // Map confidences to non-collapsed positions
        let mut result_confidences = Vec::new();
        let mut prev_idx = None;
        let mut confidence_idx = 0;

        for &idx in &indices {
            if idx != self.vocabulary.blank_idx() && Some(idx) != prev_idx {
                if confidence_idx < confidences.len() {
                    result_confidences.push(confidences[confidence_idx]);
                }
            }
            confidence_idx += 1;
            prev_idx = Some(idx);
        }

        Ok((text, result_confidences))
    }
}

/// Calculate softmax and return max probability
fn softmax_max(logits: &[f32]) -> f32 {
    if logits.is_empty() {
        return 0.0;
    }

    let max_logit = logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let exp_sum: f32 = logits.iter().map(|&x| (x - max_logit).exp()).sum();

    let max_exp = (logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)) - max_logit).exp();
    max_exp / exp_sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vocabulary() -> Arc<Vocabulary> {
        Arc::new(Vocabulary::default())
    }

    #[test]
    fn test_vocabulary_default() {
        let vocab = Vocabulary::default();
        assert!(vocab.size() > 0);
        assert_eq!(vocab.get_char(0), Some('a'));
        assert_eq!(vocab.get_idx('a'), Some(0));
    }

    #[test]
    fn test_greedy_decoder() {
        let vocab = create_test_vocabulary();
        let decoder = GreedyDecoder::new(vocab.clone());

        // Mock logits for "hi"
        let h_idx = vocab.get_idx('h').unwrap();
        let i_idx = vocab.get_idx('i').unwrap();
        let blank = vocab.blank_idx();

        let mut logits = vec![
            vec![0.0; vocab.size() + 1],
            vec![0.0; vocab.size() + 1],
            vec![0.0; vocab.size() + 1],
        ];

        logits[0][h_idx] = 10.0;
        logits[1][blank] = 10.0;
        logits[2][i_idx] = 10.0;

        let result = decoder.decode(&logits).unwrap();
        assert_eq!(result, "hi");
    }

    #[test]
    fn test_beam_search_decoder() {
        let vocab = create_test_vocabulary();
        let decoder = BeamSearchDecoder::new(vocab.clone(), 3);

        assert_eq!(decoder.beam_width(), 3);

        let logits = vec![vec![0.0; vocab.size() + 1]; 5];
        let result = decoder.decode(&logits);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ctc_decoder() {
        let vocab = create_test_vocabulary();
        let decoder = CTCDecoder::new(vocab.clone());

        // Test collapse repeats
        let a_idx = vocab.get_idx('a').unwrap();
        let b_idx = vocab.get_idx('b').unwrap();
        let blank = vocab.blank_idx();

        let indices = vec![a_idx, a_idx, blank, b_idx, b_idx, b_idx];
        let collapsed = decoder.collapse_repeats(&indices);

        assert_eq!(collapsed, vec![a_idx, b_idx]);
    }

    #[test]
    fn test_softmax_max() {
        let logits = vec![1.0, 2.0, 3.0, 2.0, 1.0];
        let max_prob = softmax_max(&logits);
        assert!(max_prob > 0.0 && max_prob <= 1.0);
        assert!(max_prob > 0.5); // The max should have high probability
    }

    #[test]
    fn test_empty_logits() {
        let vocab = create_test_vocabulary();
        let decoder = GreedyDecoder::new(vocab);

        let empty_logits: Vec<Vec<f32>> = vec![];
        let result = decoder.decode(&empty_logits).unwrap();
        assert_eq!(result, "");
    }
}
