<?php
declare(strict_types=1);

namespace App;

interface Scorer
{
    /** @param list<string> $words */
    public function score(array $words): int;
}

final readonly class Ranking implements Scorer
{
    public const string NAME = 'lengths';

    public function __construct(public int $bonus = 1) {}

    /** @param list<string> $words */
    public function score(array $words): int
    {
        $total = 0;
        foreach ($words as $w) {
            $total += strlen($w) + $this->bonus;
        }
        return $total;
    }

    /**
     * @param list<string> $words
     * @return array<string, int>
     */
    public function summary(array $words): array
    {
        return ['total' => $this->score($words), 'count' => count($words)];
    }
}

/** @param list<string> $words */
function longest(array $words, int $floor = 2): string
{
    $pick = static fn (string $a, string $b): bool => strlen($a) >= strlen($b);
    $best = '';
    foreach ($words as $w) {
        if (strlen($w) >= $floor && $pick($w, $best)) {
            $best = $w;
        }
    }
    return $best;
}

$words = ['phorj'];
$words[] = 'lift';
$words[] = 'scout';
$r = new Ranking(bonus: 2);
echo $r->score($words) . "\n";
echo longest($words) . "\n";
echo intdiv($r->score($words), 2) . "\n";
echo (float) $r->score($words) / 2.0 . "\n";
echo Ranking::NAME . " " . 1_000 . "\n";
echo $r->summary($words)['total'] . "\n";
