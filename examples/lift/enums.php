<?php

/**
 * PHP enums, and the three things they do that have no direct phorj spelling.
 *
 * Lifted with `phg lift enums.php`. The paired `enums.phg` beside this file is the lifter's own
 * output, byte for byte — a test asserts that, so the pair cannot drift.
 */
enum Tenure: string
{
    case LLI = 'LLI';
    case PLAI = 'PLAI';
    case UNKNOWN = 'UNKNOWN';

    /** An enum METHOD: no phorj form — enums carry no methods. DEC-509 lowers it. */
    public function isExcluded(): bool
    {
        return match ($this) {
            self::PLAI => true,
            default => false,
        };
    }

    /** A STATIC enum method has no `$this`, so it lowers with no receiver. */
    public static function fallback(): self
    {
        return self::UNKNOWN;
    }
}

final class Listing
{
    public function __construct(public string $title, public Tenure $tenure) {}

    /** `Tenure::PLAI` is an enum CASE — spelled exactly like a class constant in PHP. */
    public function isBlocked(): bool
    {
        return $this->tenure === Tenure::PLAI;
    }
}

function main(): void
{
    $l = new Listing('T3 Cergy', Tenure::PLAI);
    echo $l->title, "\n";
    echo $l->isBlocked(), "\n";
    // The lowered method, reached by UFCS in the draft exactly as it is reached by `->` here.
    echo $l->tenure->isExcluded(), "\n";
    echo Tenure::LLI->isExcluded(), "\n";
    echo Tenure::fallback()->value, "\n";
}

