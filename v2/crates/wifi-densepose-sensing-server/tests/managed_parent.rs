//! Real isolated child processes and sockets; no physical sensing claims.
#![cfg(unix)]

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    process::{Child, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

struct Fixture {
    parent: Child,
    helper: u32,
    http: u16,
    ws: u16,
    directory: tempfile::TempDir,
}

fn signal(pid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status();
}

fn running(pid: u32) -> bool {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "stat="])
        .output()
        .unwrap();
    let state = String::from_utf8_lossy(&output.stdout);
    !state.trim().is_empty() && !state.trim().starts_with('Z')
}

impl Fixture {
    fn start(guarded: bool) -> Self {
        let a = TcpListener::bind("127.0.0.1:0").unwrap();
        let b = TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (http, ws, udp_port) = (
            a.local_addr().unwrap().port(),
            b.local_addr().unwrap().port(),
            udp.local_addr().unwrap().port(),
        );
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("retained-record.json"),
            b"synthetic-retained-record",
        )
        .unwrap();
        let script = if guarded {
            "\"$@\" --parent-pid \"$$\" >/dev/null & child=$!; printf '%s\\n' \"$child\"; wait \"$child\""
        } else {
            "\"$@\" >/dev/null & child=$!; printf '%s\\n' \"$child\"; wait \"$child\""
        };
        drop((a, b, udp));
        let mut parent = Command::new("/bin/sh")
            .args([
                "-c",
                script,
                "managed-parent-test",
                env!("CARGO_BIN_EXE_sensing-server"),
            ])
            .args([
                "--source",
                "esp32",
                "--bind-addr",
                "127.0.0.1",
                "--udp-bind",
                "127.0.0.1",
                "--no-mdns",
                "--http-port",
                &http.to_string(),
                "--ws-port",
                &ws.to_string(),
                "--udp-port",
                &udp_port.to_string(),
                "--data-dir",
                directory.path().to_str().unwrap(),
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("RUST_LOG", "warn")
            .stdout(Stdio::piped())
            .stderr(std::fs::File::create(directory.path().join("helper.log")).unwrap())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(parent.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let fixture = Self {
            parent,
            helper: line.trim().parse().unwrap(),
            http,
            ws,
            directory,
        };
        let deadline = Instant::now() + Duration::from_secs(8);
        while !fixture.healthy() {
            assert!(
                Instant::now() < deadline,
                "helper did not start: {}",
                std::fs::read_to_string(fixture.directory.path().join("helper.log")).unwrap()
            );
            sleep(Duration::from_millis(25));
        }
        fixture
    }

    fn healthy(&self) -> bool {
        let Ok(mut stream) = TcpStream::connect(("127.0.0.1", self.http)) else {
            return false;
        };
        stream
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        if stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .is_err()
        {
            return false;
        }
        let mut response = [0; 64];
        stream
            .read(&mut response)
            .is_ok_and(|count| response[..count].starts_with(b"HTTP/1.1 200"))
    }

    fn websocket(&self, port: u16) -> TcpStream {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream.write_all(b"GET /ws/sensing HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").unwrap();
        let mut response = [0; 1024];
        let count = stream.read(&mut response).unwrap();
        assert!(response[..count].starts_with(b"HTTP/1.1 101"));
        stream
    }

    fn end_parent(&mut self, how: &str) {
        signal(self.parent.id(), how);
        self.parent.wait().unwrap();
    }

    fn assert_stopped(&self) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while running(self.helper) {
            assert!(
                Instant::now() < deadline,
                "managed helper survived its parent"
            );
            sleep(Duration::from_millis(25));
        }
        assert!(TcpStream::connect(("127.0.0.1", self.http)).is_err());
        assert!(TcpStream::connect(("127.0.0.1", self.ws)).is_err());
        assert_eq!(
            std::fs::read(self.directory.path().join("retained-record.json")).unwrap(),
            b"synthetic-retained-record"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only these fixture-created processes; never enumerate a user's ports.
        if running(self.helper) {
            signal(self.helper, "-KILL");
        }
        if self.parent.try_wait().unwrap().is_none() {
            let _ = self.parent.kill();
        }
        let _ = self.parent.wait();
    }
}

#[test]
fn managed_helper_stops_after_parent_force_quit_even_with_an_open_websocket() {
    let mut fixture = Fixture::start(true);
    let _open_socket = fixture.websocket(fixture.ws);
    let _open_http_socket = fixture.websocket(fixture.http);
    fixture.end_parent("-KILL");
    fixture.assert_stopped();
}

#[test]
fn managed_helper_stops_after_parent_termination() {
    let mut fixture = Fixture::start(true);
    fixture.end_parent("-TERM");
    fixture.assert_stopped();
}

#[test]
fn unmanaged_server_survives_parent_exit() {
    let mut fixture = Fixture::start(false);
    fixture.end_parent("-KILL");
    sleep(Duration::from_millis(500));
    assert!(running(fixture.helper));
    assert!(fixture.healthy());
}

#[test]
fn managed_helper_remains_available_while_its_parent_lives() {
    let fixture = Fixture::start(true);
    sleep(Duration::from_millis(500));
    assert!(running(fixture.helper));
    assert!(fixture.healthy());
}

#[test]
fn wrong_parent_identity_is_rejected_before_creating_state() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("must-not-be-created");
    for parent in ["0", "1", "4294967295"] {
        let result = Command::new(env!("CARGO_BIN_EXE_sensing-server"))
            .args([
                "--parent-pid",
                parent,
                "--data-dir",
                state.to_str().unwrap(),
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!state.exists());
    }
}

// Subprocess fixture for the production watchdog when its async receiver is
// never polled. Ordinary test execution does not arm or park this fixture.
#[test]
fn watchdog_fixture() {
    let Ok(parent) = std::env::var("RUVIEW_TEST_PARENT_PID") else {
        return;
    };
    let (_guard, _unpolled_receiver) =
        wifi_densepose_sensing_server::parent_lifetime::arm(Some(parent.parse().unwrap())).unwrap();
    std::fs::write(std::env::var("RUVIEW_TEST_READY_FILE").unwrap(), b"ready").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn watchdog_bounds_exit_when_graceful_shutdown_cannot_run() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("retained-record.json"),
        b"synthetic-retained-record",
    )
    .unwrap();
    let ready = directory.path().join("ready");
    let mut parent = Command::new("/bin/sh")
        .args(["-c", "RUVIEW_TEST_PARENT_PID=\"$$\" \"$@\" --exact watchdog_fixture --nocapture >/dev/null & child=$!; printf '%s\\n' \"$child\"; wait \"$child\"", "watchdog-parent"])
        .arg(std::env::current_exe().unwrap())
        .env_clear().env("PATH", "/usr/bin:/bin").env("RUVIEW_TEST_READY_FILE", &ready)
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(parent.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let mut fixture = Fixture {
        parent,
        helper: line.trim().parse().unwrap(),
        http: 0,
        ws: 0,
        directory,
    };
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready.exists() {
        assert!(Instant::now() < deadline, "watchdog fixture did not arm");
        sleep(Duration::from_millis(25));
    }
    let started = Instant::now();
    fixture.end_parent("-KILL");
    fixture.assert_stopped();
    assert!(
        started.elapsed() >= Duration::from_secs(2),
        "fixture must exercise the hard deadline, not async shutdown"
    );
}
