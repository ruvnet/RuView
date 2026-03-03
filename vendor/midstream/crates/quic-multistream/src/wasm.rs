//! WASM implementation using WebTransport

use crate::{ConnectionStats, QuicError, StreamPriority};
use js_sys::{Uint8Array, Promise};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    WebTransport, WebTransportBidirectionalStream, WebTransportSendStream,
    WebTransportReceiveStream,
};

/// QUIC connection wrapper for WASM targets using WebTransport
pub struct QuicConnection {
    transport: WebTransport,
    bytes_sent: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
}

impl QuicConnection {
    /// Connect to a WebTransport server
    ///
    /// # Arguments
    /// * `url` - Server URL (must use HTTPS, e.g., "https://server.example.com")
    ///
    /// # Examples
    /// ```no_run
    /// # #[cfg(target_arch = "wasm32")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use midstreamer_quic::QuicConnection;
    ///
    /// let connection = QuicConnection::connect("https://localhost:4433").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(url: &str) -> Result<Self, QuicError> {
        // Create WebTransport
        let transport = WebTransport::new(url)
            .map_err(|e| QuicError::ConnectionFailed(format!("{:?}", e)))?;

        // Wait for connection to be ready
        let ready_promise = transport.ready();
        JsFuture::from(ready_promise)
            .await
            .map_err(|e| QuicError::ConnectionFailed(format!("{:?}", e)))?;

        Ok(Self {
            transport,
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Open a bidirectional stream
    pub async fn open_bi_stream(&self) -> Result<QuicStream, QuicError> {
        let create_stream = self.transport.create_bidirectional_stream();
        let stream_js = JsFuture::from(create_stream)
            .await
            .map_err(|e| QuicError::StreamError(format!("{:?}", e)))?;

        let bi_stream = WebTransportBidirectionalStream::from(stream_js);

        Ok(QuicStream::new(
            bi_stream,
            self.bytes_sent.clone(),
            self.bytes_received.clone(),
        ))
    }

    /// Open a bidirectional stream with priority
    pub async fn open_bi_stream_with_priority(
        &self,
        priority: StreamPriority,
    ) -> Result<QuicStream, QuicError> {
        let mut stream = self.open_bi_stream().await?;
        stream.set_priority(priority);
        Ok(stream)
    }

    /// Open a unidirectional stream (send-only)
    pub async fn open_uni_stream(&self) -> Result<QuicSendStream, QuicError> {
        let create_stream = self.transport.create_unidirectional_stream();
        let stream_js = JsFuture::from(create_stream)
            .await
            .map_err(|e| QuicError::StreamError(format!("{:?}", e)))?;

        let send_stream = WebTransportSendStream::from(stream_js);

        Ok(QuicSendStream::new(send_stream, self.bytes_sent.clone()))
    }

    /// Accept an incoming bidirectional stream
    pub async fn accept_bi_stream(&self) -> Result<QuicStream, QuicError> {
        // WebTransport uses an async iterator for incoming streams
        // This is a simplified implementation
        Err(QuicError::StreamError(
            "accept_bi_stream not yet implemented for WASM".to_string(),
        ))
    }

    /// Get connection statistics
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            active_bi_streams: 0, // Not easily available in WebTransport
            active_uni_streams: 0,
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            rtt_ms: 0.0, // Not available in WebTransport API
        }
    }

    /// Close the connection
    pub fn close(&self, _error_code: u64, _reason: &[u8]) {
        self.transport.close();
    }
}

/// Bidirectional QUIC stream for WASM
pub struct QuicStream {
    bi_stream: WebTransportBidirectionalStream,
    priority: StreamPriority,
    bytes_sent: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
}

impl QuicStream {
    fn new(
        bi_stream: WebTransportBidirectionalStream,
        bytes_sent: Arc<AtomicU64>,
        bytes_received: Arc<AtomicU64>,
    ) -> Self {
        Self {
            bi_stream,
            priority: StreamPriority::default(),
            bytes_sent,
            bytes_received,
        }
    }

    /// Send data on the stream
    pub async fn send(&mut self, data: &[u8]) -> Result<usize, QuicError> {
        let writable = self.bi_stream.writable();
        let writer = writable.get_writer()
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        // Convert data to Uint8Array
        let uint8_array = Uint8Array::new_with_length(data.len() as u32);
        uint8_array.copy_from(data);

        // Write to stream
        let write_promise = writer.write_with_chunk(&uint8_array)
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        JsFuture::from(write_promise)
            .await
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        // Release writer
        writer.release_lock();

        self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(data.len())
    }

    /// Receive data from the stream
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, QuicError> {
        let readable = self.bi_stream.readable();
        let reader = readable.get_reader()
            .map_err(|e| QuicError::RecvError(format!("{:?}", e)))?;

        // Read from stream
        let read_promise = reader.read();
        let read_result = JsFuture::from(read_promise)
            .await
            .map_err(|e| QuicError::RecvError(format!("{:?}", e)))?;

        // Extract data from result
        let done = js_sys::Reflect::get(&read_result, &JsValue::from_str("done"))
            .map_err(|e| QuicError::RecvError(format!("{:?}", e)))?
            .as_bool()
            .unwrap_or(false);

        if done {
            reader.release_lock();
            return Ok(0);
        }

        let value = js_sys::Reflect::get(&read_result, &JsValue::from_str("value"))
            .map_err(|e| QuicError::RecvError(format!("{:?}", e)))?;

        let uint8_array = Uint8Array::from(value);
        let len = uint8_array.length() as usize;
        let to_copy = len.min(buf.len());

        uint8_array.copy_to(&mut buf[..to_copy]);

        reader.release_lock();

        self.bytes_received.fetch_add(to_copy as u64, Ordering::Relaxed);
        Ok(to_copy)
    }

    /// Finish sending on this stream
    pub async fn finish(&mut self) -> Result<(), QuicError> {
        let writable = self.bi_stream.writable();
        let writer = writable.get_writer()
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        let close_promise = writer.close()
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        JsFuture::from(close_promise)
            .await
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        Ok(())
    }

    /// Set stream priority
    pub fn set_priority(&mut self, priority: StreamPriority) {
        self.priority = priority;
        // WebTransport doesn't expose priority directly
    }

    /// Get current priority
    pub fn priority(&self) -> StreamPriority {
        self.priority
    }
}

/// Unidirectional send-only stream for WASM
pub struct QuicSendStream {
    send_stream: WebTransportSendStream,
    bytes_sent: Arc<AtomicU64>,
}

impl QuicSendStream {
    fn new(send_stream: WebTransportSendStream, bytes_sent: Arc<AtomicU64>) -> Self {
        Self {
            send_stream,
            bytes_sent,
        }
    }

    /// Send data on the stream
    pub async fn send(&mut self, data: &[u8]) -> Result<usize, QuicError> {
        let writer = self.send_stream.get_writer()
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        // Convert data to Uint8Array
        let uint8_array = Uint8Array::new_with_length(data.len() as u32);
        uint8_array.copy_from(data);

        // Write to stream
        let write_promise = writer.write_with_chunk(&uint8_array)
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        JsFuture::from(write_promise)
            .await
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        writer.release_lock();

        self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(data.len())
    }

    /// Finish sending on this stream
    pub async fn finish(&mut self) -> Result<(), QuicError> {
        let writer = self.send_stream.get_writer()
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        let close_promise = writer.close()
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        JsFuture::from(close_promise)
            .await
            .map_err(|e| QuicError::SendError(format!("{:?}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_in_wasm_stream() {
        // Can't easily test full WASM functionality in unit tests
        // These would require browser environment
        assert_eq!(StreamPriority::default(), StreamPriority::Normal);
    }

    #[test]
    fn test_stats_tracking() {
        let bytes_sent = Arc::new(AtomicU64::new(0));
        let bytes_received = Arc::new(AtomicU64::new(0));

        bytes_sent.fetch_add(100, Ordering::Relaxed);
        bytes_received.fetch_add(200, Ordering::Relaxed);

        assert_eq!(bytes_sent.load(Ordering::Relaxed), 100);
        assert_eq!(bytes_received.load(Ordering::Relaxed), 200);
    }
}
