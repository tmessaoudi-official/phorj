<?php
// PHP front-controller for `examples/web/server/`, runnable under `php -S`.
//
// The application logic is pure Phorj transpiled to PHP. The SAME code `phg serve` runs natively
// also runs here under PHP's built-in server — that is the value unit that round-trips. The
// superglobal↔wire adapter below is runtime GLUE (never transpiled), exactly as `src/serve/` is the
// glue on the native side.
//
// Generate the transpiled application next to this file, with the CLI demo's bootstrap removed,
// then start a dev server with this script as the router:
//
//   phg transpile src/main.phg | sed '/^ *\\Main\\main();$/d' > web_app.php
//   php -S 127.0.0.1:8080 server.php
//
// Then: curl -i http://127.0.0.1:8080/greet -H 'Host: phorj.dev'
//
// NOTE the `sed` pattern matches the bootstrap LINE, not the last line. The bootstrap is emitted
// inside the trailing global `namespace { }` block ahead of the runtime helpers, so it is NOT the
// final line of the file and the older `sed '$d'` recipe silently deleted a helper's closing brace
// instead (DEC-455.12).
require __DIR__ . '/web_app.php';

// Rebuild the raw HTTP/1.1 request, then hand it to the transpiled `respond(bytes): bytes` — the
// same bytes-in/bytes-out entry the native runtime calls. Going through the wire form rather than
// constructing a Request directly is deliberate: `Core.Http`'s Request is built by ITS OWN parser
// (`Request.parse`), so the example's parsing story stays single-sourced across both runtimes.
$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';
$target = $_SERVER['REQUEST_URI'] ?? '/';
$body = file_get_contents('php://input') ?: '';

$lines = ["$method $target HTTP/1.1"];
foreach ($_SERVER as $k => $v) {
    if (str_starts_with($k, 'HTTP_')) {
        $name = str_replace(' ', '-', ucwords(strtolower(str_replace('_', ' ', substr($k, 5)))));
        $lines[] = "$name: $v";
    }
}
$lines[] = 'Content-Length: ' . strlen($body);
$raw = implode("\r\n", $lines) . "\r\n\r\n" . $body;

$wire = \Main\respond($raw);

// Split the response wire form back into the pieces PHP emits separately.
$split = strpos($wire, "\r\n\r\n");
$head = $split === false ? $wire : substr($wire, 0, $split);
$out = $split === false ? '' : substr($wire, $split + 4);
$headLines = explode("\r\n", $head);
$statusLine = array_shift($headLines);

if (preg_match('#^HTTP/1\.1 (\d{3})#', $statusLine, $m)) {
    http_response_code((int) $m[1]);
}
foreach ($headLines as $h) {
    // Content-Length is recomputed by PHP; re-sending ours would fight it.
    if ($h !== '' && stripos($h, 'Content-Length:') !== 0) {
        header($h);
    }
}
echo $out;
