---
description: Mede cobertura em outl-core via cargo-llvm-cov. Foca nas 4 funções críticas (do_op, undo_op, apply_op, creates_cycle) que devem estar em 100%.
allowed-tools: Bash(cargo llvm-cov:*), Bash(cargo install:*)
argument-hint: [crate] (default: outl-core)
---

Alvo: `${1:-outl-core}`

1. Se `cargo llvm-cov` não estiver instalado, instale (`cargo install cargo-llvm-cov --locked`).

2. Rode:
   ```bash
   cargo llvm-cov -p ${1:-outl-core} --html --output-dir target/llvm-cov
   cargo llvm-cov -p ${1:-outl-core} --summary-only
   ```

3. Reporte:
   - Cobertura total
   - **Cobertura específica de** `tree::do_op`, `tree::undo_op`, `tree::apply_op`, `tree::creates_cycle` (deve ser 100%)
   - Top 5 funções com pior cobertura

4. Se cobertura crítica < 100%, liste as branches descobertas com `cargo llvm-cov report --show-missing-lines -p ${1:-outl-core}`.
