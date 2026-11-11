//! Keys as a terminal sends them: bytes written to the host in a
//! pseudo-terminal, and the keys its app hears.

#[cfg(unix)]
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Child, Command},
    time::{Duration, Instant},
};

/// The app on a pseudo-terminal, its output read into a terminal emulator.
#[cfg(unix)]
struct Pty {
    master: File,
    child: Child,
    screen: vt100::Parser,
}

#[cfg(unix)]
impl Pty {
    fn spawn(name: &str, source: &str) -> Pty {
        let dir = std::env::temp_dir().join(format!(
            "exact-terminal-input-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.contract");
        std::fs::write(&path, source).unwrap();
        let (mut master_fd, mut slave_fd) = (-1, -1);
        let mut size = libc::winsize {
            ws_row: 6,
            ws_col: 40,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master_fd,
                    &mut slave_fd,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size,
                )
            },
            0
        );
        let master = unsafe { File::from_raw_fd(master_fd) };
        let slave = unsafe { File::from_raw_fd(slave_fd) };
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            0
        );
        let child = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
            .arg(path)
            .arg("--fullscreen")
            .stdin(slave.try_clone().unwrap())
            .stdout(slave.try_clone().unwrap())
            .stderr(slave)
            .spawn()
            .unwrap();
        Pty {
            master,
            child,
            screen: vt100::Parser::new(6, 40, 0),
        }
    }

    fn text(&mut self) -> String {
        let mut chunk = [0; 4096];
        while let Ok(n) = self.master.read(&mut chunk) {
            if n == 0 {
                break;
            }
            self.screen.process(&chunk[..n]);
        }
        self.screen.screen().contents()
    }

    /// Wait up to ten seconds for the screen to show `what`.
    fn until(&mut self, what: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.text().contains(what) {
            assert!(
                Instant::now() < deadline,
                "the screen never showed {what:?}:\n{}",
                self.text()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).unwrap();
    }
}

#[cfg(unix)]
impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A screen that lists the keys its root hears, under a timer that fires
/// every 2 ms.
#[cfg(unix)]
const HEARD: &str = r#"component App
  state heard = ""
  state ticks = 0
  action keyed(k: string)
    heard = `${heard}${k};`
  action step
    ticks = ticks + 1
  task spin mount
    every(2, step)
  view
    column key=keyed
      text `heard [${heard}]`
"#;

/// A lone ESC waits 25 ms for the rest of a sequence however often the
/// loop wakes: here a timer wakes it every 2 ms. The bound is a lower one,
/// so a slow machine cannot fail it.
#[cfg(unix)]
#[test]
fn a_held_escape_waits_its_time_whatever_wakes_the_loop() {
    let mut pty = Pty::spawn("escape-wait", HEARD);
    pty.until("heard []");
    let sent = Instant::now();
    pty.write(b"\x1b");
    pty.until("heard [Escape;]");
    let waited = sent.elapsed();
    assert!(
        waited >= Duration::from_millis(25),
        "Escape after {waited:?}"
    );
}

/// Ctrl-\ leaves whatever the app does with keys (LLP 1101 D6): a focused
/// field's handler or the root's that prevents the default, or a button
/// that names it as its shortcut.
#[test]
fn control_backslash_always_leaves() {
    use exact_terminal::host::{After, Host, Key, Mode};
    use exact_terminal::vt::Vt;
    for (case, view) in [
        (
            "a focused field's handler",
            "    column\n      text `keys ${n}`\n      input value=\"\" key=onKey autofocus=true\n",
        ),
        ("the root's handler", "    column key=onKey\n      text `keys ${n}`\n"),
        (
            "a shortcut",
            "    column\n      text `keys ${n}`\n      button press=hit aria-keyshortcuts=\"Control+\\\\\"\n        text \"hit\"\n",
        ),
    ] {
        let source = format!(
            "component App\n  state n = 0\n  action onKey(key: string)\n    n = n + 1\n    preventDefault()\n  action hit\n    n = n + 1\n  view\n{view}"
        );
        let plan = contract::compile_terminal(&source).expect("compiles");
        let mut host = Host::boot(plan, (), Mode::Fullscreen, 30, 4).expect("boots");
        let mut vt = Vt::new(&mut host);
        vt.render(&mut host);
        host.read_at = host.frames;
        assert_eq!(host.key(Key::Ctrl('\\')), After::Quit, "{case}");
        vt.render(&mut host);
        assert!(
            vt.text(false).contains("keys 0"),
            "{case}: {}",
            vt.text(false)
        );
    }
}
