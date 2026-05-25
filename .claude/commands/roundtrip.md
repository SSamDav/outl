---
description: Roda bateria de roundtrip md ↔ ops do outl-md e dispara o markdown-roundtrip-tester pra validação adicional.
allowed-tools: Bash(cargo test:*)
---

```bash
cargo test -p outl-md
```

Se tudo passar, invoque o agent `markdown-roundtrip-tester` pra rodar checagens adicionais (property tests, sidecar validity, orphan logging).

Se falhar, pare e mostre saída exata.
