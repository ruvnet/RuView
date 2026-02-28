//! QMP (QEMU Machine Protocol) client.
//!
//! Implements just enough of the QMP JSON protocol to negotiate
//! capabilities and issue `system_powerdown` / `quit` commands for
//! graceful or forced VM shutdown.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::error::LaunchError;

/// A minimal QMP client connected via a Unix socket.
pub struct QmpClient {
    stream: UnixStream,
}

impl QmpClient {
    /// Connect to the QMP Unix socket and perform the capability
    /// negotiation handshake.
    pub fn connect(socket_path: &Path, timeout: Duration) -> Result<Self, LaunchError> {
        let stream = UnixStream::connect(socket_path).map_err(LaunchError::QmpIo)?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(LaunchError::QmpIo)?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(LaunchError::QmpIo)?;

        let mut client = Self { stream };

        // Read the server greeting (QMP banner).
        let greeting = client.read_line()?;
        if !greeting.contains("\"QMP\"") {
            return Err(LaunchError::Qmp(format!(
                "unexpected QMP greeting: {greeting}"
            )));
        }

        // Negotiate capabilities.
        client.send_command(r#"{"execute":"qmp_capabilities"}"#)?;
        let resp = client.read_line()?;
        if !resp.contains("\"return\"") {
            return Err(LaunchError::Qmp(format!("qmp_capabilities failed: {resp}")));
        }

        Ok(client)
    }

    /// Send `system_powerdown` for a graceful ACPI shutdown.
    pub fn system_powerdown(&mut self) -> Result<(), LaunchError> {
        self.send_command(r#"{"execute":"system_powerdown"}"#)?;
        let resp = self.read_line()?;
        if resp.contains("\"error\"") {
            return Err(LaunchError::Qmp(format!("system_powerdown failed: {resp}")));
        }
        Ok(())
    }

    /// Send `quit` to force QEMU to exit immediately.
    pub fn quit(&mut self) -> Result<(), LaunchError> {
        self.send_command(r#"{"execute":"quit"}"#)?;
        // QEMU may close the socket before we can read the response.
        let _ = self.read_line();
        Ok(())
    }

    /// Send `query-status` to check the VM's run state.
    pub fn query_status(&mut self) -> Result<String, LaunchError> {
        self.send_command(r#"{"execute":"query-status"}"#)?;
        self.read_line()
    }

    fn send_command(&mut self, cmd: &str) -> Result<(), LaunchError> {
        self.stream
            .write_all(cmd.as_bytes())
            .map_err(LaunchError::QmpIo)?;
        self.stream.write_all(b"\n").map_err(LaunchError::QmpIo)?;
        self.stream.flush().map_err(LaunchError::QmpIo)?;
        Ok(())
    }

    fn read_line(&mut self) -> Result<String, LaunchError> {
        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(LaunchError::QmpIo)?;
        Ok(line)
    }
}

#[cfg(test)]
mod tests {
    // QMP tests require a running QEMU instance, so we only test
    // construction logic here. Full integration tests belong in
    // tests/rvf-integration.
    #[test]
    fn connect_to_nonexistent_socket_fails() {
        use super::*;
        let result = QmpClient::connect(
            Path::new("/tmp/nonexistent_qmp.sock"),
            Duration::from_secs(1),
        );
        assert!(result.is_err());
    }
}
