//! Text region detection and segmentation

use super::{RegionType, Result, TextRegion};
use image::GrayImage;
use std::collections::{HashMap, HashSet};

/// Find text regions in a binary or grayscale image
///
/// Uses connected component analysis and geometric heuristics to identify
/// text regions and classify them by type (text, math, table, etc.)
///
/// # Arguments
/// * `image` - Input grayscale or binary image
/// * `min_region_size` - Minimum region area in pixels
///
/// # Returns
/// Vector of detected text regions with bounding boxes
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::segmentation::find_text_regions;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let regions = find_text_regions(&image, 100).unwrap();
/// println!("Found {} regions", regions.len());
/// ```
pub fn find_text_regions(image: &GrayImage, min_region_size: u32) -> Result<Vec<TextRegion>> {
    // Find connected components
    let components = connected_components(image);

    // Extract bounding boxes for each component
    let bboxes = extract_bounding_boxes(&components);

    // Filter by size and merge overlapping regions
    let filtered = filter_by_size(bboxes, min_region_size);
    let merged = merge_overlapping_regions(filtered, 10);

    // Find text lines and group components
    let text_lines = find_text_lines(image, &merged);

    // Classify regions and create TextRegion objects
    let regions = classify_regions(image, text_lines);

    Ok(regions)
}

/// Connected component labeling using flood-fill algorithm
///
/// Returns labeled image where each connected component has a unique ID
fn connected_components(image: &GrayImage) -> Vec<Vec<u32>> {
    let (width, height) = image.dimensions();
    let mut labels = vec![vec![0u32; width as usize]; height as usize];
    let mut current_label = 1u32;

    for y in 0..height {
        for x in 0..width {
            if labels[y as usize][x as usize] == 0 && image.get_pixel(x, y)[0] < 128 {
                // Found unlabeled foreground pixel, start flood fill
                flood_fill(image, &mut labels, x, y, current_label);
                current_label += 1;
            }
        }
    }

    labels
}

/// Flood fill algorithm for connected component labeling
fn flood_fill(image: &GrayImage, labels: &mut [Vec<u32>], start_x: u32, start_y: u32, label: u32) {
    let (width, height) = image.dimensions();
    let mut stack = vec![(start_x, start_y)];

    while let Some((x, y)) = stack.pop() {
        if x >= width || y >= height {
            continue;
        }

        if labels[y as usize][x as usize] != 0 || image.get_pixel(x, y)[0] >= 128 {
            continue;
        }

        labels[y as usize][x as usize] = label;

        // Add 4-connected neighbors
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x < width - 1 {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y < height - 1 {
            stack.push((x, y + 1));
        }
    }
}

/// Extract bounding boxes for each labeled component
fn extract_bounding_boxes(labels: &[Vec<u32>]) -> HashMap<u32, (u32, u32, u32, u32)> {
    let mut bboxes: HashMap<u32, (u32, u32, u32, u32)> = HashMap::new();

    for (y, row) in labels.iter().enumerate() {
        for (x, &label) in row.iter().enumerate() {
            if label == 0 {
                continue;
            }

            let bbox = bboxes
                .entry(label)
                .or_insert((x as u32, y as u32, x as u32, y as u32));

            // Update bounding box
            bbox.0 = bbox.0.min(x as u32); // min_x
            bbox.1 = bbox.1.min(y as u32); // min_y
            bbox.2 = bbox.2.max(x as u32); // max_x
            bbox.3 = bbox.3.max(y as u32); // max_y
        }
    }

    // Convert to (x, y, width, height) format
    bboxes
        .into_iter()
        .map(|(label, (min_x, min_y, max_x, max_y))| {
            let width = max_x - min_x + 1;
            let height = max_y - min_y + 1;
            (label, (min_x, min_y, width, height))
        })
        .collect()
}

/// Filter regions by minimum size
fn filter_by_size(
    bboxes: HashMap<u32, (u32, u32, u32, u32)>,
    min_size: u32,
) -> Vec<(u32, u32, u32, u32)> {
    bboxes
        .into_values()
        .filter(|(_, _, w, h)| w * h >= min_size)
        .collect()
}

/// Merge overlapping or nearby regions
///
/// # Arguments
/// * `regions` - Vector of bounding boxes (x, y, width, height)
/// * `merge_distance` - Maximum distance to merge regions
pub fn merge_overlapping_regions(
    regions: Vec<(u32, u32, u32, u32)>,
    merge_distance: u32,
) -> Vec<(u32, u32, u32, u32)> {
    if regions.is_empty() {
        return regions;
    }

    let mut merged = Vec::new();
    let mut used = HashSet::new();

    for i in 0..regions.len() {
        if used.contains(&i) {
            continue;
        }

        let mut current = regions[i];
        let mut changed = true;

        while changed {
            changed = false;

            for j in (i + 1)..regions.len() {
                if used.contains(&j) {
                    continue;
                }

                if boxes_overlap_or_close(&current, &regions[j], merge_distance) {
                    current = merge_boxes(&current, &regions[j]);
                    used.insert(j);
                    changed = true;
                }
            }
        }

        merged.push(current);
        used.insert(i);
    }

    merged
}

/// Check if two bounding boxes overlap or are close
fn boxes_overlap_or_close(
    box1: &(u32, u32, u32, u32),
    box2: &(u32, u32, u32, u32),
    distance: u32,
) -> bool {
    let (x1, y1, w1, h1) = *box1;
    let (x2, y2, w2, h2) = *box2;

    let x1_end = x1 + w1;
    let y1_end = y1 + h1;
    let x2_end = x2 + w2;
    let y2_end = y2 + h2;

    // Check for overlap or proximity
    let x_overlap = (x1 <= x2_end + distance) && (x2 <= x1_end + distance);
    let y_overlap = (y1 <= y2_end + distance) && (y2 <= y1_end + distance);

    x_overlap && y_overlap
}

/// Merge two bounding boxes
fn merge_boxes(box1: &(u32, u32, u32, u32), box2: &(u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    let (x1, y1, w1, h1) = *box1;
    let (x2, y2, w2, h2) = *box2;

    let min_x = x1.min(x2);
    let min_y = y1.min(y2);
    let max_x = (x1 + w1).max(x2 + w2);
    let max_y = (y1 + h1).max(y2 + h2);

    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Find text lines using projection profiles
///
/// Groups regions into lines based on vertical alignment
pub fn find_text_lines(
    _image: &GrayImage,
    regions: &[(u32, u32, u32, u32)],
) -> Vec<Vec<(u32, u32, u32, u32)>> {
    if regions.is_empty() {
        return Vec::new();
    }

    // Sort regions by y-coordinate
    let mut sorted_regions = regions.to_vec();
    sorted_regions.sort_by_key(|r| r.1);

    let mut lines = Vec::new();
    let mut current_line = vec![sorted_regions[0]];

    for region in sorted_regions.iter().skip(1) {
        let (_, y, _, h) = region;
        let (_, prev_y, _, prev_h) = current_line.last().unwrap();

        // Check if region is on the same line (vertical overlap)
        let line_height = (*prev_h).max(*h);
        let distance = if y > prev_y { y - prev_y } else { prev_y - y };

        if distance < line_height / 2 {
            current_line.push(*region);
        } else {
            lines.push(current_line.clone());
            current_line = vec![*region];
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Classify regions by type (text, math, table, etc.)
fn classify_regions(
    image: &GrayImage,
    text_lines: Vec<Vec<(u32, u32, u32, u32)>>,
) -> Vec<TextRegion> {
    let mut regions = Vec::new();

    for line in text_lines {
        for bbox in line {
            let (x, y, width, height) = bbox;

            // Calculate features for classification
            let aspect_ratio = width as f32 / height as f32;
            let density = calculate_density(image, bbox);

            // Simple heuristic classification
            let region_type = if aspect_ratio > 10.0 {
                // Very wide region might be a table or figure caption
                RegionType::Table
            } else if aspect_ratio < 0.5 && height > 50 {
                // Tall region might be a figure
                RegionType::Figure
            } else if density > 0.3 && height < 30 {
                // Dense, small region likely math
                RegionType::Math
            } else {
                // Default to text
                RegionType::Text
            };

            regions.push(TextRegion {
                region_type,
                bbox: (x, y, width, height),
                confidence: 0.8, // Default confidence
                text_height: height as f32,
                baseline_angle: 0.0,
            });
        }
    }

    regions
}

/// Calculate pixel density in a region
fn calculate_density(image: &GrayImage, bbox: (u32, u32, u32, u32)) -> f32 {
    let (x, y, width, height) = bbox;
    let total_pixels = (width * height) as f32;

    if total_pixels == 0.0 {
        return 0.0;
    }

    let mut foreground_pixels = 0;

    for py in y..(y + height) {
        for px in x..(x + width) {
            if image.get_pixel(px, py)[0] < 128 {
                foreground_pixels += 1;
            }
        }
    }

    foreground_pixels as f32 / total_pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    fn create_test_image_with_rectangles() -> GrayImage {
        let mut img = GrayImage::new(200, 200);

        // Fill with white
        for pixel in img.pixels_mut() {
            *pixel = Luma([255]);
        }

        // Draw some black rectangles (simulating text regions)
        for y in 20..40 {
            for x in 20..100 {
                img.put_pixel(x, y, Luma([0]));
            }
        }

        for y in 60..80 {
            for x in 20..120 {
                img.put_pixel(x, y, Luma([0]));
            }
        }

        for y in 100..120 {
            for x in 20..80 {
                img.put_pixel(x, y, Luma([0]));
            }
        }

        img
    }

    #[test]
    fn test_find_text_regions() {
        let img = create_test_image_with_rectangles();
        let regions = find_text_regions(&img, 100);

        assert!(regions.is_ok());
        let r = regions.unwrap();

        // Should find at least 3 regions
        assert!(r.len() >= 3);

        for region in r {
            println!("Region: {:?} at {:?}", region.region_type, region.bbox);
        }
    }

    #[test]
    fn test_connected_components() {
        let img = create_test_image_with_rectangles();
        let components = connected_components(&img);

        // Check that we have non-zero labels
        let max_label = components
            .iter()
            .flat_map(|row| row.iter())
            .max()
            .unwrap_or(&0);

        assert!(*max_label > 0);
    }

    #[test]
    fn test_merge_overlapping_regions() {
        let regions = vec![(10, 10, 50, 20), (40, 10, 50, 20), (100, 100, 30, 30)];

        let merged = merge_overlapping_regions(regions, 10);

        // First two should merge, third stays separate
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_boxes() {
        let box1 = (10, 10, 50, 20);
        let box2 = (40, 15, 30, 25);

        let merged = merge_boxes(&box1, &box2);

        assert_eq!(merged.0, 10); // min x
        assert_eq!(merged.1, 10); // min y
        assert!(merged.2 >= 50); // width
        assert!(merged.3 >= 25); // height
    }

    #[test]
    fn test_boxes_overlap() {
        let box1 = (10, 10, 50, 20);
        let box2 = (40, 10, 50, 20);

        assert!(boxes_overlap_or_close(&box1, &box2, 0));
        assert!(boxes_overlap_or_close(&box1, &box2, 10));
    }

    #[test]
    fn test_boxes_dont_overlap() {
        let box1 = (10, 10, 20, 20);
        let box2 = (100, 100, 20, 20);

        assert!(!boxes_overlap_or_close(&box1, &box2, 0));
    }

    #[test]
    fn test_find_text_lines() {
        let regions = vec![
            (10, 10, 50, 20),
            (70, 12, 50, 20),
            (10, 50, 50, 20),
            (70, 52, 50, 20),
        ];

        let img = GrayImage::new(200, 100);
        let lines = find_text_lines(&img, &regions);

        // Should find 2 lines
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].len(), 2);
        assert_eq!(lines[1].len(), 2);
    }

    #[test]
    fn test_calculate_density() {
        let mut img = GrayImage::new(100, 100);

        // Fill region with 50% black pixels
        for y in 10..30 {
            for x in 10..30 {
                let val = if (x + y) % 2 == 0 { 0 } else { 255 };
                img.put_pixel(x, y, Luma([val]));
            }
        }

        let density = calculate_density(&img, (10, 10, 20, 20));
        assert!((density - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_filter_by_size() {
        let mut bboxes = HashMap::new();
        bboxes.insert(1, (10, 10, 50, 50)); // 2500 pixels
        bboxes.insert(2, (100, 100, 10, 10)); // 100 pixels
        bboxes.insert(3, (200, 200, 30, 30)); // 900 pixels

        let filtered = filter_by_size(bboxes, 500);

        // Should keep regions 1 and 3
        assert_eq!(filtered.len(), 2);
    }
}
