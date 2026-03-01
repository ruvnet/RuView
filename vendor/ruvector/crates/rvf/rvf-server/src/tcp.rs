//! TCP streaming protocol for inter-agent exchange.
//!
//! Uses a simplified length-prefixed binary framing:
//!
//! ```text
//! [4 bytes: payload length (big-endian)] [1 byte: msg_type] [3 bytes: msg_id] [payload]
//! ```
//!
//! Message types follow the spec in 10-operations-api.md section 6.2.

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use rvf_runtime::QueryOptions;

use crate::http::SharedStore;

/// TCP message types (client -> server).
const MSG_QUERY: u8 = 0x01;
const MSG_INGEST: u8 = 0x02;
const MSG_DELETE: u8 = 0x03;
const MSG_STATUS: u8 = 0x04;

/// TCP message types (server -> client).
const MSG_QUERY_RESULT: u8 = 0x81;
const MSG_INGEST_ACK: u8 = 0x82;
const MSG_DELETE_ACK: u8 = 0x83;
const MSG_STATUS_RESP: u8 = 0x84;
const MSG_ERROR: u8 = 0xFF;

/// Maximum frame payload: 16 MB.
const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Start the TCP listener on the given address.
pub async fn serve_tcp(addr: &str, store: SharedStore) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _peer) = listener.accept().await?;
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, store).await {
                eprintln!("tcp connection error: {e}");
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream, store: SharedStore) -> std::io::Result<()> {
    loop {
        // Read frame header: 4 bytes length + 1 byte msg_type + 3 bytes msg_id = 8 bytes
        let mut header = [0u8; 8];
        match stream.read_exact(&mut header).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        }

        let payload_len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let msg_type = header[4];
        let msg_id = [header[5], header[6], header[7]];

        if payload_len > MAX_FRAME_SIZE {
            send_error(&mut stream, &msg_id, 0x0104, "frame too large").await?;
            return Ok(());
        }

        let mut payload = vec![0u8; payload_len as usize];
        stream.read_exact(&mut payload).await?;

        let response = match msg_type {
            MSG_QUERY => handle_query(&payload, &store).await,
            MSG_INGEST => handle_ingest(&payload, &store).await,
            MSG_DELETE => handle_delete(&payload, &store).await,
            MSG_STATUS => handle_status(&store).await,
            _ => Err(TcpError {
                code: 0x0107,
                message: "unknown message type".into(),
            }),
        };

        match response {
            Ok((resp_type, resp_payload)) => {
                send_frame(&mut stream, resp_type, &msg_id, &resp_payload).await?;
            }
            Err(e) => {
                send_error(&mut stream, &msg_id, e.code, &e.message).await?;
            }
        }
    }
}

struct TcpError {
    code: u16,
    message: String,
}

async fn send_frame(
    stream: &mut TcpStream,
    msg_type: u8,
    msg_id: &[u8; 3],
    payload: &[u8],
) -> std::io::Result<()> {
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.push(msg_type);
    frame.extend_from_slice(msg_id);
    frame.extend_from_slice(payload);
    stream.write_all(&frame).await
}

async fn send_error(
    stream: &mut TcpStream,
    msg_id: &[u8; 3],
    code: u16,
    description: &str,
) -> std::io::Result<()> {
    let desc_bytes = description.as_bytes();
    let mut payload = Vec::with_capacity(4 + desc_bytes.len());
    payload.extend_from_slice(&code.to_le_bytes());
    payload.extend_from_slice(&(desc_bytes.len() as u16).to_le_bytes());
    payload.extend_from_slice(desc_bytes);
    send_frame(stream, MSG_ERROR, msg_id, &payload).await
}

/// Handle a QUERY message. Payload is a simplified JSON-encoded query
/// for ease of inter-agent use (vector, k as little-endian).
async fn handle_query(payload: &[u8], store: &SharedStore) -> Result<(u8, Vec<u8>), TcpError> {
    // Simplified binary protocol:
    // [4 bytes: k (LE)] [4 bytes: dim (LE)] [dim * 4 bytes: vector f32s (LE)]
    if payload.len() < 8 {
        return Err(TcpError {
            code: 0x0200,
            message: "payload too short for query".into(),
        });
    }

    let k = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let dim = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;

    let expected = 8 + dim * 4;
    if payload.len() < expected {
        return Err(TcpError {
            code: 0x0200,
            message: "payload too short for vector data".into(),
        });
    }

    let mut vector = Vec::with_capacity(dim);
    for i in 0..dim {
        let offset = 8 + i * 4;
        let val = f32::from_le_bytes([
            payload[offset],
            payload[offset + 1],
            payload[offset + 2],
            payload[offset + 3],
        ]);
        vector.push(val);
    }

    let results = {
        let s = store.lock().await;
        s.query(&vector, k, &QueryOptions::default())
            .map_err(|e| TcpError {
                code: 0x0200,
                message: format!("{e:?}"),
            })?
    };

    // Response: [4 bytes: result_count (LE)] [per result: 8 bytes id (LE) + 4 bytes dist (LE)]
    let mut resp = Vec::with_capacity(4 + results.len() * 12);
    resp.extend_from_slice(&(results.len() as u32).to_le_bytes());
    for r in &results {
        resp.extend_from_slice(&r.id.to_le_bytes());
        resp.extend_from_slice(&r.distance.to_le_bytes());
    }

    Ok((MSG_QUERY_RESULT, resp))
}

/// Handle an INGEST message.
/// Binary payload: [4 bytes: count (LE)] [2 bytes: dim (LE)] [per vector: 8 bytes id (LE) + dim*4 bytes data (LE)]
async fn handle_ingest(payload: &[u8], store: &SharedStore) -> Result<(u8, Vec<u8>), TcpError> {
    if payload.len() < 6 {
        return Err(TcpError {
            code: 0x0300,
            message: "payload too short for ingest header".into(),
        });
    }

    let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let dim = u16::from_le_bytes([payload[4], payload[5]]) as usize;

    let entry_size = 8 + dim * 4;
    let expected = 6 + count * entry_size;
    if payload.len() < expected {
        return Err(TcpError {
            code: 0x0300,
            message: "payload too short for ingest data".into(),
        });
    }

    let mut ids = Vec::with_capacity(count);
    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(count);

    let mut offset = 6;
    for _ in 0..count {
        let id = u64::from_le_bytes([
            payload[offset],
            payload[offset + 1],
            payload[offset + 2],
            payload[offset + 3],
            payload[offset + 4],
            payload[offset + 5],
            payload[offset + 6],
            payload[offset + 7],
        ]);
        offset += 8;

        let mut vec_data = Vec::with_capacity(dim);
        for _ in 0..dim {
            let val = f32::from_le_bytes([
                payload[offset],
                payload[offset + 1],
                payload[offset + 2],
                payload[offset + 3],
            ]);
            vec_data.push(val);
            offset += 4;
        }

        ids.push(id);
        vectors.push(vec_data);
    }

    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();

    let result = {
        let mut s = store.lock().await;
        s.ingest_batch(&vec_refs, &ids, None)
            .map_err(|e| TcpError {
                code: 0x0300,
                message: format!("{e:?}"),
            })?
    };

    // Response: [8 bytes: accepted (LE)] [8 bytes: rejected (LE)] [4 bytes: epoch (LE)]
    let mut resp = Vec::with_capacity(20);
    resp.extend_from_slice(&result.accepted.to_le_bytes());
    resp.extend_from_slice(&result.rejected.to_le_bytes());
    resp.extend_from_slice(&result.epoch.to_le_bytes());

    Ok((MSG_INGEST_ACK, resp))
}

/// Handle a DELETE message.
/// Binary payload: [4 bytes: count (LE)] [per id: 8 bytes (LE)]
async fn handle_delete(payload: &[u8], store: &SharedStore) -> Result<(u8, Vec<u8>), TcpError> {
    if payload.len() < 4 {
        return Err(TcpError {
            code: 0x0300,
            message: "payload too short for delete".into(),
        });
    }

    let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if payload.len() < 4 + count * 8 {
        return Err(TcpError {
            code: 0x0300,
            message: "payload too short for delete ids".into(),
        });
    }

    let mut ids = Vec::with_capacity(count);
    let mut offset = 4;
    for _ in 0..count {
        let id = u64::from_le_bytes([
            payload[offset],
            payload[offset + 1],
            payload[offset + 2],
            payload[offset + 3],
            payload[offset + 4],
            payload[offset + 5],
            payload[offset + 6],
            payload[offset + 7],
        ]);
        ids.push(id);
        offset += 8;
    }

    let result = {
        let mut s = store.lock().await;
        s.delete(&ids).map_err(|e| TcpError {
            code: 0x0300,
            message: format!("{e:?}"),
        })?
    };

    // Response: [8 bytes: deleted (LE)] [4 bytes: epoch (LE)]
    let mut resp = Vec::with_capacity(12);
    resp.extend_from_slice(&result.deleted.to_le_bytes());
    resp.extend_from_slice(&result.epoch.to_le_bytes());

    Ok((MSG_DELETE_ACK, resp))
}

/// Handle a STATUS message (no payload needed).
async fn handle_status(store: &SharedStore) -> Result<(u8, Vec<u8>), TcpError> {
    let s = store.lock().await;
    let st = s.status();

    // Simplified STATUS_RESP: epoch(4) + total_vectors(8) + total_segments(4) +
    // file_size(8) + profile_id(1) + read_only(1)
    let mut resp = Vec::with_capacity(26);
    resp.extend_from_slice(&st.current_epoch.to_le_bytes());
    resp.extend_from_slice(&st.total_vectors.to_le_bytes());
    resp.extend_from_slice(&st.total_segments.to_le_bytes());
    resp.extend_from_slice(&st.file_size.to_le_bytes());
    resp.push(st.profile_id);
    resp.push(if st.read_only { 1 } else { 0 });

    Ok((MSG_STATUS_RESP, resp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvf_runtime::{RvfOptions, RvfStore};
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    fn create_test_store() -> (TempDir, SharedStore) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.rvf");
        let options = RvfOptions {
            dimension: 4,
            ..Default::default()
        };
        let store = RvfStore::create(&path, options).unwrap();
        (dir, Arc::new(Mutex::new(store)))
    }

    fn build_frame(msg_type: u8, msg_id: [u8; 3], payload: &[u8]) -> Vec<u8> {
        let len = payload.len() as u32;
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&len.to_be_bytes());
        frame.push(msg_type);
        frame.extend_from_slice(&msg_id);
        frame.extend_from_slice(payload);
        frame
    }

    async fn read_frame(stream: &mut TcpStream) -> (u8, [u8; 3], Vec<u8>) {
        let mut header = [0u8; 8];
        stream.read_exact(&mut header).await.unwrap();
        let payload_len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let msg_type = header[4];
        let msg_id = [header[5], header[6], header[7]];
        let mut payload = vec![0u8; payload_len as usize];
        stream.read_exact(&mut payload).await.unwrap();
        (msg_type, msg_id, payload)
    }

    #[tokio::test]
    async fn test_tcp_status() {
        let (_dir, store) = create_test_store();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let store_clone = Arc::clone(&store);
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, store_clone).await.unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();

        // Send STATUS request (no payload)
        let frame = build_frame(MSG_STATUS, [0, 0, 1], &[]);
        client.write_all(&frame).await.unwrap();

        let (msg_type, msg_id, payload) = read_frame(&mut client).await;
        assert_eq!(msg_type, MSG_STATUS_RESP);
        assert_eq!(msg_id, [0, 0, 1]);
        assert_eq!(payload.len(), 26);
    }

    #[tokio::test]
    async fn test_tcp_ingest_and_query() {
        let (_dir, store) = create_test_store();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let store_clone = Arc::clone(&store);
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, store_clone).await.unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();

        // Ingest 2 vectors of dim=4
        let mut ingest_payload = Vec::new();
        ingest_payload.extend_from_slice(&2u32.to_le_bytes()); // count
        ingest_payload.extend_from_slice(&4u16.to_le_bytes()); // dim
                                                               // Vector 1: id=1, data=[1,0,0,0]
        ingest_payload.extend_from_slice(&1u64.to_le_bytes());
        ingest_payload.extend_from_slice(&1.0f32.to_le_bytes());
        ingest_payload.extend_from_slice(&0.0f32.to_le_bytes());
        ingest_payload.extend_from_slice(&0.0f32.to_le_bytes());
        ingest_payload.extend_from_slice(&0.0f32.to_le_bytes());
        // Vector 2: id=2, data=[0,1,0,0]
        ingest_payload.extend_from_slice(&2u64.to_le_bytes());
        ingest_payload.extend_from_slice(&0.0f32.to_le_bytes());
        ingest_payload.extend_from_slice(&1.0f32.to_le_bytes());
        ingest_payload.extend_from_slice(&0.0f32.to_le_bytes());
        ingest_payload.extend_from_slice(&0.0f32.to_le_bytes());

        let frame = build_frame(MSG_INGEST, [0, 0, 2], &ingest_payload);
        client.write_all(&frame).await.unwrap();

        let (msg_type, _, payload) = read_frame(&mut client).await;
        assert_eq!(msg_type, MSG_INGEST_ACK);
        let accepted = u64::from_le_bytes(payload[0..8].try_into().unwrap());
        assert_eq!(accepted, 2);

        // Query for nearest to [1,0,0,0], k=1
        let mut query_payload = Vec::new();
        query_payload.extend_from_slice(&1u32.to_le_bytes()); // k
        query_payload.extend_from_slice(&4u32.to_le_bytes()); // dim
        query_payload.extend_from_slice(&1.0f32.to_le_bytes());
        query_payload.extend_from_slice(&0.0f32.to_le_bytes());
        query_payload.extend_from_slice(&0.0f32.to_le_bytes());
        query_payload.extend_from_slice(&0.0f32.to_le_bytes());

        let frame = build_frame(MSG_QUERY, [0, 0, 3], &query_payload);
        client.write_all(&frame).await.unwrap();

        let (msg_type, _, payload) = read_frame(&mut client).await;
        assert_eq!(msg_type, MSG_QUERY_RESULT);
        let count = u32::from_le_bytes(payload[0..4].try_into().unwrap());
        assert_eq!(count, 1);
        let id = u64::from_le_bytes(payload[4..12].try_into().unwrap());
        assert_eq!(id, 1);
    }
}
