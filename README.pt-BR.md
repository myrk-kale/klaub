<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/myrk-kale/klaub/main/assets/logo-dark.png">
    <img alt="Klaub: um revisor de código no terminal para agentes de código com IA" src="https://raw.githubusercontent.com/myrk-kale/klaub/main/assets/logo-light.png" width="420">
  </picture>
</p>

# Klaub: um revisor de código para agentes de IA

O Klaub é uma ferramenta gratuita e de código aberto, para o terminal, que revisa código escrito por um agente de IA. Funciona com Codex CLI, Claude Code, Aider ou qualquer programa interativo. Mostra cada mudança por função e classe em vez de por linha, ordena as mudanças por risco, mostra quem as chama e deixa você devolver comentários ao agente. Roda offline, não usa LLM e nunca aperta Enter no seu agente por você.

<p align="center">
  <a href="https://github.com/myrk-kale/klaub/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/myrk-kale/klaub/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/myrk-kale/klaub/releases/latest"><img alt="Última versão" src="https://img.shields.io/github/v/release/myrk-kale/klaub"></a>
  <a href="LICENSE"><img alt="Licença: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="Plataformas" src="https://img.shields.io/badge/platforms-linux%20%7C%20macOS-lightgrey.svg">
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="#início-rápido">Início rápido</a> ·
  <a href="#atalhos-de-teclado">Atalhos</a> ·
  <a href="#perguntas-frequentes">Perguntas frequentes</a> ·
  <a href="#recursos">Documentação</a>
</p>

<p align="center">
  <img src="docs/media/klaub-review.gif" alt="O Klaub revisando uma mudança no terminal: uma fila ordenada, o motivo de cada prioridade, o código antigo e o novo, a visão de contrato, os chamadores até uma rota e um comentário na linha exata" width="860">
</p>

## Início rápido

Instale o Klaub, entre num repositório Git em que um agente alterou arquivos e abra o revisor.

```shell
brew tap myrk-kale/klaub && brew install klaub      # macOS e Linux com Homebrew
# ou
curl -fsSL https://raw.githubusercontent.com/myrk-kale/klaub/main/scripts/install.sh | sh
# ou
cargo install --git https://github.com/myrk-kale/klaub klaub --locked   # qualquer plataforma com Rust instalado

cd meu-projeto
klaub review                                        # revisa o que mudou desde o HEAD
```

Para manter o agente e o revisor lado a lado, inicie o agente pelo Klaub:

```shell
klaub wrap codex                 # ou: klaub wrap -- claude --model <modelo>
```

O Klaub roda o agente numa sessão própria, com uma janela de revisão ao lado. Aperte `Ctrl+B` e depois `R` para abrir o revisor, `Ctrl+B` e depois `A` para voltar ao agente, e `Ctrl+B` e depois `D` para deixar a sessão rodando em segundo plano. `klaub session attach` traz a sessão de volta.

A primeira revisão tem seis passos:

1. Abra a fila com `klaub review`. A mudança mais arriscada vem primeiro.
2. Aperte `w` para ver por que ela tem essa prioridade.
3. Aperte `Enter` para ler a função inteira. Aperte `v` para alternar entre a versão antiga e a nova.
4. Aperte `i` para o contrato e `4` para os chamadores e as rotas que chegam até a mudança.
5. Aperte `c` para comentar uma linha e `r` para marcar uma unidade como revisada.
6. Aperte `F` para finalizar. O Klaub salva os comentários e prepara uma instrução curta para o agente.

Aperte `?` a qualquer momento para ver a lista de teclas.

### Instale as skills do agente

O Klaub traz duas skills para o seu agente de código: `klaub-flow` escreve um diagrama de fluxo de uma mudança, e `klaub-feedback` faz o agente tratar os comentários de uma revisão finalizada. Elas se instalam com a [CLI de skills](https://github.com/vercel-labs/skills) e funcionam com Claude Code, Codex e os outros agentes que ela suporta.

```shell
npx skills add myrk-kale/klaub                     # escolha as skills e os agentes de forma interativa
npx skills add myrk-kale/klaub --skill klaub-flow --agent claude-code codex
```

Instalar com `cargo` compila o Klaub sem o ambiente de sessões embutido, então o `klaub wrap` precisa que um esteja instalado no seu `PATH`. O [guia de desenvolvimento](docs/development.md) tem os detalhes, e o `klaub review` funciona sem ele.

## Por que o Klaub

Um diff por linhas é uma forma ruim de ler o que um agente escreveu. Um arquivo novo não tem hunks, uma função reescrita fica desalinhada, e nada diz qual contrato público mudou nem quem depende dele. O Klaub lê o código como a linguagem lê, então a revisão segue a estrutura do código.

- Você vê funções, classes e métodos, não linhas. Um arquivo novo aparece inteiro, e uma função reescrita aparece completa, com a versão antiga e a nova no mesmo lugar.
- A fila é ordenada por evidência. Cada prioridade, de P0 a P3, vem de uma pontuação detalhada, e cada achado diz se a mudança o introduziu, se já existia ou se foi removido.
- Você vê o impacto. O Klaub segue as chamadas para cima, da função alterada até as rotas, comandos e funções `main` que conseguem alcançá-la, e classifica cada aresta como resolvida, heurística ou não resolvida.
- Seu feedback chega ao agente. Os comentários ficam no repositório como JSON versionado, e o Klaub pode colocar uma instrução de uma linha na entrada do agente. Quem aperta Enter é você.
- Tudo é local. Não há conta, chave de API, telemetria nem acesso à rede. O mesmo código gera sempre a mesma revisão.
- Você continua no comando. O Klaub nunca aprova, rejeita nem corrige código. Ele ajuda você a responder o que mudou, o que pode ser afetado, que evidências existem e o que você ainda não olhou.

## O que você recebe

### Uma fila de revisão ordenada por risco

Cada função, classe e arquivo alterado é uma unidade da fila, ordenada de P0 a P3. Aperte `w` numa unidade para ver os pontos que a colocaram ali: um contrato alterado, um ponto de entrada que a alcança, uma área sensível como `auth`, um achado de risco, um teste que falta. Uma palavra sensível sozinha nunca produz um P0.

O Klaub classifica cada símbolo como adicionado, removido, modificado, movido, renomeado, com assinatura alterada, reescrito ou só de formatação. Ele acompanha uma função que mudou de arquivo. Os achados são marcados como introduzidos, preexistentes, removidos ou desconhecidos, para você separar problemas novos de antigos.

Os achados de risco que o Klaub procura são erros engolidos, consultas montadas com texto dinâmico, segredos escritos no código, I/O dentro de laços, execução dinâmica de código, execução de shell, blocos `unsafe` e verificação de certificado desligada. Ele também confere regras de arquitetura que você define, como dependências proibidas e novos ciclos de importação. Se um achado for um falso positivo, aperte `w`, depois o número do achado, e o Klaub silencia essa regra na revisão.

### Código antigo e novo no mesmo lugar

A versão atual ocupa a tela. Aperte `v` para trocá-la pela versão anterior no mesmo viewport, e `[` e `]` para pular entre as regiões alteradas. Aperte `o` para abrir a unidade dentro do arquivo inteiro. Num terminal que informa a soltura de teclas, `[ui] hold_to_peek` deixa você segurar `B` para ver a versão antiga e soltar para voltar.

### O contrato de uma mudança

Aperte `i` para ver o que um chamador notaria: parâmetros e tipo de retorno, decorators e anotações, as rotas que o símbolo atende, as exceções que ele lança ou declara, seus efeitos observáveis e os membros públicos de um tipo. Cada item aparece como linhas adicionadas e removidas. A mesma tecla funciona em migrações (SQL, Alembic, Django, Rails), manifestos de dependências e arquivos de configuração, e nunca mostra valores secretos.

### Impacto que você consegue seguir

Aperte `4` para ver os chamadores da função alterada, as funções que ela chama e os pontos de entrada que conseguem alcançá-la, com o caminho e a confiança de cada salto. Você pode abrir qualquer um deles e pivotar a visão nele.

### Comentários e o ciclo de feedback

Os comentários ficam ancorados a um símbolo e a uma versão, e acompanham o símbolo quando as linhas mudam. Se o código comentado desaparece, o comentário é marcado como obsoleto em vez de cair na função errada. Ao finalizar, o Klaub congela os comentários num arquivo de feedback e oferece preparar uma instrução de uma linha na entrada do agente, copiá-la ou mostrar o arquivo. Na rodada seguinte, os comentários anteriores voltam, e você decide para cada um se foi resolvido, parcialmente resolvido, não resolvido ou uma regressão.

`klaub review --wait` deixa um agente rodar o revisor e esperar por você. Ele sai com 0 quando você finaliza a revisão e com 3 quando você fecha como rascunho, e com `--format json` imprime a revisão e mais nada.

### Testes, cobertura e contexto

O Klaub liga os símbolos alterados aos testes que provavelmente os exercitam e diz como sabe (uma referência, um nome, um arquivo). Ele lê relatórios JUnit e Coverage.py e avisa quando são mais antigos que a mudança. `X` roda o comando de testes configurado, depois de mostrá-lo e perguntar. Uma visão de cobertura mostra quais unidades você ainda não revisou. Você pode importar um briefing com `--brief ARQUIVO` e marcar você mesmo os critérios de aceitação.

### Diagramas de fluxo

Peça ao agente um diagrama da mudança com `p` na visão Flow. O agente o escreve como JSON com a skill `klaub-flow`, o Klaub o valida contra o snapshot, e você navega pelos passos até o código real. Um diagrama obsoleto, malformado ou que aponta para fora do repositório é recusado.

### Exportação e automação

`klaub reviews export` grava uma revisão como JSON, Markdown ou SARIF 2.1.0, para que os comentários possam ir para ferramentas de varredura de código. `klaub feedback` imprime o feedback de uma revisão finalizada. As skills `klaub-flow` e `klaub-feedback` se instalam com `npx skills add myrk-kale/klaub`.

## Linguagens suportadas

| Linguagem | Extensões | O que o Klaub entende |
| --- | --- | --- |
| Python | `.py`, `.pyi` | Símbolos, assinaturas, decorators, chamadas, imports, rotas (estilo FastAPI e Flask), funções de teste, achados de risco |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` | Símbolos, assinaturas, chamadas, imports e reexports, rotas estilo Express, achados de risco |
| TypeScript | `.ts`, `.tsx`, `.mts`, `.cts` | O mesmo do JavaScript, mais interfaces, tipos e enums |
| Rust | `.rs` | Funções, impls, traits, structs, enums, caminhos `use`, erros `Result`, `unsafe`, `#[test]` |
| Go | `.go` | Funções, métodos, structs, interfaces, pacotes, retornos `error`, rotas, `main` |
| Java | `.java` | Classes, interfaces, enums, records, métodos, anotações, `throws`, rotas Spring e JAX-RS, testes JUnit |

Arquivos em outras linguagens são listados, mostrados inteiros e comparados como texto, e o Klaub avisa que não houve análise semântica neles. Adicionar uma linguagem é uma mudança documentada, passo a passo: veja [como adicionar uma linguagem](docs/adding-a-language.md).

## Atalhos de teclado

Todas as teclas abaixo podem ser mudadas no `.klaub.toml`, em `[keys]` ([configuração](docs/configuration.md)). Aperte `?` no leitor para ver a mesma lista, `:` ou `Ctrl+P` para a paleta de comandos e `q` duas vezes para fechar.

### Modos

| Tecla | Modo |
| --- | --- |
| `1` ou `F2` | Queue (fila) |
| `2` ou `F3` | Structure: módulos, arquivos e símbolos |
| `3` ou `F4` | Code (código) |
| `4`, `g` ou `F5` | Impact: chamadores, pontos de entrada, chamados |
| `5` ou `F6` | Comments (comentários) |
| `6`, `b` ou `F8` | Context: briefing e critérios de aceitação |
| `7`, `t` ou `F7` | Tests (testes) |
| `8` ou `F10` | Follow-up: decidir sobre os comentários da rodada anterior |
| `9` | Coverage (cobertura) |
| `0`, `f` ou `F9` | Flow (fluxo) |
| `Tab`, `Shift+Tab` | Próximo modo e modo anterior |
| `Esc` | Voltar |

### Mover e ler

| Tecla | Ação |
| --- | --- |
| `j`, `k` ou setas | Mover |
| `h`, `l` | Rolar para os lados no código |
| `Enter` | Abrir a unidade, símbolo ou passo selecionado |
| `v` | Alternar entre antigo e novo |
| `B` (segurar) | Mostrar a versão anterior enquanto estiver apertada, em terminais compatíveis |
| `[`, `]` | Região alterada anterior e próxima |
| `o` | Abrir a unidade no arquivo inteiro |
| `S` | Alternar entre o símbolo e o arquivo inteiro |
| `i` | Inspecionar o contrato |
| `<`, `>` | Voltar e avançar pelas unidades visitadas |
| `/` | Buscar |
| `+`, `-` | Mostrar mais ou menos na árvore de estrutura |

### Revisar

| Tecla | Ação |
| --- | --- |
| `c` | Comentar a seleção (`V` seleciona várias linhas) |
| `w` | Por que essa prioridade, e silenciar um falso positivo |
| `r` | Marcar como revisado ou desmarcar |
| `m`, `,` | Marcar uma unidade, ir para o próximo marcador |
| `n`, `P` | Próxima e anterior unidade não revisada |
| `N` | Próxima unidade P0 ou P1 não revisada |
| `e`, `d` | Em Comments: editar ou apagar um rascunho |
| `y`, `p`, `u`, `X` | Em Follow-up: resolvido, parcial, não resolvido, regressão |
| `F` | Finalizar a revisão |
| `R` | Tirar um novo snapshot depois que o worktree mudou |

### Filtros e ferramentas

| Tecla | Ação |
| --- | --- |
| `H` | Só P0 e P1 |
| `u` | Só não revisadas |
| `I` | Introduzidas por esta mudança |
| `T` | Unidades sem testes |
| `W` | Arquivos novos |
| `s` | Alcance mínimo: local, módulo ou global |
| `M` | Próximo módulo |
| `x` | Limpar filtros |
| `X` | Rodar os testes associados (pergunta antes) |
| `U` | Recarregar relatórios de testes |
| `L` | Recarregar o diagrama de fluxo |
| `p` | Em Impact: pivotar num símbolo. Em Flow: pedir um diagrama ao agente |

### Ao redor do agente

Funcionam depois de `klaub wrap`. Aperte o prefixo (`Ctrl+B` por padrão), solte e aperte a tecla.

| Tecla | Ação |
| --- | --- |
| `R` | Abrir o revisor |
| `A` | Voltar ao agente |
| `I`, `C`, `T`, `B`, `L` | Abrir o revisor em impacto, comentários, testes, briefing ou fluxo |
| `D` | Desanexar e deixar a sessão rodando |
| `?` | Imprimir estes atalhos |

Um modo opcional de mouse (`[ui] mouse = true`) adiciona rolagem pela roda e cliques em abas, linhas e linhas de código.

## Referência de comandos

| Comando | O que faz |
| --- | --- |
| `klaub review` | Abre o revisor. Opções: `--base REV`, `--scope worktree\|staged\|unstaged\|branch`, `--brief ARQUIVO`, `--wait`, `--format json` |
| `klaub wrap <agente>` | Roda um agente com um revisor ao lado. `--detach` inicia em segundo plano |
| `klaub session list\|attach\|kill` | Gerencia sessões |
| `klaub reviews list\|show\|export` | Lista revisões salvas, imprime uma, exporta em JSON, Markdown ou SARIF |
| `klaub feedback ID` | Imprime o feedback de uma revisão finalizada |
| `klaub tests list\|run` | Lista associações de testes, ou roda o comando configurado depois de confirmar |
| `klaub flow status\|context\|validate\|prompt` | Trabalha com diagramas de fluxo |
| `klaub doctor` | Verifica Git, terminal, gramáticas, cache, diretórios e SSH |
| `klaub crashes list\|show` | Lê relatórios de falha salvos nesta máquina. Nada é enviado |
| `klaub completions SHELL` | Imprime um script de autocompletar do shell |

## Perguntas frequentes

### O que é o Klaub?

O Klaub é um aplicativo de terminal que revisa mudanças de código por função e classe. Foi feito para ler o que agentes de IA escrevem, mas revisa qualquer mudança no Git.

### Como reviso o código que o Claude Code ou o Codex escreveu?

Rode `klaub wrap claude` ou `klaub wrap codex` no seu repositório, deixe o agente trabalhar e aperte `Ctrl+B` e `R`. Você também pode rodar `klaub review` em outro terminal a qualquer momento. Ao finalizar, o Klaub pode preparar seus comentários na entrada do agente para que ele os trate.

### O Klaub envia meu código para algum lugar?

Não. O Klaub não faz requisições de rede, não tem telemetria e não precisa de conta. Comentários e revisões são arquivos em `.klaub/` no seu repositório, e os relatórios de falha ficam na sua máquina.

### O Klaub usa um LLM?

Não. A análise é determinística: ele interpreta o código, casa símbolos entre versões, monta um grafo de chamadas e aplica regras fixas. A mesma mudança gera a mesma fila, e toda pontuação pode ser explicada.

### O Klaub aperta Enter ou envia algo no meu agente?

Nunca. Colocar texto na entrada do agente sempre mostra antes o destino e o texto exato, e envia uma linha como bracketed paste, então não pode ser lido como um envio. Você aperta o Enter.

### Com quais agentes de IA ele funciona?

Codex CLI, Claude Code e Aider são os alvos, e qualquer programa interativo de terminal roda pelo `klaub wrap`. Você também pode usar `klaub review` sem agente nenhum.

### Quais linguagens de programação ele suporta?

Python, JavaScript, TypeScript (incluindo TSX), Rust, Go e Java têm análise semântica completa. Os outros arquivos aparecem inteiros e são comparados como texto. Veja [linguagens suportadas](#linguagens-suportadas).

### Como a prioridade de uma mudança é decidida?

O Klaub soma pontos por evidência: um contrato público alterado, um ponto de entrada que alcança o símbolo, uma área sensível, um achado de risco, complexidade crescente, testes ausentes. Aperte `w` para ver cada termo. Os pesos ficam em `[priority]` e estão descritos na [configuração](docs/configuration.md).

### Como silencio um falso positivo?

Abra "Why this priority" com `w`, aperte o número do achado e confirme. A regra é silenciada naquela revisão e a fila é recalculada. "Bring back silenced rules" na paleta desfaz.

### Onde ficam meus comentários?

Em `.klaub/reviews/`, na raiz do repositório, como JSON versionado. O Klaub não edita o `.gitignore` e não faz commit de nada. A primeira revisão de um repositório avisa isso uma vez.

### Posso mudar as teclas?

Sim. Toda ação, e toda tecla que só funciona num modo, pode ser movida para outro caractere em `[keys]`. O rodapé e a ajuda mostram as teclas que você escolheu.

### Como uso o Klaub no CI ou com varredura de código?

Rode `klaub reviews export ID --format sarif`. Cada comentário vira um resultado num log SARIF 2.1.0, com a prioridade nas propriedades.

### Funciona por SSH?

Sim, como programa de terminal. A área de transferência da sua máquina fica fora de alcance numa sessão remota, então use "Show file" ou `klaub feedback` ali. O `klaub doctor` avisa quando detecta SSH. Veja [usando o Klaub por SSH](docs/ssh.md).

### Roda no Windows?

Ainda não. Linux e macOS (Intel e Apple Silicon) são suportados, e o Windows é uma possível extensão futura.

### Qual a velocidade?

Num checkout do Django com 7.000 arquivos, abrir a revisão de uma mudança de 266 arquivos leva cerca de 2 segundos na primeira vez e 1 segundo com o cache aquecido. Projetos de algumas centenas de arquivos abrem em 0,3 a 1,1 segundo. Uma tecla numa lista leva bem menos de 100 ms. Os [benchmarks](docs/benchmarks.md) trazem o método e os limites desses números.

### Como desinstalo?

Remova o binário (`brew uninstall klaub`, ou apague `~/.local/bin/klaub`). Se quiser, remova também as pastas `.klaub/` dos seus repositórios e os diretórios de estado e cache do Klaub, que o `klaub doctor` imprime.

## Recursos

| | |
| --- | --- |
| [Configuração](docs/configuration.md) | Cada seção do `.klaub.toml` e como mover teclas |
| [Arquitetura](docs/architecture.md) | Como o Klaub é construído: crates, portas, fluxo de dados, a camada de terminal |
| [Modelo de segurança](docs/security.md) | Ameaças, controles e limites explícitos |
| [Usando o Klaub por SSH](docs/ssh.md) | O que funciona e o que a área de transferência não alcança |
| [Benchmarks](docs/benchmarks.md) | Latência em repositórios sintéticos e reais |
| [Adicionar uma linguagem](docs/adding-a-language.md) | Cada lugar que uma nova gramática toca |
| [Guia de desenvolvimento](docs/development.md) | Compilar do código-fonte, ferramentas e convenções |
| [Changelog](CHANGELOG.md) | O que mudou em cada versão |
| [Releases](RELEASING.md) | Como uma versão é publicada |

## Contribuindo

Contribuições são bem-vindas. Comece por [CONTRIBUTING.md](CONTRIBUTING.md) e leia o [Código de Conduta](CODE_OF_CONDUCT.md). Relate problemas de segurança como descrito em [SECURITY.md](SECURITY.md) e faça perguntas por [SUPPORT.md](SUPPORT.md).

## Licença

[MIT](LICENSE). Os componentes de terceiros que acompanham as versões mantêm suas próprias licenças, listadas em `THIRD-PARTY-NOTICES.md` em cada arquivo de release.
