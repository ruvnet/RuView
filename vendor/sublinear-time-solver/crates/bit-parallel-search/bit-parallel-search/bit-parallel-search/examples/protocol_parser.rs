//! Example: Network protocol parsing
//! Shows how bit-parallel search excels in parsing binary protocols

use bit_parallel_search::BitParallelSearcher;
use std::time::Instant;

struct ProtocolParser {
    // HTTP methods (short patterns - optimal for bit-parallel)
    get_searcher: BitParallelSearcher,
    post_searcher: BitParallelSearcher,
    put_searcher: BitParallelSearcher,
    delete_searcher: BitParallelSearcher,

    // Protocol delimiters
    crlf_searcher: BitParallelSearcher,
    double_crlf_searcher: BitParallelSearcher,

    // Common headers
    json_content_searcher: BitParallelSearcher,
    xml_content_searcher: BitParallelSearcher,
}

impl ProtocolParser {
    fn new() -> Self {
        Self {
            get_searcher: BitParallelSearcher::new(b"GET "),
            post_searcher: BitParallelSearcher::new(b"POST "),
            put_searcher: BitParallelSearcher::new(b"PUT "),
            delete_searcher: BitParallelSearcher::new(b"DELETE "),
            crlf_searcher: BitParallelSearcher::new(b"\r\n"),
            double_crlf_searcher: BitParallelSearcher::new(b"\r\n\r\n"),
            json_content_searcher: BitParallelSearcher::new(b"application/json"),
            xml_content_searcher: BitParallelSearcher::new(b"application/xml"),
        }
    }

    fn parse_request(&self, data: &[u8]) -> RequestInfo {
        RequestInfo {
            method: self.detect_method(data),
            header_end: self.double_crlf_searcher.find_in(data),
            line_count: self.crlf_searcher.count_in(data),
            content_type: self.detect_content_type(data),
        }
    }

    fn detect_method(&self, data: &[u8]) -> Method {
        if self.get_searcher.find_in(data).is_some() {
            Method::GET
        } else if self.post_searcher.find_in(data).is_some() {
            Method::POST
        } else if self.put_searcher.find_in(data).is_some() {
            Method::PUT
        } else if self.delete_searcher.find_in(data).is_some() {
            Method::DELETE
        } else {
            Method::Unknown
        }
    }

    fn detect_content_type(&self, data: &[u8]) -> ContentType {
        if self.json_content_searcher.find_in(data).is_some() {
            ContentType::JSON
        } else if self.xml_content_searcher.find_in(data).is_some() {
            ContentType::XML
        } else {
            ContentType::Unknown
        }
    }
}

#[derive(Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    Unknown,
}

#[derive(Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum ContentType {
    JSON,
    XML,
    Unknown,
}

#[derive(Debug)]
#[allow(dead_code)]
struct RequestInfo {
    method: Method,
    header_end: Option<usize>,
    line_count: usize,
    content_type: ContentType,
}

// Naive implementation for comparison
fn naive_parse_request(data: &[u8]) -> RequestInfo {
    RequestInfo {
        method: naive_detect_method(data),
        header_end: naive_find(data, b"\r\n\r\n"),
        line_count: naive_count(data, b"\r\n"),
        content_type: naive_detect_content_type(data),
    }
}

fn naive_detect_method(data: &[u8]) -> Method {
    if naive_find(data, b"GET ").is_some() {
        Method::GET
    } else if naive_find(data, b"POST ").is_some() {
        Method::POST
    } else if naive_find(data, b"PUT ").is_some() {
        Method::PUT
    } else if naive_find(data, b"DELETE ").is_some() {
        Method::DELETE
    } else {
        Method::Unknown
    }
}

fn naive_detect_content_type(data: &[u8]) -> ContentType {
    if naive_find(data, b"application/json").is_some() {
        ContentType::JSON
    } else if naive_find(data, b"application/xml").is_some() {
        ContentType::XML
    } else {
        ContentType::Unknown
    }
}

fn naive_find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len())
        .position(|window| window == needle)
}

fn naive_count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack.windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn generate_test_requests() -> Vec<Vec<u8>> {
    vec![
        b"GET /api/users HTTP/1.1\r\nHost: example.com\r\nAccept: application/json\r\n\r\n".to_vec(),
        b"POST /api/users HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\nContent-Length: 123\r\n\r\n{\"name\":\"test\"}".to_vec(),
        b"PUT /api/users/123 HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/xml\r\n\r\n<user><name>test</name></user>".to_vec(),
        b"DELETE /api/users/123 HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer token\r\n\r\n".to_vec(),
    ]
}

fn main() {
    println!("🌐 Network Protocol Parsing Performance Demo\n");

    let parser = ProtocolParser::new();
    let test_requests = generate_test_requests();

    // Show parsing results
    println!("📋 Parsing Results:");
    println!("===================");
    for (i, request) in test_requests.iter().enumerate() {
        let info = parser.parse_request(request);
        println!("Request {}: {:?}", i + 1, info);
    }

    // Performance benchmark
    let iterations = 100_000;
    println!("\n🚀 Performance Benchmark ({} iterations):", iterations);
    println!("===========================================");

    // Create larger test dataset
    let large_dataset: Vec<u8> = test_requests.iter()
        .cycle()
        .take(1000)
        .flat_map(|req| req.iter().copied())
        .collect();

    // Bit-parallel performance
    let start = Instant::now();
    for _ in 0..iterations {
        let _info = parser.parse_request(&large_dataset);
    }
    let bit_parallel_time = start.elapsed();

    // Naive performance
    let start = Instant::now();
    for _ in 0..iterations {
        let _info = naive_parse_request(&large_dataset);
    }
    let naive_time = start.elapsed();

    println!("Bit-parallel: {:?}", bit_parallel_time);
    println!("Naive search: {:?}", naive_time);
    println!("Speedup: {:.1}x faster",
        naive_time.as_nanos() as f64 / bit_parallel_time.as_nanos() as f64);

    // Throughput analysis
    let data_size_mb = large_dataset.len() as f64 / 1024.0 / 1024.0;
    let throughput = data_size_mb * iterations as f64 / bit_parallel_time.as_secs_f64();

    println!("\n📊 Throughput Analysis:");
    println!("========================");
    println!("Data size: {:.2} MB per iteration", data_size_mb);
    println!("Throughput: {:.0} MB/second", throughput);
    println!("Requests/second: {:.0}",
        (test_requests.len() * 1000) as f64 / bit_parallel_time.as_secs_f64() * iterations as f64);

    // Real-world scenario
    println!("\n🌍 Real-World Application:");
    println!("===========================");
    println!("High-frequency trading servers processing market data:");
    println!("- 1M packets/second × {}μs/packet = {}% CPU",
        bit_parallel_time.as_micros() / iterations as u128,
        (bit_parallel_time.as_micros() / iterations as u128) / 10);

    println!("\nWeb servers parsing HTTP requests:");
    println!("- Can handle {:.0} concurrent connections",
        1_000_000.0 / (bit_parallel_time.as_micros() as f64 / iterations as f64));

    // Memory usage analysis
    println!("\n💾 Memory Efficiency:");
    println!("=====================");
    println!("Parser memory: ~16KB (8 searchers × 2KB each)");
    println!("Zero allocation during parsing");
    println!("Cache-friendly sequential access");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_detection() {
        let parser = ProtocolParser::new();

        assert_eq!(parser.detect_method(b"GET /"), Method::GET);
        assert_eq!(parser.detect_method(b"POST /api"), Method::POST);
        assert_eq!(parser.detect_method(b"PUT /users"), Method::PUT);
        assert_eq!(parser.detect_method(b"DELETE /item"), Method::DELETE);
        assert_eq!(parser.detect_method(b"PATCH /"), Method::Unknown);
    }

    #[test]
    fn test_content_type_detection() {
        let parser = ProtocolParser::new();

        assert_eq!(
            parser.detect_content_type(b"Content-Type: application/json"),
            ContentType::JSON
        );
        assert_eq!(
            parser.detect_content_type(b"Content-Type: application/xml"),
            ContentType::XML
        );
        assert_eq!(
            parser.detect_content_type(b"Content-Type: text/plain"),
            ContentType::Unknown
        );
    }

    #[test]
    fn test_header_parsing() {
        let parser = ProtocolParser::new();
        let request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\nBody";

        let info = parser.parse_request(request);
        assert_eq!(info.method, Method::GET);
        assert_eq!(info.header_end, Some(36)); // Position of \r\n\r\n
        assert_eq!(info.line_count, 3); // Two \r\n in headers plus one ending headers
    }

    #[test]
    fn test_performance_parity() {
        let parser = ProtocolParser::new();
        let request = b"POST /api HTTP/1.1\r\nContent-Type: application/json\r\n\r\n";

        let bit_parallel_result = parser.parse_request(request);
        let naive_result = naive_parse_request(request);

        assert_eq!(bit_parallel_result.method, naive_result.method);
        assert_eq!(bit_parallel_result.header_end, naive_result.header_end);
        assert_eq!(bit_parallel_result.line_count, naive_result.line_count);
        assert_eq!(bit_parallel_result.content_type, naive_result.content_type);
    }
}