//! The fuzz targets' bodies (#16). The compiler is the first reader of code
//! that may come from someone else (a carton package), so its front end is
//! fuzzed. The bodies live here, so the nightly libFuzzer runs (`fuzz/`)
//! and the stable smoke test (`tests/fuzz_smoke.rs`) drive the same code.
//! Each must hold for any input: no panic, no crash, and a program the
//! checker accepts lowers to IR that verifies.

/// The stack cig gives its parser and checker (their depth limits, E208,
/// are sized for it); cigcompile's own main runs on the same.
pub const STACK: usize = 256 * 1024 * 1024;

/// Run `f` on a thread with cig's stack; a panic there is a panic here.
fn on_cigs_stack<F: FnOnce() + Send>(f: F) {
    std::thread::scope(|s| {
        let h = std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(s, f)
            .expect("a thread for the front end");
        if let Err(p) = h.join() {
            std::panic::resume_unwind(p);
        }
    });
}

/// Any text: the parser answers with a program or a diagnostic.
pub fn parse(data: &[u8]) {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    on_cigs_stack(|| {
        let _ = cigscript::syntax::parse(s);
    });
}

/// A program that parses: the checker answers.
pub fn check(data: &[u8]) {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    on_cigs_stack(|| {
        if let Ok(p) = cigscript::syntax::parse(s) {
            let _ = cigscript::check::check(&p, &[]);
        }
    });
}

/// A program the checker accepts lowers, and its IR verifies through
/// every pass: a lowering that fails or breaks a rule on a checked program
/// is the compiler's bug.
pub fn lower(data: &[u8]) {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    on_cigs_stack(|| match crate::cir::lower_source(s, "fuzz.cig") {
        Ok(mut m) => {
            if let Err(e) =
                crate::cir::pass::run_passes(&mut m, &crate::cir::pass::default_passes())
            {
                panic!(
                    "{} broke a rule on a checked program: {:?}\n{s}",
                    e.pass, e.errors
                );
            }
        }
        Err(crate::cir::LowerError::Diagnostic(_)) => {}
        Err(crate::cir::LowerError::Internal(e)) => {
            panic!("lowering failed on a checked program: {e}\n{s}")
        }
    });
}

/// Any bytes as the end of a program's binary: a program, none, or a
/// refusal (E1050), never a panic.
pub fn program(data: &[u8]) {
    let _ = cigscript::program::read(&mut std::io::Cursor::new(data));
}
