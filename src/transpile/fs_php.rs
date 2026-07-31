//! PHP transpiler — the `__phorj_fs_*` runtime helpers (DEC-313), gated by `uses_fs`.
//!
//! Each helper mirrors one `Core.Native.FileSystem` native (`src/native/fs.rs`) and returns a
//! 2-tuple `[ok, payload]`; the CALL SITE wraps it into the `FileSystemResult` enum (`new Ok(...)`
//! / `new Err(...)`) so the class references bind in the caller's namespace context (the DEC-313 R1
//! risk — a global helper's `new Ok` could bind the wrong class in a namespaced program).
//!
//! Error contract (DEC-313 ruling): the `<<Kind>>` marker IS the byte-identity contract — the 7-way
//! taxonomy is reconstructed with explicit PRE-CHECKS (PHP builtins expose no ErrorKind); the
//! message tail after the marker is OUT-OF-CONTRACT (Rust embeds raw OS errno text there). The
//! three kind reconstructions pinned by `tests/fs.rs` (NotFound / DirNotEmpty / PermissionDenied)
//! are exact; the rest classify best-effort with `FileSystemIoError` as the catch-all, exactly like
//! the Rust `classify()` wildcard arm. Listings sort byte-wise (`SORT_STRING` ≡ Rust `sort()`).

use super::*;

impl Transpiler {
    pub(super) fn emit_fs_helpers(&mut self) {
        if !self.gates.uses_fs {
            return;
        }
        for line in FS_HELPERS.lines() {
            self.line(line);
        }
    }
}

/// The helper bodies as literal PHP (no interpolation — simpler to keep in sync with `fs.rs`).
const FS_HELPERS: &str = r#"function __phorj_fs_err($kind, $op, $p) {
    return [false, '<<' . $kind . '>>Core.FileSystemModule.' . $op . ': `' . $p . '`'];
}
function __phorj_fs_read_text($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'readText', $p); }
    if (is_dir($p)) { return __phorj_fs_err('IsADirectory', 'readText', $p); }
    $t = @file_get_contents($p);
    if ($t === false) { return __phorj_fs_err('FileSystemIoError', 'readText', $p); }
    if (!preg_match('//u', $t)) { return __phorj_fs_err('FileSystemIoError', 'readText', $p); }
    return [true, $t];
}
// DEC-347 — the streaming-lines twin. Ladder case 1: PHP has `fgets`, so the mapping is faithful and
// no `E-TRANSPILE-*` quarantine is needed. Mirrors `native::fs_lines` exactly, including the two
// properties the prelude depends on: the chunk KEEPS its terminators (so the caller's byte-offset
// advance is exact) and it never ends mid-line (so the caller never stitches).
function __phorj_fs_read_lines_chunk($p, $off) {
    if ($off < 0) { return [false, '<<FileSystemIoError>>Core.FileSystemModule.lines: `' . $p . '`: negative offset ' . $off]; }
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'lines', $p); }
    if (is_dir($p)) { return __phorj_fs_err('IsADirectory', 'lines', $p); }
    $h = @fopen($p, 'rb');
    if ($h === false) { return __phorj_fs_err('FileSystemIoError', 'lines', $p); }
    if (fseek($h, $off) !== 0) { fclose($h); return __phorj_fs_err('FileSystemIoError', 'lines', $p); }
    $buf = '';
    while (strlen($buf) < 65536) {
        $r = fread($h, 65536 - strlen($buf));
        if ($r === false || $r === '') { break; }
        $buf .= $r;
    }
    // Stopped mid-line: `fgets` continues from the current position to the next newline (inclusive) or
    // EOF, which is precisely the Rust side's byte-at-a-time extension.
    if ($buf !== '' && substr($buf, -1) !== "\n") {
        $rest = fgets($h);
        if ($rest !== false) { $buf .= $rest; }
    }
    fclose($h);
    if ($buf === '') { return [true, null]; }
    // Same UTF-8 gate as `readText` — a chunk ends on a line boundary, so invalid UTF-8 here is real
    // file content, not a split multi-byte sequence.
    if (!preg_match('//u', $buf)) { return __phorj_fs_err('FileSystemIoError', 'lines', $p); }
    return [true, $buf];
}
// DEC-347 — the chunk-splitter twin. Mirrors `native::fs_lines::split_lines_inner` rule for rule:
// split on "\n", drop the trailing empty element the final terminator leaves, strip a preceding "\r".
function __phorj_fs_split_lines($chunk) {
    $parts = explode("\n", $chunk);
    if (end($parts) === '') { array_pop($parts); }
    $out = [];
    foreach ($parts as $l) {
        $out[] = (substr($l, -1) === "\r") ? substr($l, 0, -1) : $l;
    }
    return $out;
}
function __phorj_fs_read_bytes($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'readBytes', $p); }
    if (is_dir($p)) { return __phorj_fs_err('IsADirectory', 'readBytes', $p); }
    $t = @file_get_contents($p);
    if ($t === false) { return __phorj_fs_err('FileSystemIoError', 'readBytes', $p); }
    return [true, $t];
}
function __phorj_fs_put($p, $c, $append, $op) {
    if (is_dir($p)) { return __phorj_fs_err('IsADirectory', $op, $p); }
    $d = dirname($p);
    if (!is_dir($d)) { return __phorj_fs_err('NotFound', $op, $p); }
    if ((file_exists($p) && !is_writable($p)) || (!file_exists($p) && !is_writable($d))) {
        return __phorj_fs_err('PermissionDenied', $op, $p);
    }
    $r = @file_put_contents($p, $c, $append ? FILE_APPEND : 0);
    if ($r === false) { return __phorj_fs_err('FileSystemIoError', $op, $p); }
    return [true, null];
}
function __phorj_fs_copy($f, $t) {
    if (!file_exists($f)) { return __phorj_fs_err('NotFound', 'copy', $f); }
    if (is_dir($f)) { return __phorj_fs_err('IsADirectory', 'copy', $f); }
    if (@copy($f, $t) === false) { return __phorj_fs_err('FileSystemIoError', 'copy', $f); }
    return [true, null];
}
function __phorj_fs_move($f, $t) {
    if (!file_exists($f)) { return __phorj_fs_err('NotFound', 'move', $f); }
    if (@rename($f, $t) === false) { return __phorj_fs_err('FileSystemIoError', 'move', $f); }
    return [true, null];
}
function __phorj_fs_delete($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'delete', $p); }
    if (is_dir($p)) { return __phorj_fs_err('IsADirectory', 'delete', $p); }
    if (!is_writable(dirname($p))) { return __phorj_fs_err('PermissionDenied', 'delete', $p); }
    if (@unlink($p) === false) { return __phorj_fs_err('FileSystemIoError', 'delete', $p); }
    return [true, null];
}
function __phorj_fs_size($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'size', $p); }
    $s = @filesize($p);
    if ($s === false) { return __phorj_fs_err('FileSystemIoError', 'size', $p); }
    return [true, $s];
}
function __phorj_fs_create_dir($p) {
    if (is_dir($p)) { return [true, null]; }
    if (file_exists($p)) { return __phorj_fs_err('AlreadyExists', 'createDir', $p); }
    if (@mkdir($p, 0777, true) === false) { return __phorj_fs_err('FileSystemIoError', 'createDir', $p); }
    return [true, null];
}
function __phorj_fs_remove_dir($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'removeDir', $p); }
    if (!is_dir($p)) { return __phorj_fs_err('NotADirectory', 'removeDir', $p); }
    if (count(scandir($p)) > 2) { return __phorj_fs_err('DirNotEmpty', 'removeDir', $p); }
    if (@rmdir($p) === false) { return __phorj_fs_err('FileSystemIoError', 'removeDir', $p); }
    return [true, null];
}
function __phorj_fs_rmrf($p) {
    foreach (array_diff(scandir($p), ['.', '..']) as $n) {
        $c = $p . '/' . $n;
        if (is_dir($c) && !is_link($c)) { if (!__phorj_fs_rmrf($c)) { return false; } }
        elseif (@unlink($c) === false) { return false; }
    }
    return @rmdir($p) !== false;
}
function __phorj_fs_remove_dir_all($p) {
    if ($p === '/' || $p === '.' || $p === '..' || $p === '') {
        return __phorj_fs_err('PermissionDenied', 'removeDirAll', $p);
    }
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'removeDirAll', $p); }
    if (!is_dir($p)) { return __phorj_fs_err('NotADirectory', 'removeDirAll', $p); }
    if (!__phorj_fs_rmrf($p)) { return __phorj_fs_err('FileSystemIoError', 'removeDirAll', $p); }
    return [true, null];
}
function __phorj_fs_list_dir($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'listDir', $p); }
    if (!is_dir($p)) { return __phorj_fs_err('NotADirectory', 'listDir', $p); }
    $names = array_values(array_diff(scandir($p), ['.', '..']));
    sort($names, SORT_STRING);
    return [true, $names];
}
function __phorj_fs_walk_into($dir, $rel, &$out) {
    foreach (array_diff(scandir($dir), ['.', '..']) as $n) {
        $child = $rel === '' ? $n : $rel . '/' . $n;
        $path = $dir . '/' . $n;
        if (is_dir($path)) { __phorj_fs_walk_into($path, $child, $out); }
        else { $out[] = $child; }
    }
}
function __phorj_fs_walk($p) {
    if (!file_exists($p)) { return __phorj_fs_err('NotFound', 'walk', $p); }
    if (!is_dir($p)) { return __phorj_fs_err('NotADirectory', 'walk', $p); }
    $out = [];
    __phorj_fs_walk_into($p, '', $out);
    sort($out, SORT_STRING);
    return [true, $out];
}
// DEC-348 advisory locking — the twin of `src/native/fs_lock.rs`. PHP flock and the Rust side take the
// same kind of advisory OS lock, and each blocks the other reproducibly, so a phorj program and the PHP
// it transpiles to contend for one and the same lock. Ticket semantics are pinned to the Rust side:
// tickets start at 1, and 0 means not-acquired for the try form. The handle must stay in a table for the
// whole hold — letting it fall out of scope would close it and release early, which is the same reason
// the Rust side keeps its file in a slab.
function &__phorj_fs_locks() {
    static $locks = [];
    return $locks;
}
function __phorj_fs_lock_open($p, $op) {
    $h = @fopen($p, 'c+');
    if ($h === false) {
        if (file_exists($p) && !is_readable($p)) { return [false, __phorj_fs_err('PermissionDenied', $op, $p)[1]]; }
        if (is_dir($p)) { return [false, __phorj_fs_err('IsADirectory', $op, $p)[1]]; }
        return [false, __phorj_fs_err('FileSystemIoError', $op, $p)[1]];
    }
    return [true, $h];
}
function __phorj_fs_lock_store($h) {
    $locks = &__phorj_fs_locks();
    static $next = 1;
    $t = $next++;
    $locks[$t] = $h;
    return $t;
}
function __phorj_fs_lock_acquire($p) {
    $o = __phorj_fs_lock_open($p, 'withLock');
    if (!$o[0]) { return [false, $o[1]]; }
    if (!flock($o[1], LOCK_EX)) { fclose($o[1]); return __phorj_fs_err('FileSystemIoError', 'withLock', $p); }
    return [true, __phorj_fs_lock_store($o[1])];
}
function __phorj_fs_lock_try_acquire($p) {
    $o = __phorj_fs_lock_open($p, 'tryWithLock');
    if (!$o[0]) { return [false, $o[1]]; }
    $would_block = false;
    if (!flock($o[1], LOCK_EX | LOCK_NB, $would_block)) {
        fclose($o[1]);
        // Contention is the answer the caller asked for (0), not an error; anything else is real I/O
        // trouble and must surface as a typed error — the same split as Rust's `TryLockError`.
        if ($would_block) { return [true, 0]; }
        return __phorj_fs_err('FileSystemIoError', 'tryWithLock', $p);
    }
    return [true, __phorj_fs_lock_store($o[1])];
}
function __phorj_fs_lock_release($t) {
    $locks = &__phorj_fs_locks();
    if (isset($locks[$t])) {
        @flock($locks[$t], LOCK_UN);
        @fclose($locks[$t]);
        unset($locks[$t]);
    }
    // Idempotent, and never an error: `FileLock.close()` must not throw (see `E-USING-CLOSE-THROWS`).
    return [true, true];
}"#;
