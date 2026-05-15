# Adaptive Parser Roadmap

This roadmap describes how Odoo Knowledge RS should stay maintainable when a new Odoo version introduces source-code patterns that existing parsers do not fully understand.

The goal is not to make the parser guess freely. The goal is to detect unknown patterns, group them, generate fixtures, and promote only tested rules into stable parsing behavior.

## Design Principles

- Keep the deterministic parser as the source of high-confidence facts.
- Add fixtures before broadening parser behavior.
- Record unknown patterns as diagnostics instead of silently ignoring them.
- Treat heuristic candidates as low-confidence until reviewed.
- Do not let candidate facts outrank exact high-confidence facts.
- Validate old versions and new versions before merging parser changes.

## Proposed Pipeline

```text
new Odoo source version
  -> deterministic parser
  -> unknown-pattern detector
  -> parser_candidates table or diagnostic records
  -> fingerprint grouping
  -> fixture generator
  -> suggested parser rule
  -> tests and accuracy eval
  -> reviewed parser promotion
```

## Candidate Diagnostics

Unknown patterns should be recorded with enough context to reproduce them:

```json
{
  "severity": "warning",
  "kind": "unknown_field_pattern",
  "file_path": "addons/example/models/product.py",
  "line_start": 42,
  "message": "Possible field declaration not recognized",
  "fingerprint": "Assign(Call(Name(Boolean), kwargs=[...]))"
}
```

## Candidate Table Proposal

A future migration can add a dedicated candidate table:

```sql
CREATE TABLE IF NOT EXISTS parser_candidates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    line_start INTEGER,
    kind TEXT NOT NULL,
    candidate_name TEXT,
    fingerprint TEXT NOT NULL,
    snippet TEXT,
    confidence TEXT NOT NULL DEFAULT 'low',
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_parser_candidates_codebase_kind_fingerprint
    ON parser_candidates(codebase_id, kind, fingerprint);
```

This table is intentionally separate from stable facts such as `models`, `fields`, `methods`, and `symbols`.

## Fingerprinting

A fingerprint is a compact representation of an AST or token pattern. It lets maintainers group many similar unknown patterns.

Example fingerprints:

- `ClassDef(assigns_name_without_models_model)`
- `Assign(target=Name, value=Call(attribute=fields.Unknown))`
- `XMLRecord(model=ir.ui.view, missing_arch_child)`

## Fixture Generation

For each frequent unknown pattern, generate a minimal fixture file:

```text
tests/fixtures/parser-candidates/odoo20_unknown_field_001.py
```

The fixture should include:

- Minimal source snippet.
- Expected stable facts after rule approval.
- Expected diagnostics before approval.

## Promotion Rules

A candidate pattern can be promoted to stable parser behavior only when:

1. A fixture exists.
2. Parser tests pass.
3. Existing version accuracy does not regress.
4. New-version accuracy improves or diagnostics decrease.
5. The extracted fact can be assigned a basis and confidence.

## Search Integration

Candidate facts should not be mixed into high-confidence search results by default. If exposed later, they should be marked:

```json
{
  "basis": "heuristic_candidate",
  "confidence": "low"
}
```

Search ranking should penalize low-confidence candidates unless the user explicitly requests candidate diagnostics.

## One-Hit Version Onboarding Integration

The onboarding script should surface parser diagnostics from `validate --codebase <name>`. If diagnostics contain unknown parser pattern kinds, the report should tell maintainers to add fixtures before claiming full support.

## Safe Evolution Strategy

1. Onboard the new codebase.
2. Review diagnostics.
3. Generate parser fixtures for repeated unknown patterns.
4. Implement parser rules with explicit basis/confidence.
5. Run parser tests.
6. Run MCP accuracy tests.
7. Materialize contexts only after validation passes.

This approach makes the parser more adaptive without sacrificing deterministic behavior or accuracy guarantees.
