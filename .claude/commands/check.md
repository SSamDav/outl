---
description: Roda fmt + clippy + test do workspace inteiro. Use antes de reportar done.
allowed-tools: Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Bash(cargo build:*)
---

Rode em sequência e reporte resultado de cada etapa:

1. `cargo fmt --all -- --check` — formato
2. `cargo clippy --workspace --all-targets -- -D warnings` — lints
3. `cargo test --workspace --all-targets` — testes

Se algum falhar, **pare** e mostre a saída exata. Não tente corrigir automaticamente — só relate.

Formato de saída:

```
fmt:     PASS | FAIL (N arquivos)
clippy:  PASS | FAIL (N warnings)
test:    PASS | FAIL (N falhas)

[detalhes da falha, se houver]
```
