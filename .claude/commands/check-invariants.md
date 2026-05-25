---
description: Roda apenas a bateria de testes de invariante do tree CRDT em outl-core. Mais rápido que /check, foca no que pode quebrar sync.
allowed-tools: Bash(cargo test:*)
---

Rode em sequência:

```bash
cargo test -p outl-core --test convergence -- --nocapture
cargo test -p outl-core --test cycle
cargo test -p outl-core --test cycle_chain
cargo test -p outl-core --test concurrent_edit_move
cargo test -p outl-core --test concurrent_delete_edit
cargo test -p outl-core --test late_op
cargo test -p outl-core --test idempotency
cargo test -p outl-core --test fractional_index
cargo test -p outl-core --test large_log
cargo test -p outl-core --test property_based
```

Para no primeiro fail. Reporte saída exata da falha.

Se todos passarem, conclua chamando o agent `crdt-invariant-checker` pra validação adicional (cobertura + diff estático).
