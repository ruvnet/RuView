//! RuVector Desktop — wry webview wrapping the Causal Atlas dashboard.
//!
//! Embeds the entire Vite-built dashboard (HTML, JS, CSS, WASM) at compile
//! time via rust-embed, serves it from a tiny background HTTP server, and
//! opens a native webview window. Single binary, no external dependencies.

use rust_embed::Embed;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// All files from `../../rvf/dashboard/dist/` are embedded at compile time.
/// This includes index.html, JS chunks, CSS, and the WASM solver binary.
#[derive(Embed)]
#[folder = "../rvf/dashboard/dist/"]
struct DashboardAssets;

/// Find an available port to serve on.
fn find_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().unwrap().port()
}

/// Start a tiny HTTP server that serves embedded dashboard files.
fn start_asset_server(port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let server = Arc::new(tiny_http::Server::http(&addr).expect("start HTTP server"));

    // Spawn worker threads to handle requests
    for _ in 0..2 {
        let srv = Arc::clone(&server);
        thread::spawn(move || {
            loop {
                let request = match srv.recv() {
                    Ok(r) => r,
                    Err(_) => break,
                };

                let url_path = request.url().trim_start_matches('/');
                let path = if url_path.is_empty() { "index.html" } else { url_path };

                // Try to find the embedded file
                let response = match DashboardAssets::get(path) {
                    Some(file) => {
                        let mime = mime_guess::from_path(path).first_or_octet_stream();
                        let data = file.data.to_vec();
                        tiny_http::Response::from_data(data)
                            .with_header(
                                tiny_http::Header::from_bytes(
                                    b"Content-Type",
                                    mime.as_ref().as_bytes(),
                                )
                                .unwrap(),
                            )
                            .with_header(
                                tiny_http::Header::from_bytes(
                                    b"Access-Control-Allow-Origin",
                                    b"*",
                                )
                                .unwrap(),
                            )
                    }
                    None => {
                        // SPA fallback: serve index.html for hash-routed paths
                        match DashboardAssets::get("index.html") {
                            Some(file) => {
                                let data = file.data.to_vec();
                                tiny_http::Response::from_data(data).with_header(
                                    tiny_http::Header::from_bytes(
                                        b"Content-Type",
                                        b"text/html; charset=utf-8",
                                    )
                                    .unwrap(),
                                )
                            }
                            None => tiny_http::Response::from_string("Not Found")
                                .with_status_code(404),
                        }
                    }
                };

                let _ = request.respond(response);
            }
        });
    }
}

fn main() {
    let port = find_port();
    let url = format!("http://127.0.0.1:{port}");

    // Start embedded asset server in background
    start_asset_server(port);

    // Give server a moment to bind
    thread::sleep(std::time::Duration::from_millis(50));

    // Create native window + webview
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("RuVector — Causal Atlas")
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .with_min_inner_size(LogicalSize::new(800.0, 500.0))
        .build(&event_loop)
        .expect("create window");

    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .with_devtools(cfg!(debug_assertions))
        .with_initialization_script(
            r#"
            // Inject desktop app context so the dashboard knows it's running natively
            window.__RUVECTOR_DESKTOP__ = true;
            window.__RUVECTOR_VERSION__ = '2.0.0';
            "#,
        )
        .build(&window)
        .expect("create webview");

    println!("RuVector Desktop v2.0.0");
    println!("  Dashboard: {url}");
    println!("  Window: 1400x900");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
