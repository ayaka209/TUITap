use std::io::{Read, Write};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::error::CoreError;

/// A live PTY session wrapping a child process.
///
/// Call [`PtySession::spawn`] to start a new process under a PTY, then use
/// [`PtySession::reader`] and [`PtySession::write_input`] to exchange data with
/// it.
pub struct PtySession {
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    /// Spawn `command` with `args` under a PTY of the given dimensions.
    pub fn spawn(
        command: &str,
        args: &[&str],
        cols: u16,
        rows: u16,
    ) -> Result<Self, CoreError> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| CoreError::Pty(e.to_string()))?;

        let mut cmd = CommandBuilder::new(command);
        cmd.args(args);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| CoreError::Pty(e.to_string()))?;

        // Drop the slave end so its FDs are only held by the child.
        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| CoreError::Pty(e.to_string()))?;

        Ok(PtySession {
            master: pair.master,
            child,
            writer,
        })
    }

    /// Clone the master-side reader.  Each clone produces an independent
    /// `Box<dyn Read + Send>` that can be moved into a blocking task.
    pub fn reader(&self) -> Result<Box<dyn Read + Send>, CoreError> {
        self.master
            .try_clone_reader()
            .map_err(|e| CoreError::Pty(e.to_string()))
    }

    /// Write `data` to the PTY's stdin (i.e. the child's input).
    pub fn write_input(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.writer.write_all(data)?;
        Ok(())
    }

    /// Resize the PTY to the given dimensions.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), CoreError> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| CoreError::Pty(e.to_string()))
    }

    /// Non-blocking check whether the child has exited.
    pub fn try_wait(&mut self) -> Result<Option<portable_pty::ExitStatus>, CoreError> {
        self.child.try_wait().map_err(CoreError::Io)
    }

    /// Wait (blocking) for the child to exit.
    pub fn wait(&mut self) -> Result<portable_pty::ExitStatus, CoreError> {
        self.child.wait().map_err(CoreError::Io)
    }

    /// Send `SIGKILL` (or equivalent) to the child.
    pub fn kill(&mut self) -> Result<(), CoreError> {
        self.child.kill().map_err(CoreError::Io)
    }

    /// Return the OS process ID of the child, if known.
    pub fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }
}

// ── Attach helper (observe an existing process's controlling terminal) ─────────

/// Attempt to open the controlling terminal of process `pid` in read-write
/// mode so that the gateway can observe its output and inject input.
///
/// On Linux this reads the symlink `/proc/<pid>/fd/0` to discover the TTY
/// path and opens it.  On other platforms a [`CoreError::Pty`] is returned
/// indicating that the platform is not yet supported.
#[cfg(target_os = "linux")]
pub fn open_controlling_tty(pid: u32) -> Result<std::fs::File, CoreError> {
    use std::os::unix::fs::OpenOptionsExt;

    let link = format!("/proc/{pid}/fd/0");
    let tty_path = std::fs::read_link(&link)
        .map_err(|e| CoreError::Pty(format!("cannot read {link}: {e}")))?;

    // Verify it is actually a character device (a TTY).
    let meta = std::fs::metadata(&tty_path)?;
    if !{
        use std::os::unix::fs::FileTypeExt;
        meta.file_type().is_char_device()
    } {
        return Err(CoreError::Pty(format!(
            "{} is not a character device",
            tty_path.display()
        )));
    }

    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&tty_path)
        .map_err(|e| CoreError::Pty(format!("cannot open {}: {e}", tty_path.display())))
}

#[cfg(not(target_os = "linux"))]
pub fn open_controlling_tty(_pid: u32) -> Result<std::fs::File, CoreError> {
    Err(CoreError::Pty(
        "attach is only supported on Linux in this release".into(),
    ))
}
