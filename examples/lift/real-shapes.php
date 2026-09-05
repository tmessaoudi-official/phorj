<?php
declare(strict_types=1);

namespace Main;

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

final class Tally
{
    private int $n = 0;

    public function add(int $k): self
    {
        $this->n = $this->n + $k;
        return $this;
    }

    public function total(): int
    {
        return $this->n;
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

/** @var list<string> $words */
$words = [];
$words[] = 'phorj';
$words[] = 'lift';
$words[] = 'scout';
$r = new Ranking(bonus: 2);
echo $r->score($words) . "\n";
echo longest($words) . "\n";
echo intdiv($r->score($words), 2) . "\n";
echo (float) $r->score($words) / 2.0 . "\n";
echo Ranking::NAME . " " . 1_000 . "\n";
echo $r->summary($words)['total'] . "\n";
$t = new Tally();
$t = $t->add(3)->add(4);
echo $t->total() . "\n";
if ($r instanceof \Main\Scorer) {
    echo "scorer\n";
}
