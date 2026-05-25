---
description: Cria um workspace outl de teste em ./playground e gera fixture data (algumas pages + journals) pra teste manual.
allowed-tools: Bash(cargo run:*), Bash(mkdir:*), Bash(rm:*), Bash(ls:*), Bash(find:*)
---

Setup de workspace de teste pra smoke test manual.

```bash
rm -rf ./playground
cargo run --bin outl -- init ./playground
```

Confirme estrutura criada:

```bash
find ./playground -type f | head -20
```

Esperado:
- `./playground/.outl/log.db`
- `./playground/.outl/config.toml`
- `./playground/pages/` (vazio)
- `./playground/journals/<hoje>.md` (criado se journal-on-init estiver ativo)
- `./playground/templates/journal.md`

Se faltar algo, reporte o que está faltando.
