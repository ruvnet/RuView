//! MTC1 output: encode a [`CsiFrame`] and hand it to the sensing server.
//!
//! The sensing server multiplexes its UDP ingest on the first four bytes, so an
//! MTC1 frame needs no extra framing — `CsiFrame::to_bytes` already begins with
//! `MEDIATEK_CSI_MAGIC` ("MTC1"), which is what the server dispatches on.

use std::net::{ToSocketAddrs, UdpSocket};

use wifi_densepose_hardware::mediatek_csi::CsiFrame;

use crate::BridgeError;

/// Where assembled frames go.
pub trait FrameSink {
    fn send(&mut self, frame: &CsiFrame) -> Result<usize, BridgeError>;
}

/// Sends MTC1 frames to the sensing server's UDP ingest (default `:5005`).
pub struct UdpFrameSink {
    socket: UdpSocket,
}

impl UdpFrameSink {
    pub fn connect<A: ToSocketAddrs>(target: A) -> Result<Self, BridgeError> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(BridgeError::Io)?;
        socket.connect(target).map_err(BridgeError::Io)?;
        Ok(Self { socket })
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, BridgeError> {
        self.socket.local_addr().map_err(BridgeError::Io)
    }
}

impl FrameSink for UdpFrameSink {
    fn send(&mut self, frame: &CsiFrame) -> Result<usize, BridgeError> {
        let bytes = frame.to_bytes().map_err(BridgeError::Mtc1)?;
        self.socket.send(&bytes).map_err(BridgeError::Io)
    }
}

/// Collects encoded frames in memory. Used by tests and by `--dry-run`.
#[derive(Debug, Default)]
pub struct VecFrameSink {
    pub frames: Vec<Vec<u8>>,
}

impl FrameSink for VecFrameSink {
    fn send(&mut self, frame: &CsiFrame) -> Result<usize, BridgeError> {
        let bytes = frame.to_bytes().map_err(BridgeError::Mtc1)?;
        let n = bytes.len();
        self.frames.push(bytes);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_hardware::mediatek_csi::{CsiFrame, MEDIATEK_CSI_MAGIC};

    use crate::assemble::{AssemblerConfig, FrameAssembler};
    use crate::record::MtkRecord;

    fn one_frame() -> CsiFrame {
        let mut a = FrameAssembler::new(AssemblerConfig {
            device_id: 5,
            ..Default::default()
        });
        let mut out = None;
        for (tx, rx) in [(0u16, 0u16), (0, 1), (1, 0), (1, 1)] {
            out = a
                .push(MtkRecord {
                    ts: 1,
                    ta: [0; 6],
                    rssi: Some(-40),
                    snr: Some(30),
                    bw_code: Some(0),
                    pri_ch_idx: 0,
                    rx_mode: Some(8),
                    tx_idx: tx,
                    rx_idx: rx,
                    chain_info: 0,
                    ext_info: 0,
                    pkt_sn: None,
                    data_i: vec![1; 64],
                    data_q: vec![2; 64],
                    trimmed: false,
                })
                .unwrap();
        }
        out.unwrap()
    }

    #[test]
    fn encoded_frame_starts_with_the_mtc1_magic_the_server_dispatches_on() {
        let mut sink = VecFrameSink::default();
        sink.send(&one_frame()).unwrap();
        let bytes = &sink.frames[0];
        assert_eq!(
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            MEDIATEK_CSI_MAGIC
        );
        assert_eq!(CsiFrame::from_bytes(bytes).unwrap().0, one_frame());
    }

    #[test]
    fn udp_sink_delivers_a_decodable_frame_over_the_wire() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut sink = UdpFrameSink::connect(addr).unwrap();
        let frame = one_frame();
        let sent = sink.send(&frame).unwrap();

        let mut buf = vec![0u8; 65_535];
        let (n, _) = server.recv_from(&mut buf).unwrap();
        assert_eq!(n, sent);
        let (decoded, _) = CsiFrame::from_bytes(&buf[..n]).unwrap();
        assert_eq!(decoded, frame);
    }
}
