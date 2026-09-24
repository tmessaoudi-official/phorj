<?php
// PHP 8 throw-expressions (DEC-532): `??`, a ternary branch, a `match` arm and an arrow fn each lift
// 1:1 to phorj's throw-expression. Each throw is caught inside its own function because the lifter
// does not derive `throws` clauses (KNOWN_ISSUES §LIFT-THROWS).
function setting(?string $value): string {
    try { return $value ?? throw new \RuntimeException("missing"); }
    catch (\RuntimeException $e) { return "default"; }
}
function half(int $n): int {
    try { return $n % 2 === 0 ? intdiv($n, 2) : throw new \InvalidArgumentException("odd"); }
    catch (\InvalidArgumentException $e) { return -1; }
}
function weekday(int $day): string {
    try { return match ($day) { 1 => "Monday", 2 => "Tuesday", default => throw new \LogicException("none") }; }
    catch (\LogicException $e) { return "?"; }
}
echo setting("dark"), "|", setting(null), "|", half(10), "|", half(7), "|", weekday(2), "|", weekday(9), "\n";
