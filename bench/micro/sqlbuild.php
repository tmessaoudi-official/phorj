<?php
// Idiomatic PHP counterpart of sqlbuild.phg (hand-authored): the SAME immutable query-builder
// abstraction — table-anchored builder, per-verb object, immutable threading — as a PHP dev
// would write it (the Laravel/Doctrine-style userland builder this shape represents).
final class Query {
    public function __construct(public readonly string $sql, public readonly array $params) {}
}
final class SelectQuery {
    public function __construct(
        private string $table, private string $alias, private array $cols,
        private array $joins, private array $conds, private array $binds,
        private array $orders, private int $lim
    ) {}
    private function next(array $joins, array $conds, array $binds, array $orders, int $lim): SelectQuery {
        return new SelectQuery($this->table, $this->alias, $this->cols, $joins, $conds, $binds, $orders, $lim);
    }
    public function innerJoin(string $t, string $a): JoinClause { return new JoinClause($this, "INNER", $t, $a); }
    public function withJoin(string $frag): SelectQuery {
        $j = $this->joins; $j[] = $frag;
        return $this->next($j, $this->conds, $this->binds, $this->orders, $this->lim);
    }
    private function cond(string $col, string $op, $v): SelectQuery {
        $c = $this->conds; $c[] = "$col $op ?";
        $b = $this->binds; $b[] = $v;
        return $this->next($this->joins, $c, $b, $this->orders, $this->lim);
    }
    public function whereGt(string $col, $v): SelectQuery { return $this->cond($col, ">", $v); }
    public function whereEq(string $col, $v): SelectQuery { return $this->cond($col, "=", $v); }
    public function orderByDesc(string $col): SelectQuery {
        $o = $this->orders; $o[] = "$col DESC";
        return $this->next($this->joins, $this->conds, $this->binds, $o, $this->lim);
    }
    public function limit(int $n): SelectQuery { return $this->next($this->joins, $this->conds, $this->binds, $this->orders, $n); }
    public function toQuery(): Query {
        $qcols = [];
        foreach ($this->cols as $c) { $qcols[] = $this->qualify($c); }
        $text = "SELECT " . implode(", ", $qcols) . " FROM " . $this->table . " " . $this->alias;
        foreach ($this->joins as $j) { $text .= " " . $j; }
        if ($this->conds) {
            $qc = [];
            foreach ($this->conds as $c) {
                $sp = strpos($c, " ");
                $qc[] = $this->qualify(substr($c, 0, $sp)) . substr($c, $sp);
            }
            $text .= " WHERE " . implode(" AND ", $qc);
        }
        if ($this->orders) {
            $qo = [];
            foreach ($this->orders as $o) {
                $sp = strpos($o, " ");
                $qo[] = $this->qualify(substr($o, 0, $sp)) . substr($o, $sp);
            }
            $text .= " ORDER BY " . implode(", ", $qo);
        }
        if ($this->lim > 0) { $text .= " LIMIT " . $this->lim; }
        return new Query($text, $this->binds);
    }
    private function qualify(string $col): string {
        if (str_contains($col, ".") || str_contains($col, "(") || $col === "*") { return $col; }
        if (count($this->joins) > 0) { throw new RuntimeException("E-SQL-AMBIGUOUS-COLUMN: unqualified column '$col' with more than one table in play - qualify it with a table alias"); }
        return $this->alias . "." . $col;
    }
}
final class JoinClause {
    public function __construct(private SelectQuery $parent, private string $kind, private string $table, private string $alias) {}
    public function on(string $l, string $op, string $r): SelectQuery {
        return $this->parent->withJoin("{$this->kind} JOIN {$this->table} {$this->alias} ON $l $op $r");
    }
}
final class QueryBuilder {
    public function __construct(private string $table, private string $alias) {}
    public function select(array $cols): SelectQuery {
        return new SelectQuery($this->table, $this->alias, $cols, [], [], [], [], 0);
    }
}
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        try {
            $q = (new QueryBuilder("users", "u"))
                ->select(["u.id", "u.name", "o.total"])
                ->innerJoin("orders", "o")->on("u.id", "=", "o.userId")
                ->whereGt("u.age", $i % 80)
                ->whereEq("o.status", "paid")
                ->orderByDesc("o.total")
                ->limit(10)
                ->toQuery();
            $acc = $acc + strlen($q->sql) + count($q->params);
        } catch (RuntimeException $e) {
            $acc = $acc + strlen($e->getMessage());
        }
    }
    return $acc;
}
$iters = 100000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("sqlbuild\t%d\t%d\n", $d + $guard, $acc);
