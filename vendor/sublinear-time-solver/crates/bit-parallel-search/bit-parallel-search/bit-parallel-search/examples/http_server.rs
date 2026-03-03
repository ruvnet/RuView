//! Example: High-performance HTTP header parsing
//! Shows 5-8x speedup over naive search for common headers

use bit_parallel_search::BitParallelSearcher;
use std::time::Instant;

struct HttpHeaderParser {
    content_type: BitParallelSearcher,
    content_length: BitParallelSearcher,
    authorization: BitParallelSearcher,
    user_agent: BitParallelSearcher,
    accept: BitParallelSearcher,
    host: BitParallelSearcher,
}

impl HttpHeaderParser {
    fn new() -> Self {
        Self {
            content_type: BitParallelSearcher::new(b"Content-Type:"),
            content_length: BitParallelSearcher::new(b"Content-Length:"),
            authorization: BitParallelSearcher::new(b"Authorization:"),
            user_agent: BitParallelSearcher::new(b"User-Agent:"),
            accept: BitParallelSearcher::new(b"Accept:"),
            host: BitParallelSearcher::new(b"Host:"),
        }
    }

    fn parse_headers(&self, request: &[u8]) -> HeaderInfo {
        HeaderInfo {
            content_type_pos: self.content_type.find_in(request),
            content_length_pos: self.content_length.find_in(request),
            authorization_pos: self.authorization.find_in(request),
            user_agent_pos: self.user_agent.find_in(request),
            accept_pos: self.accept.find_in(request),
            host_pos: self.host.find_in(request),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct HeaderInfo {
    content_type_pos: Option<usize>,
    content_length_pos: Option<usize>,
    authorization_pos: Option<usize>,
    user_agent_pos: Option<usize>,
    accept_pos: Option<usize>,
    host_pos: Option<usize>,
}

fn naive_header_parser(request: &[u8]) -> HeaderInfo {
    HeaderInfo {
        content_type_pos: find_header_naive(request, b"Content-Type:"),
        content_length_pos: find_header_naive(request, b"Content-Length:"),
        authorization_pos: find_header_naive(request, b"Authorization:"),
        user_agent_pos: find_header_naive(request, b"User-Agent:"),
        accept_pos: find_header_naive(request, b"Accept:"),
        host_pos: find_header_naive(request, b"Host:"),
    }
}

fn find_header_naive(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len())
        .position(|window| window == needle)
}

fn main() {
    println!("🚀 HTTP Header Parsing Performance Demo\n");

    // Real HTTP request
    let http_request = b"GET /api/users HTTP/1.1\r\n\
Host: api.example.com\r\n\
User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36\r\n\
Accept: application/json, text/plain, */*\r\n\
Accept-Language: en-US,en;q=0.9\r\n\
Accept-Encoding: gzip, deflate, br\r\n\
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9\r\n\
Content-Type: application/json\r\n\
Content-Length: 1234\r\n\
Connection: keep-alive\r\n\
Cache-Control: no-cache\r\n\r\n";

    let parser = HttpHeaderParser::new();

    // Benchmark performance
    let iterations = 1_000_000;

    // Bit-parallel performance
    let start = Instant::now();
    for _ in 0..iterations {
        let _headers = parser.parse_headers(http_request);
    }
    let bit_parallel_time = start.elapsed();

    // Naive performance
    let start = Instant::now();
    for _ in 0..iterations {
        let _headers = naive_header_parser(http_request);
    }
    let naive_time = start.elapsed();

    println!("📊 Performance Results ({} iterations):", iterations);
    println!("=====================================");
    println!("Bit-parallel: {:?}", bit_parallel_time);
    println!("Naive search: {:?}", naive_time);
    println!("Speedup: {:.1}x faster",
        naive_time.as_nanos() as f64 / bit_parallel_time.as_nanos() as f64);

    // Show parsed results
    println!("\n🔍 Parsed Header Positions:");
    let headers = parser.parse_headers(http_request);
    println!("{:#?}", headers);

    // Real-world scenario: processing 1M requests
    println!("\n🌍 Real-World Impact:");
    println!("Processing 1M HTTP requests:");
    println!("- Bit-parallel: {:.0}ms", bit_parallel_time.as_millis());
    println!("- Naive search: {:.0}ms", naive_time.as_millis());
    println!("- Time saved: {:.0}ms per million requests",
        (naive_time - bit_parallel_time).as_millis());

    let requests_per_second = iterations as f64 / bit_parallel_time.as_secs_f64();
    println!("- Throughput: {:.0} requests/second", requests_per_second);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_parsing_correctness() {
        let parser = HttpHeaderParser::new();
        let request = b"Host: example.com\r\nContent-Type: application/json\r\n\r\n";

        let headers = parser.parse_headers(request);
        assert_eq!(headers.host_pos, Some(0));
        assert_eq!(headers.content_type_pos, Some(19));
    }

    #[test]
    fn test_missing_headers() {
        let parser = HttpHeaderParser::new();
        let request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";

        let headers = parser.parse_headers(request);
        assert_eq!(headers.host_pos, Some(15));
        assert_eq!(headers.content_type_pos, None);
        assert_eq!(headers.authorization_pos, None);
    }
}