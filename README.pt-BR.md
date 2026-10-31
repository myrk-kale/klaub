<h1 align="center">Klaub</h1>

<p align="center">
  <strong>Um revisor semântico de código para humanos que envolve qualquer agente de programação.</strong><br>
  Leia o código real, descubra o que merece atenção e devolva instruções precisas ao agente sem sair do terminal nem perder a sessão.
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="docs/architecture.md">Arquitetura</a> ·
  <a href="docs/specification.pt-BR.md">Especificação</a> ·
  <a href="CONTRIBUTING.md">Contribuindo</a>
</p>

---

> **Status: em desenvolvimento ativo.** A fundação e o roadmap já estão no repositório; as funcionalidades são marcadas conforme chegam. Veja o [changelog](CHANGELOG.md).

## Por que o Klaub

Revisar o que um agente escreveu com um diff de linhas é lento: arquivos novos não têm hunks, reescritas desalinham, a visão lado a lado desperdiça largura e nada diz *qual contrato mudou* ou *quem chama isso*.

- **Semântico, não por linhas.** Navegue por módulo, arquivo, classe, função e bloco. Arquivo novo aparece inteiro; função reescrita aparece completa, `antes` e `depois` no mesmo viewport.
- **Fila de revisão, não lista alfabética.** Prioridades P0–P3 vêm de evidências explicáveis e cada achado diz se foi *introduzido*, *preexistente*, *removido* ou *desconhecido*.
- **Impacto navegável.** Grafo reverso de chamadas do símbolo alterado até os entrypoints, com a confiança de cada aresta (`resolved`, `heuristic`, `unresolved`).
- **Mesmo terminal, mesma sessão.** `klaub wrap codex` (ou `claude`, ou qualquer programa interativo) roda o agente numa sessão tmux privada; um atalho alterna entre agente e revisor.
- **Feedback no lugar certo.** Comentários são salvos num JSON versionado no repositório. O Klaub pode *preparar* uma instrução curta no input do agente. Ele nunca aperta `Enter` por você.
- **Offline, determinístico, sem LLM.** Sem conta, sem chave de API, sem telemetria, sem rede.
- **O humano decide.** O Klaub nunca aprova, reprova, certifica nem corrige código.

## Início rápido

> As instruções de instalação serão adicionadas com o primeiro release. Por enquanto, compile do código-fonte:

```shell
git clone https://github.com/myrk-kale/klaub && cd klaub
cargo install --path crates/klaub-cli --locked
```

```shell
cd meu-projeto
klaub wrap codex                 # ou: klaub wrap -- claude --model <modelo>
# Ctrl+B, R   abre o revisor        Ctrl+B, A   volta ao agente
# c           comentar              r           marcar como revisado
# F           finalizar revisão     Preparar no agent → você aperta o Enter
```

## Documentação

- [Arquitetura](docs/architecture.md) · [Decisões de arquitetura](docs/adr/) · [Modelo de segurança](docs/security.md)
- [Guia de desenvolvimento](docs/development.md) · [Especificação](docs/specification.pt-BR.md)

## Roadmap

| Fase | Escopo | Status |
| --- | --- | --- |
| Fundação | Workspace, lints, CI, arquivos de comunidade, ADRs | ✅ |
| 0 | tmux embarcado, `wrap`, identidade de pane, bracketed paste seguro, TUI mínima | ⏳ |
| 1 | Snapshots Git, Python e JS/TS, matching semântico, fila, grafo reverso, comentários, brief, evidência de testes, risco incremental | ⏳ |
| 2 | Regras de arquitetura, inspetor de contratos, follow-up, cobertura da revisão, `Flow` e a skill `klaub-flow` | ⏳ |
| 3 | Mais linguagens, skill geral de feedback, formatos de exportação, ponto de extensão para adapters de harness | ⏳ |

## Contribuindo

Contribuições são bem-vindas. Comece por [CONTRIBUTING.md](CONTRIBUTING.md) e leia o [Código de Conduta](CODE_OF_CONDUCT.md). Problemas de segurança seguem [SECURITY.md](SECURITY.md).

## Licença

[MIT](LICENSE). O tmux, libevent e ncurses embarcados mantêm suas próprias licenças, distribuídas em cada release em `THIRD-PARTY-NOTICES.md`.
