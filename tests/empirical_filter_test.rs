//! Empirical integration test for WebKitGTK 6.0 content filtering and block observability
//!
//! Rigorously verifies:
//! 1. CONTROL RUN: Proves that without content filters, the test page requests all subresources
//!    (/test_page.html, /content/normal.js, /ads/banner.js, /track/pixel.gif) from the local server.
//! 2. FILTERED RUN: Proves that with WebKitUserContentFilter active, blocked subresources
//!    (/ads/banner.js, /track/pixel.gif) NEVER reach the network/server.
//! 3. OBSERVABILITY: Proves whether WebKitGTK 6.0 emits any signal (resource_load_started or
//!    connect_failed) to the UI process when a subresource is blocked by a content filter.

use gtk4::glib;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn test_webkit_content_blocking_and_observability() {
    // 0. Print test configuration, HTML, and filter rules upfront
    let test_filter_json = r#"[
        {"trigger": {"url-filter": ".*/ads/.*"}, "action": {"type": "block"}},
        {"trigger": {"url-filter": ".*/track/.*"}, "action": {"type": "block"}}
    ]"#;

    println!("\n======================================================================");
    println!("EMPIRICAL WEBKITGTK 6.0 CONTENT FILTER & OBSERVABILITY TEST");
    println!("======================================================================");
    println!("FILTER RULES USED (WebKit JSON format):");
    println!("{test_filter_json}");
    println!("EXPECTED ALLOWED PATHS:");
    println!("  - /test_page.html (HTML main frame)");
    println!("  - /content/normal.js (ordinary subresource script)");
    println!("EXPECTED BLOCKED PATHS (when filter is applied):");
    println!("  - /ads/banner.js (matches .*/ads/.*)");
    println!("  - /track/pixel.gif (matches .*/track/.*)");
    println!("======================================================================\n");

    // 1. Start local HTTP server on an available port
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test server");
    let port = listener.local_addr().unwrap().port();

    let control_paths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let filtered_paths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let current_phase = Arc::new(Mutex::new("control".to_string()));

    let srv_control_paths = control_paths.clone();
    let srv_filtered_paths = filtered_paths.clone();
    let srv_phase = current_phase.clone();
    let server_running = Arc::new(AtomicBool::new(true));
    let srv_stop = server_running.clone();

    // Background HTTP server thread
    thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        while srv_stop.load(Ordering::SeqCst) {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                if let Ok(n) = stream.read(&mut buf) {
                    let req = String::from_utf8_lossy(&buf[..n]);
                    if let Some(first_line) = req.lines().next() {
                        let parts: Vec<&str> = first_line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let path = parts[1].to_string();
                            let phase = srv_phase.lock().unwrap().clone();
                            if phase == "control" {
                                srv_control_paths.lock().unwrap().push(path.clone());
                            } else {
                                srv_filtered_paths.lock().unwrap().push(path.clone());
                            }

                            let (content_type, body): (&str, Vec<u8>) = match path.as_str() {
                                "/test_page.html" => (
                                    "text/html; charset=utf-8",
                                    format!(
                                        r#"<!DOCTYPE html>
<html>
<head>
    <title>Empirical Content Filter Test</title>
    <script src="http://127.0.0.1:{port}/content/normal.js"></script>
    <script src="http://127.0.0.1:{port}/ads/banner.js"></script>
</head>
<body>
    <h1>Observability Test</h1>
    <img src="http://127.0.0.1:{port}/track/pixel.gif" alt="tracking pixel">
</body>
</html>"#
                                    )
                                    .into_bytes(),
                                ),
                                "/content/normal.js" => (
                                    "application/javascript",
                                    b"console.log('normal js loaded');".to_vec(),
                                ),
                                "/ads/banner.js" => (
                                    "application/javascript",
                                    b"console.log('banner ad loaded');".to_vec(),
                                ),
                                "/track/pixel.gif" => (
                                    "image/gif",
                                    // Valid 1x1 transparent GIF bytes
                                    b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;".to_vec(),
                                ),
                                _ => ("text/plain", b"Not Found".to_vec()),
                            };

                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nCache-Control: no-store, no-cache, must-revalidate\r\nConnection: close\r\n\r\n",
                                body.len(),
                                content_type
                            );
                            let _ = stream.write_all(header.as_bytes());
                            let _ = stream.write_all(&body);
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    });

    // 2. Headless WSL2 / Display initialization check
    let env_dmabuf = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER");
    if env_dmabuf.is_err() {
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    if let Err(err) = gtk4::init() {
        eprintln!(
            "\n===============================================================================\n\
             SKIPPED: GTK4 display initialization failed: {err:?}\n\
             A running display server (WSLg Wayland/X11 or Xvfb) is required to run WebKitGTK.\n\n\
             To run this test with a display:\n\
               1. Under WSLg (Windows Subsystem for Linux with GUI):\n\
                  WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo test --test empirical_filter_test -- --nocapture\n\
               2. Under headless Xvfb (virtual framebuffer):\n\
                  xvfb-run -a cargo test --test empirical_filter_test -- --nocapture\n\
             ===============================================================================\n"
        );
        server_running.store(false, Ordering::SeqCst);
        return;
    }

    use webkit6::prelude::*;
    use webkit6::{LoadEvent, UserContentFilterStore, UserContentManager, WebView};

    let page_url = format!("http://127.0.0.1:{port}/test_page.html");

    // =========================================================================
    // PASS 1: CONTROL RUN (NO FILTER)
    // =========================================================================
    println!("[PASS 1: CONTROL RUN] Loading page without any content filter...");
    *current_phase.lock().unwrap() = "control".to_string();

    let control_loop = glib::MainLoop::new(None, false);
    let control_manager = UserContentManager::new();
    let control_wv = WebView::builder()
        .user_content_manager(&control_manager)
        .build();

    let c_loop = control_loop.clone();
    control_wv.connect_load_changed(move |_, event| {
        if event == LoadEvent::Finished {
            let l = c_loop.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                l.quit();
            });
        }
    });

    let c_timeout = control_loop.clone();
    glib::timeout_add_local_once(Duration::from_secs(12), move || {
        c_timeout.quit();
    });

    control_wv.load_uri(&page_url);
    control_loop.run();

    let ctrl_paths = control_paths.lock().unwrap().clone();
    println!(
        "Control run HTTP requests received by server: {:?}",
        ctrl_paths
    );

    // Assert that without filter, ALL subresources were requested by the browser
    assert!(
        ctrl_paths.contains(&"/test_page.html".to_string()),
        "CONTROL RUN FAILED: Server must receive /test_page.html"
    );
    assert!(
        ctrl_paths.contains(&"/content/normal.js".to_string()),
        "CONTROL RUN FAILED: Server must receive /content/normal.js"
    );
    assert!(
        ctrl_paths.contains(&"/ads/banner.js".to_string()),
        "CONTROL RUN FAILED: Server must receive /ads/banner.js in unfiltered baseline"
    );
    assert!(
        ctrl_paths.contains(&"/track/pixel.gif".to_string()),
        "CONTROL RUN FAILED: Server must receive /track/pixel.gif in unfiltered baseline"
    );
    println!("[PASS 1: CONTROL RUN] PASSED: Server received all 4 resources as expected.\n");

    // =========================================================================
    // PASS 2: FILTERED RUN (WITH WebKitUserContentFilter)
    // =========================================================================
    println!("[PASS 2: FILTERED RUN] Compiling rules and loading page with content filter...");
    *current_phase.lock().unwrap() = "filtered".to_string();

    let temp_dir = std::env::temp_dir().join(format!(
        "filter_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    let _ = std::fs::create_dir_all(&temp_dir);
    let store = UserContentFilterStore::new(temp_dir.to_str().unwrap());
    let filter_bytes = glib::Bytes::from_owned(test_filter_json.as_bytes().to_vec());

    let filtered_loop = glib::MainLoop::new(None, false);
    let filtered_manager = UserContentManager::new();
    let filtered_wv = WebView::builder()
        .user_content_manager(&filtered_manager)
        .build();

    let started_resources: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let failed_resources: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

    let sr_started = started_resources.clone();
    let sr_failed = failed_resources.clone();

    filtered_wv.connect_resource_load_started(move |_, resource, _req| {
        let uri = resource.uri().map(|s| s.to_string()).unwrap_or_default();
        sr_started.lock().unwrap().push(uri.clone());

        let failed_clone = sr_failed.clone();
        let uri_clone = uri.clone();
        resource.connect_failed(move |_, err| {
            failed_clone
                .lock()
                .unwrap()
                .push((uri_clone.clone(), err.to_string()));
        });
    });

    let f_loop = filtered_loop.clone();
    let f_wv = filtered_wv.clone();
    let f_page_url = page_url.clone();
    let f_mgr = filtered_manager.clone();

    store.save(
        "test_rules",
        &filter_bytes,
        Option::<&gtk4::gio::Cancellable>::None,
        move |save_res| {
            let filter = save_res.expect("Filter compilation failed");
            f_mgr.add_filter(&filter);

            let l_done = f_loop.clone();
            f_wv.connect_load_changed(move |_, event| {
                if event == LoadEvent::Finished {
                    let l = l_done.clone();
                    glib::timeout_add_local_once(Duration::from_millis(600), move || {
                        l.quit();
                    });
                }
            });

            f_wv.load_uri(&f_page_url);
        },
    );

    let f_timeout = filtered_loop.clone();
    glib::timeout_add_local_once(Duration::from_secs(12), move || {
        f_timeout.quit();
    });

    filtered_loop.run();

    server_running.store(false, Ordering::SeqCst);
    let _ = std::fs::remove_dir_all(&temp_dir);

    let filt_paths = filtered_paths.lock().unwrap().clone();
    let started = started_resources.lock().unwrap().clone();
    let failed = failed_resources.lock().unwrap().clone();

    // =========================================================================
    // RESULTS & PROOFS
    // =========================================================================
    println!("\n======================================================================");
    println!("EMPIRICAL VERIFICATION RESULTS");
    println!("======================================================================");
    println!("1. CONTROL RUN (No Filter) Requests Received by Server:");
    for p in &ctrl_paths {
        println!("   -> {p}");
    }

    println!("\n2. FILTERED RUN (With Filter) Requests Received by Server:");
    for p in &filt_paths {
        println!("   -> {p}");
    }

    println!("\n3. WEBKIT resource_load_started Events Emitted (Filtered Run):");
    if started.is_empty() {
        println!("   (none)");
    } else {
        for s in &started {
            println!("   -> {s}");
        }
    }

    println!("\n4. WEBKIT resource connect_failed Events Emitted (Filtered Run):");
    if failed.is_empty() {
        println!("   (none)");
    } else {
        for (u, e) in &failed {
            println!("   -> {u}: {e}");
        }
    }

    // Determine effectiveness
    let ctrl_had_banner = ctrl_paths.contains(&"/ads/banner.js".to_string());
    let ctrl_had_pixel = ctrl_paths.contains(&"/track/pixel.gif".to_string());
    let filt_had_banner = filt_paths.contains(&"/ads/banner.js".to_string());
    let filt_had_pixel = filt_paths.contains(&"/track/pixel.gif".to_string());

    let filter_effective = ctrl_had_banner && ctrl_had_pixel && !filt_had_banner && !filt_had_pixel;

    let blocked_observable_started = started
        .iter()
        .any(|u| u.contains("/ads/") || u.contains("/track/"));
    let blocked_observable_failed = failed
        .iter()
        .any(|(u, _)| u.contains("/ads/") || u.contains("/track/"));
    let blocked_observable = blocked_observable_started || blocked_observable_failed;

    println!("\n======================================================================");
    println!("SUMMARY:");
    println!(
        "FILTER EFFECTIVE: {}",
        if filter_effective { "yes" } else { "no" }
    );
    println!(
        "BLOCKED REQUESTS OBSERVABLE VIA WEBKIT SIGNALS: {}",
        if blocked_observable { "yes" } else { "no" }
    );
    println!("======================================================================\n");

    // ASSERT BOTH FACTS AS MANDATED BY TEST SPECIFICATION:
    // (the test must FAIL if the control run does not request the blocked paths, or if the filtered run does)
    assert!(
        ctrl_had_banner,
        "ASSERTION FAILED: Control run did not request /ads/banner.js"
    );
    assert!(
        ctrl_had_pixel,
        "ASSERTION FAILED: Control run did not request /track/pixel.gif"
    );
    assert!(
        !filt_had_banner,
        "ASSERTION FAILED: /ads/banner.js reached server despite active content filter!"
    );
    assert!(
        !filt_had_pixel,
        "ASSERTION FAILED: /track/pixel.gif reached server despite active content filter!"
    );
    assert!(
        filter_effective,
        "FILTER PROOF FAILED: Content filter is not effective"
    );
}
