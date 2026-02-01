//! Log capture output streams for WASM services.
//!
//! Provides types for redirecting WASM stdout/stderr into a log writer
//! instead of inheriting the host's stdio. This enables per-service log
//! capture in the daemon.

use std::sync::Arc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use wasmtime_wasi::{HostOutputStream, StdoutStream, StreamResult, Subscribe};

/// Stream origin for a log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Trait for receiving log output from WASM execution.
///
/// Implemented by the daemon's `ServiceLogBuffer` to capture output
/// from WASM instances without depending on daemon types.
pub trait LogWriter: Send + Sync + 'static {
    /// Write a log line from the given stream.
    fn write_log(&self, stream: LogStream, message: &str);
}

/// A `StdoutStream` implementation that captures output into a `LogWriter`.
///
/// Each call to `stream()` creates a new `LogCaptureStream` that writes
/// to the same underlying log writer.
pub struct LogCaptureSink {
    writer: Arc<dyn LogWriter>,
    stream: LogStream,
}

impl LogCaptureSink {
    /// Creates a new log capture sink.
    #[must_use]
    pub fn new(writer: Arc<dyn LogWriter>, stream: LogStream) -> Self {
        Self { writer, stream }
    }
}

impl StdoutStream for LogCaptureSink {
    fn stream(&self) -> Box<dyn HostOutputStream> {
        tracing::debug!(stream = ?self.stream, "Creating new LogCaptureStream");
        Box::new(LogCaptureStream {
            writer: Arc::clone(&self.writer),
            stream: self.stream,
            line_buffer: String::new(),
        })
    }

    fn isatty(&self) -> bool {
        false
    }
}

/// An output stream that captures bytes, line-buffers them, and pushes
/// complete lines to a `LogWriter`.
struct LogCaptureStream {
    writer: Arc<dyn LogWriter>,
    stream: LogStream,
    /// Accumulates partial lines until a newline is encountered.
    line_buffer: String,
}

impl LogCaptureStream {
    /// Flush any remaining content in the line buffer as a final log entry.
    fn flush_line_buffer(&mut self) {
        if !self.line_buffer.is_empty() {
            let line = std::mem::take(&mut self.line_buffer);
            tracing::debug!(line = %line, stream = ?self.stream, "flush_line_buffer: flushing partial line");
            self.writer.write_log(self.stream, &line);
        }
    }
}

impl HostOutputStream for LogCaptureStream {
    fn write(&mut self, bytes: Bytes) -> StreamResult<()> {
        let text = String::from_utf8_lossy(&bytes);
        tracing::trace!(
            bytes_len = bytes.len(),
            stream = ?self.stream,
            "LogCaptureStream::write called"
        );

        for ch in text.chars() {
            if ch == '\n' {
                let line = std::mem::take(&mut self.line_buffer);
                if !line.is_empty() {
                    tracing::debug!(line = %line, "Flushing complete log line");
                    self.writer.write_log(self.stream, &line);
                }
            } else {
                self.line_buffer.push(ch);
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        self.flush_line_buffer();
        Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}

#[async_trait::async_trait]
impl Subscribe for LogCaptureStream {
    async fn ready(&mut self) {
        // Always ready — no backpressure needed for log capture.
    }
}

impl Drop for LogCaptureStream {
    fn drop(&mut self) {
        // Flush any remaining partial line on drop.
        self.flush_line_buffer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Test log writer that collects entries for assertions.
    struct TestLogWriter {
        entries: Mutex<Vec<(LogStream, String)>>,
    }

    impl TestLogWriter {
        fn new() -> Self {
            Self {
                entries: Mutex::new(Vec::new()),
            }
        }

        fn entries(&self) -> Vec<(LogStream, String)> {
            self.entries.lock().unwrap().clone()
        }
    }

    impl LogWriter for TestLogWriter {
        fn write_log(&self, stream: LogStream, message: &str) {
            self.entries
                .lock()
                .unwrap()
                .push((stream, message.to_string()));
        }
    }

    #[test]
    fn test_line_buffering() {
        let writer = Arc::new(TestLogWriter::new());
        let sink = LogCaptureSink::new(Arc::clone(&writer) as Arc<dyn LogWriter>, LogStream::Stdout);
        let mut stream = sink.stream();

        // Write a partial line
        stream.write(Bytes::from("hel")).unwrap();
        assert!(writer.entries().is_empty());

        // Complete the line
        stream.write(Bytes::from("lo\n")).unwrap();
        let entries = writer.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, "hello");
    }

    #[test]
    fn test_multiple_lines_in_one_write() {
        let writer = Arc::new(TestLogWriter::new());
        let sink = LogCaptureSink::new(Arc::clone(&writer) as Arc<dyn LogWriter>, LogStream::Stderr);
        let mut stream = sink.stream();

        stream.write(Bytes::from("line1\nline2\nline3\n")).unwrap();
        let entries = writer.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].1, "line1");
        assert_eq!(entries[1].1, "line2");
        assert_eq!(entries[2].1, "line3");
    }

    #[test]
    fn test_flush_remaining_on_drop() {
        let writer = Arc::new(TestLogWriter::new());
        let sink = LogCaptureSink::new(Arc::clone(&writer) as Arc<dyn LogWriter>, LogStream::Stdout);

        {
            let mut stream = sink.stream();
            stream.write(Bytes::from("no newline")).unwrap();
            // Drop stream here
        }

        let entries = writer.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, "no newline");
    }

    #[test]
    fn test_stream_tagging() {
        let writer = Arc::new(TestLogWriter::new());

        let stdout_sink = LogCaptureSink::new(Arc::clone(&writer) as Arc<dyn LogWriter>, LogStream::Stdout);
        let stderr_sink = LogCaptureSink::new(Arc::clone(&writer) as Arc<dyn LogWriter>, LogStream::Stderr);

        let mut stdout = stdout_sink.stream();
        let mut stderr = stderr_sink.stream();

        stdout.write(Bytes::from("out\n")).unwrap();
        stderr.write(Bytes::from("err\n")).unwrap();

        let entries = writer.entries();
        assert_eq!(entries[0].0, LogStream::Stdout);
        assert_eq!(entries[1].0, LogStream::Stderr);
    }
}
