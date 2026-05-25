---
description: Guia pra adicionar uma nova variante ao enum Op (ex Op::Tag, Op::Link). Cobre todos os pontos que precisam mudar.
argument-hint: <NomeDaVariante>
---

Você quer adicionar `Op::$1` ao tree CRDT do outl. Checklist OBRIGATÓRIO:

## 1. Definir a variante em `crates/outl-core/src/op.rs`

```rust
$1 {
    node: NodeId,
    // ... campos da op
    // CRÍTICO: incluir campos `old_*` pra undo.
    // Sem old_*, undo_op não consegue reverter.
}
```

## 2. Implementar `do_op` em `crates/outl-core/src/tree.rs`

- Preencher `old_*` no `LogOp` antes de aplicar mutação.
- Se a op pode violar invariante (ciclo, etc), checar primeiro e tratar como no-op materialização (mas a op vai pro log).

## 3. Implementar `undo_op`

- Reverter usando os campos `old_*` do `LogOp`.
- Idempotência: undo de op nunca aplicada deve ser no-op.

## 4. Atualizar serialização

- Verificar que serde derive já cobre. Se houver campo binário, garantir base64 ou bincode.
- Adicionar conversão pra schema SQLite em `storage/sqlite.rs` se a op tem campos extras.

## 5. Testes obrigatórios (em `crates/outl-core/tests/`)

- Convergência: 3 réplicas aplicam a op em ordens diferentes → mesmo estado final.
- Idempotência: aplicar 2x = aplicar 1x.
- Reordering: op chegando atrasada força undo/replay correto.
- Interação com `Move`: op concorrente com Move do mesmo nó converge.

## 6. Documentar em `docs/crdt.md`

- Adicionar parágrafo na seção "Operations" descrevendo semântica.
- Adicionar exemplo de caso concorrente se a op tem interação não-óbvia.

## 7. Pre-flight antes de PR

- [ ] `cargo fmt`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test -p outl-core`
- [ ] Cobertura 100% nas branches novas de `do_op`/`undo_op`
- [ ] Invocar agent `crdt-invariant-checker`
- [ ] Invocar agent `paper-verifier` se a op tem analogo no paper

**Não pule etapas.** Op nova que quebra convergência destrói confiança no outl.
