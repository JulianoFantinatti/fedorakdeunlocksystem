slint::include_modules!();

use slint::{Image, Rgb8Pixel, SharedPixelBuffer};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use zeroize::Zeroize;

struct CameraStream {
    running: Arc<std::sync::atomic::AtomicBool>,
    child: Option<Child>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CameraStream {
    fn new() -> Self {
        Self {
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            child: None,
            thread: None,
        }
    }

    fn start(&mut self, window: slint::Weak<AuthWindow>) {
        self.stop();

        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        self.running = running.clone();

        // Prefer HD Camera (/dev/video0) for crisp color preview, fallback to /dev/video2
        let dev = if std::path::Path::new("/dev/video0").exists() {
            "/dev/video0"
        } else {
            "/dev/video2"
        };

        let child_res = Command::new("ffmpeg")
            .args([
                "-f", "v4l2",
                "-video_size", "320x240",
                "-framerate", "15",
                "-i", dev,
                "-f", "rawvideo",
                "-pix_fmt", "rgb24",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn();

        if let Ok(mut child) = child_res {
            let mut stdout = match child.stdout.take() {
                Some(s) => s,
                None => return,
            };
            self.child = Some(child);

            let handle = std::thread::spawn(move || {
                let frame_size = 320 * 240 * 3;
                let mut frame_buf = vec![0u8; frame_size];

                while running.load(std::sync::atomic::Ordering::Relaxed) {
                    if stdout.read_exact(&mut frame_buf).is_ok() {
                        let frame_copy = frame_buf.clone();
                        let w_weak = window.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = w_weak.upgrade() {
                                let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(320, 240);
                                pixel_buffer.make_mut_bytes().copy_from_slice(&frame_copy);
                                let img = Image::from_rgb8(pixel_buffer);
                                w.set_camera_frame(img);
                                w.set_has_camera_frame(true);
                            }
                        });
                    } else {
                        break;
                    }
                }
            });
            self.thread = Some(handle);
        }
    }

    fn stop(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CameraStream {
    fn drop(&mut self) {
        self.stop();
    }
}

fn get_runtime_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = std::path::PathBuf::from(dir);
        if p.exists() {
            return p;
        }
    }
    if let Ok(output) = Command::new("id").arg("-u").output() {
        let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let p = std::path::PathBuf::from(format!("/run/user/{uid}"));
        if p.exists() {
            return p;
        }
    }
    std::env::temp_dir()
}

fn set_biometric_mode(mode: &str) {
    let path = get_runtime_dir().join("biometric_mode");
    let _ = std::fs::write(path, mode);
}

fn clear_biometric_mode() {
    let path = get_runtime_dir().join("biometric_mode");
    let _ = std::fs::remove_file(path);
}

fn get_session_id() -> String {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if !id.is_empty() && id != "auto" {
            return id;
        }
    }
    // Query loginctl for active seat0 session
    if let Ok(output) = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts.contains(&"seat0") {
                return parts[0].to_string();
            }
        }
    }
    "2".to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let is_test = args.iter().any(|a| a == "--test" || a == "--demo");

    if is_test {
        println!("[*] Starting Biometric Polkit Agent in test/preview mode...");
        let test_user = std::env::var("USER").unwrap_or_else(|_| "user".into());
        run_dialog(
            "Authentication is required to run '/usr/bin/dnf' as super user.",
            &test_user,
            None,
        )
        .await?;
        return Ok(());
    }

    println!("[*] Starting Biometric Polkit Agent daemon...");
    let session_id = get_session_id();
    println!("[*] Target session ID: {session_id}");

    // Connect to System D-Bus where Polkit Authority resides
    let connection = zbus::connection::Builder::system()?
        .serve_at("/org/freedesktop/PolicyKit1/AuthenticationAgent", PolkitAgent)?
        .build()
        .await?;

    let authority: zbus::Proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await?;

    let mut details = std::collections::HashMap::new();
    details.insert(
        "session-id".to_string(),
        zbus::zvariant::Value::from(session_id.clone()),
    );

    let subject = ("unix-session", details);
    let locale = std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".into());

    let res: Result<(), _> = authority
        .call(
            "RegisterAuthenticationAgent",
            &(&subject, locale.as_str(), "/org/freedesktop/PolicyKit1/AuthenticationAgent"),
        )
        .await;

    match res {
        Ok(_) => println!("[+] Successfully registered as PolicyKit Authentication Agent on system bus!"),
        Err(e) => eprintln!("[!] Registration warning: {e}"),
    }

    // Keep running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

struct PolkitAgent;

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl PolkitAgent {
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        _icon_name: String,
        _details: std::collections::HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, std::collections::HashMap<String, zbus::zvariant::OwnedValue>)>,
    ) -> zbus::fdo::Result<()> {
        println!("[*] BeginAuthentication triggered for action: {action_id}");

        let target_user = identities
            .first()
            .and_then(|(_, map)| {
                if let Some(uid_val) = map.get("uid") {
                    if let Ok(uid) = uid_val.downcast_ref::<u32>() {
                        if let Ok(output) = Command::new("id")
                            .args(["-nu", &uid.to_string()])
                            .output()
                        {
                            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if !name.is_empty() {
                                return Some(name);
                            }
                        }
                    }
                }
                if let Some(name_val) = map.get("name") {
                    if let Ok(name) = name_val.downcast_ref::<&str>() {
                        return Some(name.to_string());
                    }
                }
                None
            })
            .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "root".into()));

        let prompt_message = if message.is_empty() {
            format!("Authentication required for {action_id}")
        } else {
            message
        };

        let result = run_dialog(&prompt_message, &target_user, Some(&cookie)).await;

        match result {
            Ok(true) => Ok(()),
            _ => Err(zbus::fdo::Error::Failed("Authentication canceled".into())),
        }
    }

    async fn cancel_authentication(&self, cookie: String) -> zbus::fdo::Result<()> {
        println!("[*] CancelAuthentication received for cookie: {cookie}");
        clear_biometric_mode();
        Ok(())
    }
}

async fn run_dialog(
    message: &str,
    user: &str,
    cookie: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let window = AuthWindow::new()?;

    window.set_action_message(message.into());
    window.set_active_mode(0); // Default to Fingerprint
    window.set_status_text("Touch fingerprint sensor...".into());

    // Initially active mode is fingerprint (mode 0)
    set_biometric_mode("fingerprint");

    let authenticated = Arc::new(Mutex::new(false));

    // Live camera streaming controller
    let cam_stream = Arc::new(Mutex::new(CameraStream::new()));

    // Spawn helper process if cookie is provided
    let child_handle: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    if let Some(c) = cookie {
        let child = Command::new("/usr/lib/polkit-1/polkit-agent-helper-1")
            .arg(user)
            .arg(c)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .ok();
        *child_handle.lock().unwrap() = child;
    }

    // Mode changed callback: isolates authentication to currently selected mode
    let window_weak = window.as_weak();
    let child_ref_mode = child_handle.clone();
    let cookie_opt = cookie.map(|c| c.to_string());
    let user_str = user.to_string();
    let cam_ref = cam_stream.clone();

    window.on_mode_changed(move |mode| {
        // Kill existing child process if running
        if let Some(mut child) = child_ref_mode.lock().unwrap().take() {
            let _ = child.kill();
        }

        let mode_str = match mode {
            0 => "fingerprint",
            1 => "face",
            _ => "password",
        };
        set_biometric_mode(mode_str);

        if let Some(w) = window_weak.upgrade() {
            match mode {
                0 => {
                    w.set_status_text("Touch fingerprint sensor...".into());
                    cam_ref.lock().unwrap().stop();
                }
                1 => {
                    w.set_status_text("Looking for your face...".into());
                    // Start live camera video stream
                    cam_ref.lock().unwrap().start(window_weak.clone());
                }
                2 => {
                    w.set_status_text("Enter password and press Enter".into());
                    cam_ref.lock().unwrap().stop();
                }
                _ => {}
            }
        }

        // Spawn a fresh helper process configured for the newly selected mode
        if let Some(ref c) = cookie_opt {
            let child = Command::new("/usr/lib/polkit-1/polkit-agent-helper-1")
                .arg(&user_str)
                .arg(c)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .ok();
            *child_ref_mode.lock().unwrap() = child;
        }
    });

    // iOS-style scanning beam animation & helper monitoring timer
    let window_weak = window.as_weak();
    let child_monitor = child_handle.clone();
    let auth_status = authenticated.clone();
    let cam_stream_timer = cam_stream.clone();
    let mut pos = 0.0f32;
    let mut dir = 0.05f32;

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(35),
        move || {
            if let Some(w) = window_weak.upgrade() {
                // Scanning animation
                pos += dir;
                if pos >= 1.0 {
                    pos = 1.0;
                    dir = -0.05;
                } else if pos <= 0.0 {
                    pos = 0.0;
                    dir = 0.05;
                }
                w.set_scan_pos(pos);

                // Check if background helper succeeded (biometrics / pam)
                if !*auth_status.lock().unwrap() {
                    let mut lock = child_monitor.lock().unwrap();
                    if let Some(ref mut child) = *lock {
                        if let Ok(Some(status)) = child.try_wait() {
                            if status.success() {
                                *auth_status.lock().unwrap() = true;
                                cam_stream_timer.lock().unwrap().stop();
                                w.set_auth_success(true);
                                let w_close = window_weak.clone();
                                slint::Timer::single_shot(
                                    std::time::Duration::from_millis(350),
                                    move || {
                                        if let Some(w2) = w_close.upgrade() {
                                            w2.hide().unwrap();
                                        }
                                    },
                                );
                            }
                        }
                    }
                }
            }
        },
    );

    // Cancel callback
    let window_weak = window.as_weak();
    let child_ref = child_handle.clone();
    let cam_stream_cancel = cam_stream.clone();
    window.on_cancel_clicked(move || {
        cam_stream_cancel.lock().unwrap().stop();
        clear_biometric_mode();
        if let Some(mut child) = child_ref.lock().unwrap().take() {
            let _ = child.kill();
        }
        if let Some(w) = window_weak.upgrade() {
            w.hide().unwrap();
        }
    });

    // OK callback
    let window_weak = window.as_weak();
    let child_ref = child_handle.clone();
    let auth_status_ok = authenticated.clone();
    let cam_stream_ok = cam_stream.clone();
    window.on_ok_clicked(move |password| {
        let mut pwd_bytes = password.as_str().as_bytes().to_vec();

        if let Some(mut child) = child_ref.lock().unwrap().take() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(&pwd_bytes);
                let _ = stdin.write_all(b"\n");
                let _ = stdin.flush();
            }
            pwd_bytes.zeroize(); // Secure memory wipe immediately

            // Wait for helper result
            let status = child.wait();
            if let Ok(s) = status {
                if s.success() {
                    *auth_status_ok.lock().unwrap() = true;
                    cam_stream_ok.lock().unwrap().stop();
                    if let Some(w) = window_weak.upgrade() {
                        w.set_auth_success(true);
                        let w_close = window_weak.clone();
                        slint::Timer::single_shot(std::time::Duration::from_millis(350), move || {
                            if let Some(w2) = w_close.upgrade() {
                                w2.hide().unwrap();
                            }
                        });
                    }
                    return;
                }
            }
        } else {
            // Test mode simulation
            pwd_bytes.zeroize();
            *auth_status_ok.lock().unwrap() = true;
            cam_stream_ok.lock().unwrap().stop();
            if let Some(w) = window_weak.upgrade() {
                w.set_auth_success(true);
                let w_close = window_weak.clone();
                slint::Timer::single_shot(std::time::Duration::from_millis(400), move || {
                    if let Some(w2) = w_close.upgrade() {
                        w2.hide().unwrap();
                    }
                });
            }
            return;
        }

        if let Some(w) = window_weak.upgrade() {
            w.set_auth_failed(true);
            w.set_status_text("Authentication failed. Please retry.".into());
        }
    });

    // Run the Slint UI event loop
    window.run()?;
    cam_stream.lock().unwrap().stop();
    clear_biometric_mode();
    let is_ok = *authenticated.lock().unwrap();
    Ok(is_ok)
}
