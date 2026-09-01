//! `Core.FileSystemModule` — the TYPED filesystem PRELUDE source (the phorj-side surface), colocated
//! with the natives it wraps (`fs.rs` / `fs_bodies.rs` / `fs_lock.rs`), the same way the database, mail
//! and http_client prelude sources are colocated with their extensions (DEC-273 wave 3).
//!
//! Moved out of `cli::preludes` by DEC-348.1: that file is a grandfathered Invariant-13 breach, so the
//! size gate FAILS when it grows, and adding `tryWithLock` grew it. Splitting the biggest string const
//! out is the split-as-you-go answer the invariant asks for.
//! `Core.FileSystemModule` (W3, TOP-20 #5 blocker) — the TYPED filesystem prelude: every failure is a catchable
//! `FileSystemError` subtype (contrast the older `Core.File`, whose write/delete failures are uncatchable
//! hard faults — its deprecation is a queued adjudication; this module is purely additive).
//! Listings are SORTED (determinism). Std-only, always compiled (no feature gate). The taxonomy is
//! FileSystem-PREFIXED throughout (`FileSystemNotFoundError`, not `NotFound` — a bare generic name would CAPTURE
//! user-space classes via the injected-type discipline; caught live when the then-flat
//! `examples/web/server.phg` — now the `examples/web/server/` project — had its own `NotFound` class
//! collide).
pub(crate) const FS_PRELUDE: &str = r#"
import Core.Native.FileSystem as NativeFileSystem;
import Core.String;
import Core.List;
import Core.ClosableModule;
import Core.IteratorModule;
import Core.Bytes;
import Core.Option;

// Prelude-local result carrier (NOT Core.Result — the Core.Database injection-order rationale).
enum FileSystemResult<T> { Ok(T value), Err(string message) }

open class FileSystemError implements Error {
  constructor(public string message) {}
  static function fail(string message): never throws FileSystemError {
    if (String.startsWith(message, "<<NotFound>>")) { throw new FileSystemNotFoundError(String.removePrefix(message, "<<NotFound>>")); }
    if (String.startsWith(message, "<<PermissionDenied>>")) { throw new FileSystemPermissionDeniedError(String.removePrefix(message, "<<PermissionDenied>>")); }
    if (String.startsWith(message, "<<AlreadyExists>>")) { throw new FileSystemAlreadyExistsError(String.removePrefix(message, "<<AlreadyExists>>")); }
    if (String.startsWith(message, "<<NotADirectory>>")) { throw new FileSystemNotADirectoryError(String.removePrefix(message, "<<NotADirectory>>")); }
    if (String.startsWith(message, "<<IsADirectory>>")) { throw new FileSystemIsADirectoryError(String.removePrefix(message, "<<IsADirectory>>")); }
    if (String.startsWith(message, "<<DirNotEmpty>>")) { throw new FileSystemDirNotEmptyError(String.removePrefix(message, "<<DirNotEmpty>>")); }
    if (String.startsWith(message, "<<FileSystemIoError>>")) { throw new FileSystemIoError(String.removePrefix(message, "<<FileSystemIoError>>")); }
    throw new FileSystemError(message);
  }
}

class FileSystemNotFoundError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemPermissionDeniedError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemAlreadyExistsError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemNotADirectoryError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemIsADirectoryError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemDirNotEmptyError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemIoError extends FileSystemError { constructor(string message) { parent.constructor(message); } }

// The typed filesystem surface (static module functions — filesystem state is ambient, so an
// instance would carry nothing; the SORTED listings + typed errors are the value).
class FileSystem {
  static function readText(string path): string throws FileSystemError {
    return match (NativeFileSystem.readText(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function readBytes(string path): bytes throws FileSystemError {
    return match (NativeFileSystem.readBytes(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function writeText(string path, string contents): void throws FileSystemError {
    match (NativeFileSystem.writeText(path, contents)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function writeBytes(string path, bytes contents): void throws FileSystemError {
    match (NativeFileSystem.writeBytes(path, contents)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function appendText(string path, string contents): void throws FileSystemError {
    match (NativeFileSystem.appendText(path, contents)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function copy(string from, string to): void throws FileSystemError {
    match (NativeFileSystem.copy(from, to)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function move(string from, string to): void throws FileSystemError {
    match (NativeFileSystem.move(from, to)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function delete(string path): void throws FileSystemError {
    match (NativeFileSystem.delete(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function size(string path): int throws FileSystemError {
    return match (NativeFileSystem.size(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function exists(string path): bool throws FileSystemError {
    return match (NativeFileSystem.exists(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function isFile(string path): bool throws FileSystemError {
    return match (NativeFileSystem.isFile(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function isDir(string path): bool throws FileSystemError {
    return match (NativeFileSystem.isDir(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  // Recursive create (mkdir -p semantics); removeDir removes ONE EMPTY dir; removeDirAll is the
  // loud recursive delete (refuses "/", "." and "..").
  static function createDir(string path): void throws FileSystemError {
    match (NativeFileSystem.createDir(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function removeDir(string path): void throws FileSystemError {
    match (NativeFileSystem.removeDir(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function removeDirAll(string path): void throws FileSystemError {
    match (NativeFileSystem.removeDirAll(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  // Entry NAMES of one directory, sorted; walk = every FILE under a root as sorted relative paths.
  static function listDir(string path): List<string> throws FileSystemError {
    return match (NativeFileSystem.listDir(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function walk(string root): List<string> throws FileSystemError {
    return match (NativeFileSystem.walk(root)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function tempDir(): string throws FileSystemError {
    return match (NativeFileSystem.tempDir()) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  // DEC-347 — STREAMING line reads. `lines(path)` is an `Iterator<string>`, so it is foreach-able and
  // reads O(chunk) memory rather than slurping the file: `readText` on an 88 MB file costs ~200 MB,
  // which is the gap this closes.
  //
  // No file HANDLE exists — the ruling rejected a `FileHandle` type (blocked by C4: no transpiling
  // precedent for an opaque handle). The iterator's whole state is a byte OFFSET in an `int`, so there
  // is nothing to leak and nothing to close, and swapping in a real handle later stays non-breaking
  // because the user-facing syntax never mentions the mechanism.
  //
  // Line terminators are STRIPPED from what the iterator yields (`\n`, and a preceding `\r` so CRLF
  // files read the same as LF ones) — the terminator is a delimiter, not data.
  static function lines(string path): Iterator<string> {
    return new FileLines(path, 0, new List<string>(), 0, 0, false);
  }
  // DEC-422(a) — the FAST path for the common case: read every line, do something with each.
  //
  // Same lines as `lines(path)`, same terminator rules, but the loop runs INSIDE the native (and
  // inside `fgets` on the PHP leg), so there are no per-line phorj virtual calls. `lines` is an
  // `Iterator<string>`, which costs a `hasNext` plus a `next` per element against PHP's C loop — a
  // MEASURED 4x loss that no tuning inside that design removes.
  //
  // What you give up for it: the body is a CLOSURE, so there is no `break`, no `return` from the
  // enclosing function, and the only error it may throw is `FileSystemError` (a native parameter type
  // is fixed in Rust; the same restriction `withLock` carries). Reach for `lines` when you need any of
  // those, or when you need an `Iterator<string>` as a VALUE to pass along.
  static function forEachLine(string path, (string) => void throws FileSystemError fn): void throws FileSystemError {
    match (NativeFileSystem.forEachLine(path, fn)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  // DEC-348 — scoped advisory file locking. `withLock` is a THIN wrapper over `using` (DEC-364):
  // that is the whole design, and it is why DEC-348 was sequenced after it. The release is
  // guaranteed by construction — there is no leak path, because there is no way to hold the lock
  // without the `using` block that releases it, on every exit edge including a throw.
  //
  // Whole-file and ADVISORY: it excludes other flock/`FileSystem.withLock` users, NOT arbitrary
  // readers/writers. Byte-range locking and timeouts were both REJECTED by the ruling (byte-range
  // needs `fcntl`; a timeout would need a spin-sleep bandaid).
  //
  // The lock file is created if absent and never truncated — locking must not destroy the content
  // the lock protects.
  //
  // [Unverified on Windows] Windows is a shipped target, its lock semantics may be MANDATORY rather
  // than advisory, and there is no Windows CI. Verified on Linux only: `/proc/locks` shows
  // `FLOCK ADVISORY`, and a Rust holder and a PHP `flock()` holder block each other both ways.
  static function withLock<T>(string path, () => T throws FileSystemError fn): T throws FileSystemError {
    using (FileLock guard = FileSystem.acquireLock(path)?) {
      return fn()?;
    }
  }
  // DEC-348 — the NON-BLOCKING sibling. Returns `Option<T>`, ruled by the developer 2026-07-31 over
  // the cheaper `T?`: with `T?`, a busy lock and a closure that legitimately returned null are the
  // SAME value, so the ambiguity is invisible at the call site and type-checks clean. `None` here
  // means only *the lock was busy*; `Some(v)` means the closure ran and returned `v`, `v` possibly
  // null itself. That distinction is the whole reason this returns `Option`.
  //
  // Same release guarantee as `withLock`: the hold is a `using` block, so the only difference between
  // the two is whether a busy lock blocks or reports.
  static function tryWithLock<T>(string path, () => T throws FileSystemError fn): Option<T> throws FileSystemError {
    // `0` is the native's NOT_ACQUIRED sentinel (tickets are 1-based precisely so it can be) — NOT an
    // error: contention is the expected outcome this function exists to report. A real I/O failure
    // still arrives as `Err` and still throws.
    int ticket = match (NativeFileSystem.lockTryAcquire(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
    if (ticket == 0) {
      return new Option.None();
    }
    using (FileLock guard = new FileLock(ticket)) {
      return new Option.Some(fn()?);
    }
  }
  // Internal — the `using` subject. Not the user-facing surface: reaching for this instead of
  // `withLock` would reintroduce exactly the leak path the ruling rejected in option (B).
  private static function acquireLock(string path): FileLock throws FileSystemError {
    int ticket = match (NativeFileSystem.lockAcquire(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
    return new FileLock(ticket);
  }
  private static function ok(): void {}
}

// The held lock, as a `Closable` so `using` releases it. Carries an opaque native ticket (an `int`,
// so DEC-348 needs no new `Value`); `close()` is idempotent and declares no `throws`, which is what
// keeps `using (FileLock …)` free of `E-USING-CLOSE-THROWS` boilerplate at every call site.
// DEC-347's `Iterator<string>` over an offset. It THROWS from `hasNext`/`next`: a read can fail
// mid-iteration (the file is deleted, permissions change), and the alternative — swallowing it and
// reporting exhaustion — would turn a truncated read into a silently short loop. DEC-257 already
// requires a `foreach` over a throwing iterator to catch or declare, so the cost lands where it should.
class FileLines implements Iterator<string> {
  // `buffer`/`index` hold the lines decoded from the current chunk; `exhausted` latches at EOF so a
  // repeated `hasNext()` after the end does not keep re-reading the file.
  // `count` CACHES `List.length(buffer)`. Not a micro-optimisation for its own sake: `List.length` is a
  // native call, and the hot path hit it three times per LINE (twice in `fill`'s loop guard, once in
  // `hasNext`) — measured worth ~2.4x on the 40k-line microbench.
  constructor(
    private string path,
    private mutable int offset,
    private mutable List<string> buffer,
    private mutable int index,
    private mutable int count,
    private mutable bool exhausted
  ) {}
  function hasNext(): bool throws FileSystemError {
    if (this.index < this.count) {
      return true;
    }
    this.fill()?;
    return this.index < this.count;
  }
  function next(): string throws FileSystemError {
    // The common case needs no refill at all — `hasNext` has already run it, and a direct `next()` on a
    // ready buffer must not pay for a check it cannot need.
    if (this.index >= this.count) {
      this.fill()?;
    }
    string line = this.buffer[this.index];
    this.index = this.index + 1;
    return line;
  }
  // Refill until a line is available or the file ends. A `while`, not an `if`: a chunk CAN decode to
  // zero lines (a trailing "\n" at the very end of the file yields nothing after the drop below), and
  // treating that as exhaustion would stop the iterator one chunk early.
  private function fill(): void throws FileSystemError {
    while (this.index >= this.count && !this.exhausted) {
      string? read = match (NativeFileSystem.readLinesChunk(this.path, this.offset)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
      // `if (var …)` binds the NON-null inner, so the chunk needs no `!` unwrap below.
      if (var chunk = read) {
        // Advance by the chunk's BYTE length — the native kept the terminators precisely so this is
        // exact. A character count would desynchronise on the first non-ASCII line.
        this.offset = this.offset + Bytes.length(Bytes.fromString(chunk));
        this.buffer = NativeFileSystem.splitLines(chunk);
        this.count = List.length(this.buffer);
        this.index = 0;
      } else {
        this.exhausted = true;
        this.buffer = new List<string>();
        this.count = 0;
        this.index = 0;
        return;
      }
    }
  }
}

class FileLock implements Closable {
  constructor(private int ticket) {}
  // Both arms discard: releasing must not throw, and the native's release is already idempotent, so
  // there is nothing a caller could do with a failure on this path.
  function close(): void {
    match (NativeFileSystem.lockRelease(this.ticket)) { FileSystemResult.Ok(_) => FileLock.ok(), FileSystemResult.Err(_) => FileLock.ok() };
  }
  private static function ok(): void {}
}
"#;
