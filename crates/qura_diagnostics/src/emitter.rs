use crate::diagnostic::Diagnostic;
use qura_syntax::SourceFile;
use std::fmt::Write;

pub struct DiagnosticEmitter<'a> {
    source: &'a SourceFile,
}

impl<'a> DiagnosticEmitter<'a> {
    pub fn new(source: &'a SourceFile) -> Self {
        Self { source }
    }

    pub fn emit_to_string(&self, diag: &Diagnostic) -> String {
        let mut out = String::new();
        let loc = self.source.location(diag.span.start);

        // Header: error[E0001]: message
        let code_str = match diag.code {
            Some(c) => format!("[{}]", c),
            None => String::new(),
        };
        let _ = writeln!(
            out,
            "{}{}: {}",
            diag.severity,
            code_str,
            diag.message
        );

        // Location: --> filename:line:col
        let _ = writeln!(
            out,
            "  --> {}:{}:{}",
            self.source.name, loc.line, loc.column
        );

        // Line snippet and carets
        if let Some(line_text) = self.source.line_text(loc.line) {
            let line_num_str = loc.line.to_string();
            let padding = " ".repeat(line_num_str.len());

            let _ = writeln!(out, "{} |", padding);
            let _ = writeln!(out, "{} | {}", line_num_str, line_text);

            // Carets
            let col = loc.column.saturating_sub(1);
            let span_len = diag.span.len().max(1) as usize;
            let underline_len = span_len.min(line_text.len().saturating_sub(col)).max(1);

            let carets = "^".repeat(underline_len);
            let _ = writeln!(out, "{} | {}{}", padding, " ".repeat(col), carets);
        }

        // Secondary labels if any
        for label in &diag.labels {
            if let Some(msg) = &label.message {
                let _ = writeln!(out, "  = note: {}", msg);
            }
        }

        // Notes
        for note in &diag.notes {
            let _ = writeln!(out, "  = note: {}", note);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qura_syntax::Span;

    #[test]
    fn test_diagnostic_formatting() {
        let src = "fn main() {\n    let x = 10\n    return 0;\n}";
        let source_file = SourceFile::new("test.qr", src);
        let emitter = DiagnosticEmitter::new(&source_file);

        let diag = Diagnostic::error("expected ';' after let statement", Span::new(26, 27))
            .with_code("E0002")
            .with_note("statements must end with a semicolon in v0.1");

        let formatted = emitter.emit_to_string(&diag);
        assert!(formatted.contains("error[E0002]: expected ';' after let statement"));
        assert!(formatted.contains("--> test.qr:2:15"));
        assert!(formatted.contains("let x = 10"));
        assert!(formatted.contains("^"));
        assert!(formatted.contains("statements must end with a semicolon"));
    }
}
