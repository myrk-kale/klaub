//! Sentences of the command line.
#![allow(missing_docs)] // The name of each entry says what it is for.

// ---- shared pieces ------------------------------------------------------------------------------

texts! {
    const invalid_review_id {
        en: "invalid review id",
        pt: "id de revisão inválido",
        es: "id de revisión no válido",
    }
    const cancelled {
        en: "cancelled",
        pt: "cancelado",
        es: "cancelado",
    }
    /// Lower-case answers that mean yes, separated by spaces. `y` and `yes` are always accepted.
    const yes_answers {
        en: "y yes",
        pt: "s sim",
        es: "s si sí",
    }
    const yes_no_prompt {
        en: "[y/N]",
        pt: "[s/N]",
        es: "[s/N]",
    }
}

messages! {
    fn cannot_load_review(id: &str) {
        en: "cannot load review {id}",
        pt: "não foi possível carregar a revisão {id}",
        es: "no se pudo cargar la revisión {id}",
    }
    fn cannot_load_configuration(path: &str) {
        en: "cannot load configuration for {path}",
        pt: "não foi possível carregar a configuração de {path}",
        es: "no se pudo cargar la configuración de {path}",
    }
    fn cannot_open_review(path: &str) {
        en: "cannot open a review of {path}",
        pt: "não foi possível abrir uma revisão de {path}",
        es: "no se pudo abrir una revisión de {path}",
    }
}

// ---- klaub review, reviews, feedback ------------------------------------------------------------

texts! {
    const review_needs_terminal {
        en: "the reviewer is interactive and needs a terminal; run it in your terminal or through `klaub wrap`",
        pt: "o leitor é interativo e precisa de um terminal; execute-o no seu terminal ou por meio de `klaub wrap`",
        es: "el lector es interactivo y necesita un terminal; ejecútalo en tu terminal o mediante `klaub wrap`",
    }
    const keys_table_invalid {
        en: "the [keys] table in the configuration is not valid",
        pt: "a tabela [keys] da configuração não é válida",
        es: "la tabla [keys] de la configuración no es válida",
    }
    const invalid_session_value {
        en: "invalid --session value",
        pt: "valor de --session inválido",
        es: "valor de --session no válido",
    }
    const cannot_read_cwd {
        en: "cannot read the current directory",
        pt: "não foi possível ler o diretório atual",
        es: "no se pudo leer el directorio actual",
    }
}

messages! {
    fn review_finished(id: &str, comments: usize, file: &str) {
        en: "review {id} finished: {comments} comment(s). Feedback: {file}",
        pt: "revisão {id} concluída: {comments} comentário(s). Feedback: {file}",
        es: "revisión {id} terminada: {comments} comentario(s). Feedback: {file}",
    }
    fn review_closed_unfinished(comments: usize, file: &str) {
        en: "the review was closed without being finished; the draft ({comments} comment(s)) is in {file}",
        pt: "a revisão foi fechada sem ser concluída; o rascunho ({comments} comentário(s)) está em {file}",
        es: "la revisión se cerró sin terminar; el borrador ({comments} comentario(s)) está en {file}",
    }
    fn cannot_load_session(id: &str) {
        en: "cannot load session {id}",
        pt: "não foi possível carregar a sessão {id}",
        es: "no se pudo cargar la sesión {id}",
    }
    fn review_listing(id: &str, status: &str, comments: usize, updated: &str) {
        en: "{id}  {status:<9}  {comments:>4} comments  updated {updated}",
        pt: "{id}  {status:<9}  {comments:>4} comentários  atualizada {updated}",
        es: "{id}  {status:<9}  {comments:>4} comentarios  actualizada {updated}",
    }
    fn review_summary(id: &str, status: &str, comments: usize) {
        en: "{id}  {status}  {comments} comments",
        pt: "{id}  {status}  {comments} comentários",
        es: "{id}  {status}  {comments} comentarios",
    }
    fn review_not_finished(id: &str) {
        en: "review {id} is not finished; it has no feedback yet",
        pt: "a revisão {id} não foi concluída; ainda não tem feedback",
        es: "la revisión {id} no está terminada; aún no tiene feedback",
    }
    fn review_made_for_earlier(id: &str) {
        en: "review {id} was made for an earlier state of this worktree and the files have changed since. Open the current review with `klaub review`, or find it with `klaub reviews list`",
        pt: "a revisão {id} foi feita para um estado anterior deste worktree e os arquivos mudaram desde então. Abra a revisão atual com `klaub review`, ou encontre-a com `klaub reviews list`",
        es: "la revisión {id} se hizo para un estado anterior de este worktree y los archivos han cambiado desde entonces. Abre la revisión actual con `klaub review`, o encuéntrala con `klaub reviews list`",
    }
    fn cannot_read_worktree_to_compare(id: &str) {
        en: "cannot read the worktree to compare it with {id}",
        pt: "não foi possível ler o worktree para compará-lo com {id}",
        es: "no se pudo leer el worktree para compararlo con {id}",
    }
}

// ---- klaub session, wrap ------------------------------------------------------------------------

texts! {
    const sessions_none {
        en: "no sessions",
        pt: "nenhuma sessão",
        es: "ninguna sesión",
    }
    const session_attach_none {
        en: "no running sessions; start one with `klaub wrap <command>`",
        pt: "nenhuma sessão em execução; inicie uma com `klaub wrap <comando>`",
        es: "ninguna sesión en ejecución; inicia una con `klaub wrap <comando>`",
    }
    const session_kill_needs_yes {
        en: "refusing to end a session without confirmation; pass --yes",
        pt: "recusando encerrar uma sessão sem confirmação; passe --yes",
        es: "se rechaza terminar una sesión sin confirmación; pasa --yes",
    }
    const wrap_inside_tmux {
        en: "klaub wrap cannot run inside tmux: nested sessions are not supported. Detach from tmux first (or open a plain terminal) and run it again. `klaub review` works inside tmux.",
        pt: "klaub wrap não pode rodar dentro do tmux: sessões aninhadas não são suportadas. Saia do tmux primeiro (ou abra um terminal comum) e execute de novo. `klaub review` funciona dentro do tmux.",
        es: "klaub wrap no puede ejecutarse dentro de tmux: las sesiones anidadas no están soportadas. Sal de tmux primero (o abre un terminal normal) y vuelve a ejecutarlo. `klaub review` funciona dentro de tmux.",
    }
    const wrap_not_git {
        en: "not inside a Git repository; the reviewer needs one to compare changes",
        pt: "fora de um repositório Git; o leitor precisa de um para comparar as mudanças",
        es: "fuera de un repositorio Git; el lector necesita uno para comparar los cambios",
    }
}

messages! {
    fn session_not_an_id(raw: &str) {
        en: "{raw:?} is not a session id",
        pt: "{raw:?} não é um id de sessão",
        es: "{raw:?} no es un id de sesión",
    }
    fn session_several(ids: &str) {
        en: "several sessions are running ({ids}); pass one of them to `klaub session attach <id>`",
        pt: "várias sessões estão em execução ({ids}); passe uma delas a `klaub session attach <id>`",
        es: "hay varias sesiones en ejecución ({ids}); pasa una de ellas a `klaub session attach <id>`",
    }
    fn session_ended(id: &str) {
        en: "session {id} has ended; its record can be removed with `klaub session kill {id}`",
        pt: "a sessão {id} terminou; seu registro pode ser removido com `klaub session kill {id}`",
        es: "la sesión {id} ha terminado; su registro se puede eliminar con `klaub session kill {id}`",
    }
    fn session_kill_prompt(id: &str, command: &str, answers: &str) {
        en: "End session {id} and the agent running in it (`{command}`)? {answers} ",
        pt: "Encerrar a sessão {id} e o agente que roda nela (`{command}`)? {answers} ",
        es: "¿Terminar la sesión {id} y el agente que se ejecuta en ella (`{command}`)? {answers} ",
    }
    fn session_killed(id: &str) {
        en: "ended session {id}",
        pt: "sessão {id} encerrada",
        es: "sesión {id} terminada",
    }
    fn session_started(id: &str) {
        en: "started session {id}",
        pt: "sessão {id} iniciada",
        es: "sesión {id} iniciada",
    }
    fn session_attach_with(id: &str) {
        en: "attach with: klaub session attach {id}",
        pt: "para anexar: klaub session attach {id}",
        es: "para adjuntar: klaub session attach {id}",
    }
}

// ---- klaub tests --------------------------------------------------------------------------------

texts! {
    const tests_not_confirmed {
        en: "not confirmed; nothing was run",
        pt: "não confirmado; nada foi executado",
        es: "no confirmado; no se ejecutó nada",
    }
    const tests_time_limit {
        en: "the test run hit its time limit",
        pt: "a execução dos testes atingiu o limite de tempo",
        es: "la ejecución de las pruebas alcanzó el límite de tiempo",
    }
    const tests_run_cancelled {
        en: "the test run was cancelled",
        pt: "a execução dos testes foi cancelada",
        es: "la ejecución de las pruebas se canceló",
    }
    const tests_nothing_to_run {
        en: "no association found; there is nothing to run",
        pt: "nenhuma associação encontrada; não há nada para executar",
        es: "ninguna asociación encontrada; no hay nada que ejecutar",
    }
    const tests_several_commands {
        en: "the review spans more than one test command; pass --select to choose tests for one",
        pt: "a revisão abrange mais de um comando de teste; passe --select para escolher testes de um só",
        es: "la revisión abarca más de un comando de pruebas; pasa --select para elegir pruebas de uno solo",
    }
    const tests_selected_several {
        en: "the selected tests need more than one test command; run them separately",
        pt: "os testes selecionados precisam de mais de um comando de teste; execute-os separadamente",
        es: "las pruebas seleccionadas necesitan más de un comando de pruebas; ejecútalas por separado",
    }
    const tests_needs_terminal {
        en: "refusing to run tests without a terminal; pass --yes to confirm",
        pt: "recusando executar testes sem um terminal; passe --yes para confirmar",
        es: "se rechaza ejecutar pruebas sin un terminal; pasa --yes para confirmar",
    }
    const tests_cannot_prompt {
        en: "cannot prompt for confirmation",
        pt: "não foi possível pedir confirmação",
        es: "no se pudo pedir confirmación",
    }
    const tests_cannot_read_answer {
        en: "cannot read the confirmation",
        pt: "não foi possível ler a confirmação",
        es: "no se pudo leer la confirmación",
    }
}

messages! {
    fn tests_exit_code(code: i32) {
        en: "exit code {code}",
        pt: "código de saída {code}",
        es: "código de salida {code}",
    }
    fn tests_invalid_id(raw: &str) {
        en: "invalid test id {raw:?}",
        pt: "id de teste inválido {raw:?}",
        es: "id de prueba no válido {raw:?}",
    }
    fn tests_not_in_plan(id: &str) {
        en: "test {id} is not part of this review's plan",
        pt: "o teste {id} não faz parte do plano desta revisão",
        es: "la prueba {id} no forma parte del plan de esta revisión",
    }
    fn tests_run_prompt(answers: &str) {
        en: "Run this? {answers} ",
        pt: "Executar isto? {answers} ",
        es: "¿Ejecutar esto? {answers} ",
    }
}

// ---- klaub crashes, version ---------------------------------------------------------------------

texts! {
    const crashes_none_listed {
        en: "No crash reports.",
        pt: "Nenhum relatório de falha.",
        es: "Ningún informe de fallos.",
    }
    const crashes_none {
        en: "there are no crash reports",
        pt: "não há relatórios de falha",
        es: "no hay informes de fallos",
    }
    const crashes_read_first {
        en: "Read this before you share it: it holds the panic message, which can quote the data that caused it.",
        pt: "Leia isto antes de compartilhar: contém a mensagem do panic, que pode citar os dados que o causaram.",
        es: "Lee esto antes de compartirlo: contiene el mensaje del panic, que puede citar los datos que lo causaron.",
    }
    const version_no_embedded_tmux {
        en: "embedded tmux: none (uses tmux from PATH, or --tmux-path)",
        pt: "tmux embutido: nenhum (usa o tmux do PATH, ou --tmux-path)",
        es: "tmux integrado: ninguno (usa el tmux del PATH, o --tmux-path)",
    }
}

messages! {
    fn crashes_unknown(name: &str) {
        en: "there is no crash report called {name}; `klaub crashes list` shows them",
        pt: "não há relatório de falha chamado {name}; `klaub crashes list` mostra os existentes",
        es: "no hay ningún informe de fallos llamado {name}; `klaub crashes list` muestra los existentes",
    }
    fn version_embedded_tmux(version: &str, sha: &str) {
        en: "embedded tmux {version} (sha256 {sha})",
        pt: "tmux embutido {version} (sha256 {sha})",
        es: "tmux integrado {version} (sha256 {sha})",
    }
}

// ---- klaub doctor -------------------------------------------------------------------------------

texts! {
    const doctor_git_missing {
        en: "`git` was not found in PATH; Klaub drives the git executable",
        pt: "o `git` não foi encontrado no PATH; o Klaub usa o executável do git",
        es: "no se encontró `git` en el PATH; Klaub usa el ejecutable de git",
    }
    const doctor_unset {
        en: "unset",
        pt: "não definido",
        es: "sin definir",
    }
    const doctor_inside_tmux {
        en: "; inside tmux: `klaub wrap` is unavailable here, `klaub review` works",
        pt: "; dentro do tmux: `klaub wrap` não está disponível aqui, `klaub review` funciona",
        es: "; dentro de tmux: `klaub wrap` no está disponible aquí, `klaub review` funciona",
    }
    const doctor_ssh {
        en: "this is an SSH session: best effort. Copy feedback goes to the clipboard of this host, not of your machine, so use Show file (or `klaub feedback`) to read it",
        pt: "esta é uma sessão SSH: melhor esforço. Copiar feedback vai para a área de transferência deste host, não da sua máquina; use Mostrar arquivo (ou `klaub feedback`) para lê-lo",
        es: "esta es una sesión SSH: mejor esfuerzo. Copiar feedback va al portapapeles de este host, no al de tu máquina; usa Mostrar archivo (o `klaub feedback`) para leerlo",
    }
    const doctor_wsl {
        en: "this is WSL: not a supported target. Klaub runs the Linux build here, but the terminal and the tmux server are on different sides of the VM, so pane identity and paste are unproven",
        pt: "isto é o WSL: não é um alvo suportado. O Klaub roda o build Linux aqui, mas o terminal e o servidor tmux ficam em lados diferentes da VM, então a identidade do painel e a colagem não estão provadas",
        es: "esto es WSL: no es un destino soportado. Klaub ejecuta aquí la compilación de Linux, pero el terminal y el servidor tmux están en lados distintos de la VM, así que la identidad del panel y el pegado no están probados",
    }
    const doctor_wsl_mount {
        en: ". The current directory is on the Windows file system (/mnt/<drive>): git is slow there and paths differ from the Linux side; keep the repository inside the Linux file system",
        pt: ". O diretório atual está no sistema de arquivos do Windows (/mnt/<unidade>): o git é lento ali e os caminhos diferem do lado Linux; mantenha o repositório dentro do sistema de arquivos do Linux",
        es: ". El directorio actual está en el sistema de archivos de Windows (/mnt/<unidad>): git es lento allí y las rutas difieren del lado Linux; mantén el repositorio dentro del sistema de archivos de Linux",
    }
    const doctor_cache_none {
        en: "no cache yet (created by the first review)",
        pt: "ainda não há cache (criado pela primeira revisão)",
        es: "aún no hay caché (se crea con la primera revisión)",
    }
    const doctor_crashes_none {
        en: "no crash reports",
        pt: "nenhum relatório de falha",
        es: "ningún informe de fallos",
    }
}

messages! {
    fn doctor_tmux_embedded(version: &str, sha: &str) {
        en: "embedded {version}, sha256 {sha} verified",
        pt: "embutido {version}, sha256 {sha} verificado",
        es: "integrado {version}, sha256 {sha} verificado",
    }
    fn doctor_tmux_explicit(path: &str) {
        en: "{path} (explicit path, not verified)",
        pt: "{path} (caminho explícito, não verificado)",
        es: "{path} (ruta explícita, sin verificar)",
    }
    fn doctor_tmux_system(path: &str) {
        en: "{path} (found on PATH, not verified)",
        pt: "{path} (encontrado no PATH, não verificado)",
        es: "{path} (encontrado en el PATH, sin verificar)",
    }
    fn doctor_tmux_version(detail: &str, version: &str) {
        en: "{detail}; version {version}",
        pt: "{detail}; versão {version}",
        es: "{detail}; versión {version}",
    }
    fn doctor_tmux_optional(error: &str) {
        en: "{error}. `klaub review` works without it",
        pt: "{error}. `klaub review` funciona sem ele",
        es: "{error}. `klaub review` funciona sin él",
    }
    fn doctor_clipboard_used(program: &str) {
        en: "`{program}` will be used",
        pt: "`{program}` será usado",
        es: "se usará `{program}`",
    }
    fn doctor_clipboard_missing(programs: &str) {
        en: "none of {programs} was found; Copy feedback will say so and Show file still works",
        pt: "nenhum de {programs} foi encontrado; Copiar feedback vai avisar e Mostrar arquivo continua funcionando",
        es: "no se encontró ninguno de {programs}; Copiar feedback lo avisará y Mostrar archivo sigue funcionando",
    }
    fn doctor_grammars_ok(count: usize) {
        en: "{count} languages parse a sample",
        pt: "{count} linguagens analisam um exemplo",
        es: "{count} lenguajes analizan una muestra",
    }
    fn doctor_grammars_broken(languages: &str) {
        en: "did not parse a sample: {languages}",
        pt: "não analisaram um exemplo: {languages}",
        es: "no analizaron una muestra: {languages}",
    }
    fn doctor_cache_ok(entries: u64) {
        en: "{entries} entries, integrity ok",
        pt: "{entries} entradas, integridade ok",
        es: "{entries} entradas, integridad ok",
    }
    fn doctor_cache_degraded(integrity: &str, schema: &str) {
        en: "integrity {integrity}, schema {schema}: the next review rebuilds it, nothing is lost",
        pt: "integridade {integrity}, esquema {schema}: a próxima revisão o reconstrói, nada se perde",
        es: "integridad {integrity}, esquema {schema}: la próxima revisión lo reconstruye, no se pierde nada",
    }
    fn doctor_cache_unreadable(error: &str) {
        en: "unreadable ({error}): the next review replaces it, nothing is lost",
        pt: "ilegível ({error}): a próxima revisão o substitui, nada se perde",
        es: "ilegible ({error}): la próxima revisión lo reemplaza, no se pierde nada",
    }
    fn doctor_crashes_some(count: usize) {
        en: "{count} saved on this machine; `klaub crashes list` shows them. Nothing was sent",
        pt: "{count} salvo(s) nesta máquina; `klaub crashes list` mostra. Nada foi enviado",
        es: "{count} guardado(s) en esta máquina; `klaub crashes list` los muestra. No se envió nada",
    }
    fn doctor_dir_created_on_use(shown: &str) {
        en: "{shown} (created on first use)",
        pt: "{shown} (criado no primeiro uso)",
        es: "{shown} (se crea en el primer uso)",
    }
    fn doctor_sessions(running: usize, recorded: usize) {
        en: "{running} running, {recorded} recorded",
        pt: "{running} em execução, {recorded} registrada(s)",
        es: "{running} en ejecución, {recorded} registrada(s)",
    }
}

// ---- --help: headings and the commands' descriptions ---------------------------------------------
//
// The English text of every entry below must equal the doc comment clap derives it from;
// `cli::help` has a test that compares them, so the two cannot drift apart.

texts! {
    const help_usage {
        en: "Usage",
        pt: "Uso",
        es: "Uso",
    }
    const help_commands {
        en: "Commands",
        pt: "Comandos",
        es: "Comandos",
    }
    const help_options {
        en: "Options",
        pt: "Opções",
        es: "Opciones",
    }
    const help_arguments {
        en: "Arguments",
        pt: "Argumentos",
        es: "Argumentos",
    }
    const help_print_help {
        en: "Print help",
        pt: "Mostra a ajuda",
        es: "Muestra la ayuda",
    }
    const help_print_version {
        en: "Print version",
        pt: "Mostra a versão",
        es: "Muestra la versión",
    }
    const help_help_command {
        en: "Print this message or the help of the given subcommand(s)",
        pt: "Mostra esta mensagem ou a ajuda dos subcomandos indicados",
        es: "Muestra este mensaje o la ayuda de los subcomandos indicados",
    }
    const help_root {
        en: "Klaub: a terminal code reviewer for the human reading AI-written code. Review changes by function, ranked by risk, offline and with no LLM.",
        pt: "Klaub: um revisor de código no terminal para quem lê código escrito por IA. Revise as mudanças por função, ordenadas por risco, offline e sem LLM.",
        es: "Klaub: un revisor de código en la terminal para quien lee código escrito por IA. Revisa los cambios por función, ordenados por riesgo, sin conexión y sin LLM.",
    }
    const help_root_long {
        en: "Klaub runs your coding agent in a private tmux session and gives you a fullscreen semantic reviewer one key chord away. Review the real code, comment on it, and hand the feedback back to the agent. Klaub never approves, rejects or submits anything for you.",
        pt: "O Klaub executa seu agente de código em uma sessão tmux privada e oferece um revisor semântico em tela cheia a um atalho de distância. Revise o código real, comente nele e devolva o feedback ao agente. O Klaub nunca aprova, rejeita nem envia nada por você.",
        es: "Klaub ejecuta tu agente de código en una sesión tmux privada y te ofrece un revisor semántico a pantalla completa a un atajo de distancia. Revisa el código real, coméntalo y devuelve el feedback al agente. Klaub nunca aprueba, rechaza ni envía nada por ti.",
    }
    const help_tmux_path {
        en: "Use this tmux executable instead of the embedded one (diagnostics and restricted environments).",
        pt: "Usa este executável do tmux em vez do embutido (diagnóstico e ambientes restritos).",
        es: "Usa este ejecutable de tmux en lugar del integrado (diagnóstico y entornos restringidos).",
    }
    const help_wrap {
        en: "Run a coding agent in a private tmux session with a reviewer window next to it.",
        pt: "Executa um agente de código em uma sessão tmux privada, com uma janela de revisão ao lado.",
        es: "Ejecuta un agente de código en una sesión tmux privada, con una ventana de revisión al lado.",
    }
    const help_wrap_after {
        en: "Examples:\n  klaub wrap codex\n  klaub wrap -- claude --model <model>\n\nArguments after the command are passed to it untouched.",
        pt: "Exemplos:\n  klaub wrap codex\n  klaub wrap -- claude --model <modelo>\n\nOs argumentos depois do comando são passados a ele sem alteração.",
        es: "Ejemplos:\n  klaub wrap codex\n  klaub wrap -- claude --model <modelo>\n\nLos argumentos después del comando se le pasan sin cambios.",
    }
    const help_wrap_detach {
        en: "Start the session but do not attach to it.",
        pt: "Inicia a sessão sem anexar a ela.",
        es: "Inicia la sesión sin adjuntarse a ella.",
    }
    const help_wrap_prefix {
        en: "tmux prefix key for the session, in tmux notation.",
        pt: "Tecla de prefixo do tmux para a sessão, na notação do tmux.",
        es: "Tecla de prefijo de tmux para la sesión, en la notación de tmux.",
    }
    const help_wrap_command {
        en: "The agent command and its arguments, run without a shell.",
        pt: "O comando do agente e seus argumentos, executados sem shell.",
        es: "El comando del agente y sus argumentos, ejecutados sin shell.",
    }
    const help_review {
        en: "Open the reviewer.",
        pt: "Abre o revisor.",
        es: "Abre el revisor.",
    }
    const help_review_session {
        en: "The wrapped session this reviewer belongs to (set by `klaub wrap`).",
        pt: "A sessão envolvida à qual este revisor pertence (definida por `klaub wrap`).",
        es: "La sesión envuelta a la que pertenece este revisor (la define `klaub wrap`).",
    }
    const help_review_base {
        en: "The revision to compare against (default: the configured base, usually HEAD).",
        pt: "A revisão com a qual comparar (padrão: a base configurada, normalmente HEAD).",
        es: "La revisión con la que comparar (por defecto: la base configurada, normalmente HEAD).",
    }
    const help_review_scope {
        en: "What to compare: worktree, staged, unstaged or branch.",
        pt: "O que comparar: worktree, staged, unstaged ou branch.",
        es: "Qué comparar: worktree, staged, unstaged o branch.",
    }
    const help_review_brief {
        en: "A local brief file to import into the review. Read once, never fetched.",
        pt: "Um arquivo de brief local para importar na revisão. Lido uma vez, nunca buscado na rede.",
        es: "Un archivo de brief local para importar en la revisión. Se lee una vez, nunca se descarga.",
    }
    const help_review_wait {
        en: "Stay until the review is closed and report how it ended: exit 0 when it was finished, 3 when it was closed as a draft. Meant for an agent that runs the reviewer and waits for the human; it needs a terminal like any review.",
        pt: "Fica até a revisão ser fechada e informa como terminou: saída 0 quando foi concluída, 3 quando foi fechada como rascunho. Feito para um agente que executa o revisor e espera o humano; precisa de um terminal como qualquer revisão.",
        es: "Se queda hasta que se cierre la revisión e informa cómo terminó: salida 0 si se terminó, 3 si se cerró como borrador. Pensado para un agente que ejecuta el revisor y espera al humano; necesita un terminal como cualquier revisión.",
    }
    const help_review_format {
        en: "With --wait: `json` prints the review export on standard output and nothing else.",
        pt: "Com --wait: `json` imprime a exportação da revisão na saída padrão e nada mais.",
        es: "Con --wait: `json` imprime la exportación de la revisión en la salida estándar y nada más.",
    }
    const help_reviews {
        en: "List and export stored reviews.",
        pt: "Lista e exporta as revisões guardadas.",
        es: "Lista y exporta las revisiones guardadas.",
    }
    const help_reviews_list {
        en: "List the reviews stored in this worktree.",
        pt: "Lista as revisões guardadas neste worktree.",
        es: "Lista las revisiones guardadas en este worktree.",
    }
    const help_reviews_show {
        en: "Print one review.",
        pt: "Imprime uma revisão.",
        es: "Imprime una revisión.",
    }
    const help_reviews_export {
        en: "Write the review as JSON, Markdown or SARIF.",
        pt: "Escreve a revisão como JSON, Markdown ou SARIF.",
        es: "Escribe la revisión como JSON, Markdown o SARIF.",
    }
    const help_review_id {
        en: "Review id (`rev_...`).",
        pt: "Id da revisão (`rev_...`).",
        es: "Id de la revisión (`rev_...`).",
    }
    const help_format {
        en: "Output format.",
        pt: "Formato de saída.",
        es: "Formato de salida.",
    }
    const help_export_format {
        en: "Output format. `text` is an alias of `markdown`.",
        pt: "Formato de saída. `text` é um apelido de `markdown`.",
        es: "Formato de salida. `text` es un alias de `markdown`.",
    }
    const help_feedback {
        en: "Show the feedback produced by a finished review.",
        pt: "Mostra o feedback produzido por uma revisão concluída.",
        es: "Muestra el feedback producido por una revisión terminada.",
    }
    const help_tests {
        en: "Inspect and run the tests associated with a review.",
        pt: "Inspeciona e executa os testes associados a uma revisão.",
        es: "Inspecciona y ejecuta las pruebas asociadas a una revisión.",
    }
    const help_tests_list {
        en: "List the test associations of a review. Lists only; it runs nothing.",
        pt: "Lista as associações de testes de uma revisão. Só lista; não executa nada.",
        es: "Lista las asociaciones de pruebas de una revisión. Solo lista; no ejecuta nada.",
    }
    const help_tests_run {
        en: "Run the configured test command for a review. Shows the command and asks first.",
        pt: "Executa o comando de testes configurado para uma revisão. Mostra o comando e pergunta antes.",
        es: "Ejecuta el comando de pruebas configurado para una revisión. Muestra el comando y pregunta antes.",
    }
    const help_tests_select {
        en: "Restrict the run to these test ids.",
        pt: "Restringe a execução a estes ids de teste.",
        es: "Limita la ejecución a estos ids de prueba.",
    }
    const help_yes {
        en: "Do not ask for confirmation.",
        pt: "Não pede confirmação.",
        es: "No pide confirmación.",
    }
    const help_flow {
        en: "Inspect the flow diagram of a change.",
        pt: "Inspeciona o diagrama de fluxo de uma mudança.",
        es: "Inspecciona el diagrama de flujo de un cambio.",
    }
    const help_flow_status {
        en: "Report whether the current snapshot has a flow diagram.",
        pt: "Informa se o snapshot atual tem um diagrama de fluxo.",
        es: "Informa si el snapshot actual tiene un diagrama de flujo.",
    }
    const help_flow_context {
        en: "Print the snapshot context a flow diagram must describe, as JSON.",
        pt: "Imprime, em JSON, o contexto do snapshot que um diagrama de fluxo deve descrever.",
        es: "Imprime, en JSON, el contexto del snapshot que debe describir un diagrama de flujo.",
    }
    const help_flow_validate {
        en: "Validate a flow document.",
        pt: "Valida um documento de fluxo.",
        es: "Valida un documento de flujo.",
    }
    const help_flow_file {
        en: "The flow document to check.",
        pt: "O documento de fluxo a verificar.",
        es: "El documento de flujo a comprobar.",
    }
    const help_flow_prompt {
        en: "Print the instruction that asks an agent for a flow diagram. Prints only; it sends nothing.",
        pt: "Imprime a instrução que pede um diagrama de fluxo a um agente. Só imprime; não envia nada.",
        es: "Imprime la instrucción que pide un diagrama de flujo a un agente. Solo imprime; no envía nada.",
    }
    const help_session {
        en: "Inspect and control wrapped sessions.",
        pt: "Inspeciona e controla as sessões envolvidas.",
        es: "Inspecciona y controla las sesiones envueltas.",
    }
    const help_session_list {
        en: "List sessions.",
        pt: "Lista as sessões.",
        es: "Lista las sesiones.",
    }
    const help_session_attach {
        en: "Attach the terminal to a session.",
        pt: "Anexa o terminal a uma sessão.",
        es: "Adjunta el terminal a una sesión.",
    }
    const help_session_attach_id {
        en: "Session id; optional when exactly one session is running.",
        pt: "Id da sessão; opcional quando há exatamente uma sessão em execução.",
        es: "Id de la sesión; opcional cuando hay exactamente una sesión en ejecución.",
    }
    const help_session_kill {
        en: "End a session and its agent.",
        pt: "Encerra uma sessão e seu agente.",
        es: "Termina una sesión y su agente.",
    }
    const help_session_kill_id {
        en: "Session id.",
        pt: "Id da sessão.",
        es: "Id de la sesión.",
    }
    const help_doctor {
        en: "Check the environment: git, tmux, terminal, grammars, the cache and directories.",
        pt: "Verifica o ambiente: git, tmux, terminal, gramáticas, o cache e os diretórios.",
        es: "Comprueba el entorno: git, tmux, terminal, gramáticas, la caché y los directorios.",
    }
    const help_crashes {
        en: "Read the crash reports Klaub saved on this machine. Nothing is ever sent.",
        pt: "Lê os relatórios de falha que o Klaub salvou nesta máquina. Nada é enviado.",
        es: "Lee los informes de fallos que Klaub guardó en esta máquina. Nunca se envía nada.",
    }
    const help_crashes_list {
        en: "List the saved reports, oldest first.",
        pt: "Lista os relatórios salvos, do mais antigo ao mais recente.",
        es: "Lista los informes guardados, del más antiguo al más reciente.",
    }
    const help_crashes_show {
        en: "Print one report. Read it before sharing it.",
        pt: "Imprime um relatório. Leia-o antes de compartilhá-lo.",
        es: "Imprime un informe. Léelo antes de compartirlo.",
    }
    const help_crashes_name {
        en: "The report file name from `list`; the newest when omitted.",
        pt: "O nome do arquivo do relatório em `list`; o mais recente quando omitido.",
        es: "El nombre del archivo del informe de `list`; el más reciente si se omite.",
    }
    const help_version {
        en: "Print version and build information.",
        pt: "Mostra a versão e informações da compilação.",
        es: "Muestra la versión e información de la compilación.",
    }
    const help_completions {
        en: "Print a shell completion script.",
        pt: "Imprime um script de autocompletar para o shell.",
        es: "Imprime un script de autocompletado para el shell.",
    }
    const help_completions_shell {
        en: "The shell to generate completions for.",
        pt: "O shell para o qual gerar o autocompletar.",
        es: "El shell para el que generar el autocompletado.",
    }
}
