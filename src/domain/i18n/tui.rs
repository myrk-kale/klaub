//! Sentences of the interactive reader.
#![allow(missing_docs)] // The name of each entry says what it is for.

// ---- words for states and kinds (shown in lists, chips and the status bar) ----------------------

texts! {
    const mode_queue {
        en: "Queue",
        pt: "Fila",
        es: "Cola",
    }
    const mode_structure {
        en: "Structure",
        pt: "Estrutura",
        es: "Estructura",
    }
    const mode_files {
        en: "Files",
        pt: "Arquivos",
        es: "Archivos",
    }
    const mode_code {
        en: "Code",
        pt: "Código",
        es: "Código",
    }
    const entrypoint_external {
        en: "external",
        pt: "externo",
        es: "externo",
    }
    const entrypoint_internal {
        en: "internal",
        pt: "interno",
        es: "interno",
    }

    const review_draft {
        en: "draft",
        pt: "rascunho",
        es: "borrador",
    }
    const review_finished {
        en: "finished",
        pt: "concluída",
        es: "terminada",
    }
    const review_prepared {
        en: "prepared",
        pt: "preparada",
        es: "preparada",
    }
    const review_submitted {
        en: "submitted",
        pt: "enviada",
        es: "enviada",
    }

    const change_added {
        en: "added",
        pt: "adicionado",
        es: "añadido",
    }
    const change_removed {
        en: "removed",
        pt: "removido",
        es: "eliminado",
    }
    const change_modified {
        en: "modified",
        pt: "modificado",
        es: "modificado",
    }
    const change_moved {
        en: "moved",
        pt: "movido",
        es: "movido",
    }
    const change_renamed {
        en: "renamed",
        pt: "renomeado",
        es: "renombrado",
    }
    const change_signature {
        en: "signature",
        pt: "assinatura",
        es: "firma",
    }
    const change_rewritten {
        en: "rewritten",
        pt: "reescrito",
        es: "reescrito",
    }
    const change_format {
        en: "format",
        pt: "formato",
        es: "formato",
    }
    const change_unknown {
        en: "unknown",
        pt: "indefinido",
        es: "incierto",
    }

    const tag_contract {
        en: "contract",
        pt: "contrato",
        es: "contrato",
    }
    const tag_new {
        en: "new",
        pt: "novo",
        es: "nuevo",
    }
    const tag_removed {
        en: "removed",
        pt: "removido",
        es: "eliminado",
    }
    const tag_rewrite {
        en: "rewrite",
        pt: "reescrita",
        es: "reescritura",
    }
    const tag_moved {
        en: "moved",
        pt: "movido",
        es: "movido",
    }
    const tag_format {
        en: "format",
        pt: "formato",
        es: "formato",
    }
    const tag_no_tests {
        en: "no-tests",
        pt: "sem-testes",
        es: "sin-pruebas",
    }
    const tag_unknown {
        en: "unknown",
        pt: "incerto",
        es: "incierto",
    }
    const tag_generated {
        en: "generated",
        pt: "gerado",
        es: "generado",
    }

    const component_contract {
        en: "contract",
        pt: "contrato",
        es: "contrato",
    }
    const component_reach {
        en: "reach",
        pt: "alcance",
        es: "alcance",
    }
    const component_criticality {
        en: "criticality",
        pt: "criticidade",
        es: "criticidad",
    }
    const component_complexity {
        en: "complexity",
        pt: "complexidade",
        es: "complejidad",
    }
    const component_risk {
        en: "risk",
        pt: "risco",
        es: "riesgo",
    }
    const component_tests {
        en: "tests",
        pt: "testes",
        es: "pruebas",
    }
    const component_architecture {
        en: "architecture",
        pt: "arquitetura",
        es: "arquitectura",
    }
    const component_mitigation {
        en: "mitigation",
        pt: "mitigação",
        es: "mitigación",
    }

    const reach_local {
        en: "local",
        pt: "local",
        es: "local",
    }
    const reach_module {
        en: "module",
        pt: "módulo",
        es: "módulo",
    }
    const reach_cross_module {
        en: "cross-module",
        pt: "entre módulos",
        es: "entre módulos",
    }
    const reach_global {
        en: "global",
        pt: "global",
        es: "global",
    }

    const temporal_introduced {
        en: "introduced",
        pt: "introduzido",
        es: "introducido",
    }
    const temporal_preexisting {
        en: "preexisting",
        pt: "preexistente",
        es: "preexistente",
    }
    const temporal_removed {
        en: "removed",
        pt: "removido",
        es: "eliminado",
    }
    const temporal_unknown {
        en: "unknown origin",
        pt: "origem desconhecida",
        es: "origen desconocido",
    }

    const theme_light {
        en: "Light",
        pt: "Claro",
        es: "Claro",
    }
    const theme_high_contrast {
        en: "High contrast",
        pt: "Alto contraste",
        es: "Alto contraste",
    }

}

// ---- queue filters ------------------------------------------------------------------------------

texts! {
    const filter_unreviewed {
        en: "unreviewed",
        pt: "não revisadas",
        es: "sin revisar",
    }
    const filter_introduced {
        en: "introduced",
        pt: "introduzidas",
        es: "introducidas",
    }
    const filter_no_tests {
        en: "no-tests",
        pt: "sem-testes",
        es: "sin-pruebas",
    }
    const filter_new {
        en: "new",
        pt: "novas",
        es: "nuevas",
    }
}

messages! {
    fn filter_scope(scope: &str) {
        en: "scope ≥{scope}",
        pt: "alcance ≥{scope}",
        es: "alcance ≥{scope}",
    }
    fn filter_module(module: &str) {
        en: "module {module}",
        pt: "módulo {module}",
        es: "módulo {module}",
    }
}

// ---- main area: queue ---------------------------------------------------------------------------

texts! {
    const queue_nothing_changed {
        en: "Nothing changed relative to the base: there is nothing to review.",
        pt: "Nada mudou em relação à base: não há nada para revisar.",
        es: "Nada cambió respecto a la base: no hay nada que revisar.",
    }
    const queue_no_match {
        en: "No unit matches the filters. x clears them.",
        pt: "Nenhuma unidade corresponde aos filtros. x limpa os filtros.",
        es: "Ninguna unidad coincide con los filtros. x los limpia.",
    }
    const explain_why {
        en: "Why this priority",
        pt: "Por que esta prioridade",
        es: "Por qué esta prioridad",
    }
    const explain_no_factor {
        en: "  no contributing factor",
        pt: "  nenhum fator contribuinte",
        es: "  ningún factor que contribuya",
    }
    const explain_findings {
        en: "Findings",
        pt: "Achados",
        es: "Hallazgos",
    }
    const explain_false_positive {
        en: "A false positive? Press the finding's number to silence its rule for this review.",
        pt: "Falso positivo? Pressione o número do achado para silenciar a regra nesta revisão.",
        es: "¿Falso positivo? Pulsa el número del hallazgo para silenciar su regla en esta revisión.",
    }
}

messages! {
    fn list_count(visible: usize, total: usize, summary: &str) {
        en: "{visible} of {total} unit(s){summary}",
        pt: "{visible} de {total} unidade(s){summary}",
        es: "{visible} de {total} unidad(es){summary}",
    }
    fn files_count(count: usize) {
        en: "{count} changed file(s)",
        pt: "{count} arquivo(s) alterado(s)",
        es: "{count} archivo(s) modificado(s)",
    }
    fn queue_filter_summary(filters: &str) {
        en: " · filter: {filters}",
        pt: " · filtro: {filters}",
        es: " · filtro: {filters}",
    }
    fn explain_score(score: i32, label: &str) {
        en: " score {score} · {label}",
        pt: " pontuação {score} · {label}",
        es: " puntuación {score} · {label}",
    }
    fn explain_silenced(rules: &str) {
        en: "Silenced here: {rules}",
        pt: "Silenciadas aqui: {rules}",
        es: "Silenciadas aquí: {rules}",
    }
    fn explain_reach(scope: &str, entrypoints: u32) {
        en: "Reach: {scope} · {entrypoints} entrypoint(s)",
        pt: "Alcance: {scope} · {entrypoints} ponto(s) de entrada",
        es: "Alcance: {scope} · {entrypoints} punto(s) de entrada",
    }
}

// ---- main area: structure, code -----------------------------------------------------------------

texts! {
    const structure_hint {
        en: "modules → files → symbols (+ / - to open or close levels)",
        pt: "módulos → arquivos → símbolos (+ / - abrem ou fecham níveis)",
        es: "módulos → archivos → símbolos (+ / - abren o cierran niveles)",
    }
    const structure_empty {
        en: "Nothing to show with the current filters.",
        pt: "Nada para mostrar com os filtros atuais.",
        es: "Nada que mostrar con los filtros actuales.",
    }
    const code_empty {
        en: "No code is open. Pick a unit in the Queue and press Enter.",
        pt: "Nenhum código aberto. Escolha uma unidade na Fila e pressione Enter.",
        es: "Ningún código abierto. Elige una unidad en la Cola y pulsa Enter.",
    }
    const code_new {
        en: "NEW CODE",
        pt: "CÓDIGO NOVO",
        es: "CÓDIGO NUEVO",
    }
    const code_old {
        en: "OLD CODE",
        pt: "CÓDIGO ANTIGO",
        es: "CÓDIGO ANTIGUO",
    }
    const code_symbol {
        en: "symbol",
        pt: "símbolo",
        es: "símbolo",
    }
}

messages! {
    fn code_lines(from: u32, to: u32) {
        en: " · lines {from}–{to}",
        pt: " · linhas {from}–{to}",
        es: " · líneas {from}–{to}",
    }
}

// ---- main area: impact --------------------------------------------------------------------------

texts! {}

messages! {}

// ---- main area: comments, context ---------------------------------------------------------------

texts! {}

messages! {}

// ---- main area: tests ---------------------------------------------------------------------------

texts! {}

messages! {}

// ---- main area: follow-up, coverage, flow -------------------------------------------------------

texts! {}

messages! {}

// ---- frame: title bar, tabs, status bar ---------------------------------------------------------

texts! {
    const drift_banner {
        en: " worktree changed since this snapshot, so impact may mix versions. R takes a new one (asks first)",
        pt: " o worktree mudou desde este snapshot, então o impacto pode misturar versões. R toma um novo (pergunta antes)",
        es: " el worktree cambió desde este snapshot, así que el impacto puede mezclar versiones. R toma uno nuevo (pregunta antes)",
    }
}

messages! {
    fn too_small(width: u16, height: u16, min_width: u16, min_height: u16) {
        en: "Terminal too small ({width}×{height}, need at least {min_width}×{min_height}). Resize it, or press q twice to leave.",
        pt: "Terminal pequeno demais ({width}×{height}, mínimo {min_width}×{min_height}). Redimensione, ou pressione q duas vezes para sair.",
        es: "Terminal demasiado pequeño ({width}×{height}, se necesita al menos {min_width}×{min_height}). Redimensiónalo, o pulsa q dos veces para salir.",
    }
    fn title_reviewed(progress: &str) {
        en: " {progress} reviewed ",
        pt: " {progress} revisados ",
        es: " {progress} revisados ",
    }
}

// ---- key hints in the status bar ----------------------------------------------------------------

texts! {
    const hint_move {
        en: "move",
        pt: "mover",
        es: "mover",
    }
    const hint_open {
        en: "open",
        pt: "abrir",
        es: "abrir",
    }
    const hint_reviewed {
        en: "reviewed",
        pt: "revisado",
        es: "revisado",
    }
    const hint_why {
        en: "why",
        pt: "por quê",
        es: "por qué",
    }
    const hint_search {
        en: "search",
        pt: "buscar",
        es: "buscar",
    }
    const hint_clear_filters {
        en: "clear filters",
        pt: "limpar filtros",
        es: "limpiar filtros",
    }
    const hint_help {
        en: "help",
        pt: "ajuda",
        es: "ayuda",
    }
    const hint_quit {
        en: "quit",
        pt: "sair",
        es: "salir",
    }
    const hint_depth {
        en: "depth",
        pt: "profundidade",
        es: "profundidad",
    }
    const hint_contract {
        en: "contract",
        pt: "contrato",
        es: "contrato",
    }
    const hint_panel {
        en: "panel",
        pt: "painel",
        es: "panel",
    }
    const hint_file {
        en: "file",
        pt: "arquivo",
        es: "archivo",
    }
    const hint_old_new {
        en: "old/new",
        pt: "antigo/novo",
        es: "antiguo/nuevo",
    }
    const hint_change {
        en: "change",
        pt: "mudança",
        es: "cambio",
    }
    const hint_back_forward {
        en: "back/forward",
        pt: "voltar/avançar",
        es: "atrás/adelante",
    }
}

// ---- overlays -----------------------------------------------------------------------------------

texts! {
    const help_title {
        en: "Help",
        pt: "Ajuda",
        es: "Ayuda",
    }
    const help_close {
        en: "Keys — press any key to close",
        pt: "Teclas — pressione qualquer tecla para fechar",
        es: "Teclas — pulsa cualquier tecla para cerrar",
    }
    const help_palette_key {
        en: "palette",
        pt: "paleta",
        es: "paleta",
    }
    const palette_title {
        en: "Actions",
        pt: "Ações",
        es: "Acciones",
    }
    const palette_no_match {
        en: "no match",
        pt: "nenhuma correspondência",
        es: "sin coincidencias",
    }
    const search_title {
        en: "Search",
        pt: "Buscar",
        es: "Buscar",
    }
    const hint_filters_queue {
        en: " filters the queue · ",
        pt: " filtra a fila · ",
        es: " filtra la cola · ",
    }
    const hint_cancels {
        en: " cancels",
        pt: " cancela",
        es: " cancela",
    }
    const hint_confirms {
        en: " confirms · ",
        pt: " confirma · ",
        es: " confirma · ",
    }
    const hint_cancels_nothing_sent {
        en: " cancels · nothing is sent on its own",
        pt: " cancela · nada é enviado sozinho",
        es: " cancela · nada se envía solo",
    }
    const notice_continue {
        en: "Press any key to continue.",
        pt: "Pressione qualquer tecla para continuar.",
        es: "Pulsa cualquier tecla para continuar.",
    }
    const explain_gone {
        en: "This unit is no longer in the queue.",
        pt: "Esta unidade não está mais na fila.",
        es: "Esta unidad ya no está en la cola.",
    }
    const explain_title {
        en: "Priority",
        pt: "Prioridade",
        es: "Prioridad",
    }
    const contract_title {
        en: "Contract",
        pt: "Contrato",
        es: "Contrato",
    }
    const contract_none {
        en: "This unit has no contract to inspect: it is not a symbol, a migration, a dependency manifest or a settings file.",
        pt: "Esta unidade não tem contrato para inspecionar: não é um símbolo, uma migração, um manifesto de dependências nem um arquivo de configuração.",
        es: "Esta unidad no tiene contrato que inspeccionar: no es un símbolo, una migración, un manifiesto de dependencias ni un archivo de configuración.",
    }
    const contract_any_key {
        en: "any key closes",
        pt: "qualquer tecla fecha",
        es: "cualquier tecla cierra",
    }
    const contract_change {
        en: "change",
        pt: "mudança",
        es: "cambio",
    }
    const contract_confidence {
        en: "confidence",
        pt: "confiança",
        es: "confianza",
    }
    const contract_visibility {
        en: "visibility",
        pt: "visibilidade",
        es: "visibilidad",
    }
    const contract_decorators {
        en: "decorators",
        pt: "decoradores",
        es: "decoradores",
    }
    const contract_endpoints {
        en: "endpoints",
        pt: "endpoints",
        es: "endpoints",
    }
    const contract_raises {
        en: "raises",
        pt: "lança",
        es: "lanza",
    }
    const contract_effects {
        en: "effects",
        pt: "efeitos",
        es: "efectos",
    }
    const contract_members {
        en: "members",
        pt: "membros",
        es: "miembros",
    }
    const contract_reachable_from {
        en: "reachable from",
        pt: "alcançável a partir de",
        es: "alcanzable desde",
    }
    const contract_no_entrypoint {
        en: "no entrypoint found (absence of a path is not proof of no impact)",
        pt: "nenhum ponto de entrada encontrado (a ausência de um caminho não prova que não há impacto)",
        es: "ningún punto de entrada encontrado (la ausencia de un camino no prueba que no haya impacto)",
    }
    const contract_direct_callers {
        en: "direct callers",
        pt: "chamadores diretos",
        es: "llamadores directos",
    }
    const contract_evidence {
        en: "evidence",
        pt: "evidência",
        es: "evidencia",
    }
    const contract_none_word {
        en: "none",
        pt: "nenhum",
        es: "ninguno",
    }
    const contract_file {
        en: "file",
        pt: "arquivo",
        es: "archivo",
    }
    const contract_kind {
        en: "kind",
        pt: "tipo",
        es: "tipo",
    }
    const contract_read {
        en: "read",
        pt: "lido",
        es: "leído",
    }
    const contract_yes {
        en: "yes",
        pt: "sim",
        es: "sí",
    }
    const contract_no_textual {
        en: "no, use the textual diff",
        pt: "não, use o diff textual",
        es: "no, usa el diff textual",
    }
    const contract_added {
        en: "added",
        pt: "adicionado",
        es: "añadido",
    }
    const contract_removed {
        en: "removed",
        pt: "removido",
        es: "eliminado",
    }
    const contract_changed {
        en: "changed",
        pt: "alterado",
        es: "cambiado",
    }
    const contract_notes {
        en: "notes",
        pt: "notas",
        es: "notas",
    }
    const file_kind_migration {
        en: "migration",
        pt: "migração",
        es: "migración",
    }
    const file_kind_manifest {
        en: "dependency manifest",
        pt: "manifesto de dependências",
        es: "manifiesto de dependencias",
    }
    const file_kind_config {
        en: "configuration",
        pt: "configuração",
        es: "configuración",
    }
    const finish_warnings {
        en: "Warnings (they do not block):",
        pt: "Avisos (não bloqueiam):",
        es: "Avisos (no bloquean):",
    }
}

messages! {
    fn clip_more_lines(count: usize) {
        en: "… {count} more lines; make the terminal taller",
        pt: "… mais {count} linhas; aumente a altura do terminal",
        es: "… {count} líneas más; aumenta la altura del terminal",
    }
}

// ---- opening the reader ---------------------------------------------------------------------------

texts! {
    const continuing_round {
        en: "Continuing a previous round: decide each carried-over comment in Follow-up (D).",
        pt: "Continuando uma rodada anterior: decida cada comentário trazido adiante em Acompanhamento (D).",
        es: "Continuando una ronda anterior: decide cada comentario trasladado en Seguimiento (D).",
    }
    const storage_notice_title {
        en: "Where Klaub keeps its files",
        pt: "Onde o Klaub guarda seus arquivos",
        es: "Dónde guarda Klaub sus archivos",
    }
    const storage_notice_text {
        en: "Your comments, briefs and flow diagrams are saved in a .klaub/ folder inside this repository. It is local data.\n\nKlaub does not add it to .gitignore or .git/info/exclude, and it never commits it. If you do not want .klaub/ in git, add it to .gitignore yourself; if you want to share a flow diagram, commit .klaub/flows/ yourself.\n\nKlaub always leaves .klaub/ out of the changes it reviews, so saving a comment never starts a new snapshot.",
        pt: "Seus comentários, briefs e diagramas de fluxo são salvos em uma pasta .klaub/ dentro deste repositório. São dados locais.\n\nO Klaub não a adiciona ao .gitignore nem ao .git/info/exclude, e nunca faz commit dela. Se você não quer o .klaub/ no git, adicione-o ao .gitignore você mesmo; se quiser compartilhar um diagrama de fluxo, faça commit de .klaub/flows/ você mesmo.\n\nO Klaub sempre deixa o .klaub/ fora das mudanças que revisa, então salvar um comentário nunca inicia um novo snapshot.",
        es: "Tus comentarios, briefs y diagramas de flujo se guardan en una carpeta .klaub/ dentro de este repositorio. Son datos locales.\n\nKlaub no la añade a .gitignore ni a .git/info/exclude, y nunca hace commit de ella. Si no quieres .klaub/ en git, añádela tú mismo a .gitignore; si quieres compartir un diagrama de flujo, haz commit de .klaub/flows/ tú mismo.\n\nKlaub siempre deja .klaub/ fuera de los cambios que revisa, así que guardar un comentario nunca inicia un nuevo snapshot.",
    }
}

// ---- action names (palette and help) ------------------------------------------------------------

texts! {
    const action_go_queue {
        en: "Go to Queue",
        pt: "Ir para a Fila",
        es: "Ir a la Cola",
    }
    const action_go_structure {
        en: "Go to Structure",
        pt: "Ir para a Estrutura",
        es: "Ir a la Estructura",
    }
    const action_go_files {
        en: "Go to Files",
        pt: "Ir para os Arquivos",
        es: "Ir a los Archivos",
    }
    const action_go_code {
        en: "Go to Code",
        pt: "Ir para o Código",
        es: "Ir al Código",
    }
    const action_next_unreviewed {
        en: "Next unreviewed",
        pt: "Próxima não revisada",
        es: "Siguiente sin revisar",
    }
    const action_prev_unreviewed {
        en: "Previous unreviewed",
        pt: "Não revisada anterior",
        es: "Anterior sin revisar",
    }
    const action_next_critical {
        en: "Next unreviewed P0/P1",
        pt: "Próxima P0/P1 não revisada",
        es: "Siguiente P0/P1 sin revisar",
    }
    const action_toggle_reviewed {
        en: "Mark reviewed / unmark",
        pt: "Marcar como revisada / desmarcar",
        es: "Marcar como revisada / desmarcar",
    }
    const action_toggle_bookmark {
        en: "Bookmark this unit",
        pt: "Marcar esta unidade com favorito",
        es: "Marcar esta unidad como favorita",
    }
    const action_next_bookmark {
        en: "Next bookmark",
        pt: "Próximo favorito",
        es: "Siguiente favorita",
    }
    const action_explain {
        en: "Why this priority?",
        pt: "Por que esta prioridade?",
        es: "¿Por qué esta prioridad?",
    }
    const action_restore_rules {
        en: "Bring back silenced rules",
        pt: "Restaurar regras silenciadas",
        es: "Restaurar reglas silenciadas",
    }
    const action_inspect_contract {
        en: "Inspect the contract",
        pt: "Inspecionar o contrato",
        es: "Inspeccionar el contrato",
    }
    const action_toggle_inspector {
        en: "Show or hide the symbol panel",
        pt: "Mostrar ou ocultar o painel do símbolo",
        es: "Mostrar u ocultar el panel del símbolo",
    }
    const inspector_too_narrow {
        en: "The symbol panel needs a terminal at least 125 columns wide.",
        pt: "O painel do símbolo precisa de um terminal com pelo menos 125 colunas.",
        es: "El panel del símbolo necesita un terminal de al menos 125 columnas.",
    }
    const inspector_module_note {
        en: "The file's top-level code: imports, constants and anything outside a function or class.",
        pt: "O código de nível superior do arquivo: imports, constantes e tudo que fica fora de funções e classes.",
        es: "El código de nivel superior del archivo: imports, constantes y todo lo que queda fuera de funciones y clases.",
    }
    const inspector_signature {
        en: "Signature",
        pt: "Assinatura",
        es: "Firma",
    }
    const inspector_doc {
        en: "Documentation",
        pt: "Documentação",
        es: "Documentación",
    }
    const inspector_no_doc {
        en: "no documentation comment",
        pt: "sem comentário de documentação",
        es: "sin comentario de documentación",
    }
    const inspector_before {
        en: "before",
        pt: "antes",
        es: "antes",
    }
    const action_open_whole_file {
        en: "Open the unit in the whole file",
        pt: "Abrir a unidade no arquivo inteiro",
        es: "Abrir la unidad en el archivo completo",
    }
    const action_history_back {
        en: "Back to the previous unit",
        pt: "Voltar à unidade anterior",
        es: "Volver a la unidad anterior",
    }
    const action_history_forward {
        en: "Forward to the next unit in the history",
        pt: "Avançar para a próxima unidade do histórico",
        es: "Avanzar a la siguiente unidad del historial",
    }
    const action_toggle_version {
        en: "Toggle before / after",
        pt: "Alternar antes / depois",
        es: "Alternar antes / después",
    }
    const action_peek_previous {
        en: "Hold to see the previous version",
        pt: "Segure para ver a versão anterior",
        es: "Mantén pulsada para ver la versión anterior",
    }
    const action_next_change {
        en: "Next change",
        pt: "Próxima mudança",
        es: "Siguiente cambio",
    }
    const action_prev_change {
        en: "Previous change",
        pt: "Mudança anterior",
        es: "Cambio anterior",
    }
    const action_toggle_code_scope {
        en: "Code: symbol / whole file",
        pt: "Código: símbolo / arquivo inteiro",
        es: "Código: símbolo / archivo completo",
    }
    const action_search {
        en: "Search",
        pt: "Buscar",
        es: "Buscar",
    }
    const action_filter_high {
        en: "Filter: P0/P1 only",
        pt: "Filtro: somente P0/P1",
        es: "Filtro: solo P0/P1",
    }
    const action_filter_unreviewed {
        en: "Filter: unreviewed only",
        pt: "Filtro: somente não revisadas",
        es: "Filtro: solo sin revisar",
    }
    const action_filter_introduced {
        en: "Filter: introduced by this change",
        pt: "Filtro: introduzidas por esta mudança",
        es: "Filtro: introducidas por este cambio",
    }
    const action_filter_no_tests {
        en: "Filter: no associated tests",
        pt: "Filtro: sem testes associados",
        es: "Filtro: sin pruebas asociadas",
    }
    const action_filter_new {
        en: "Filter: new files",
        pt: "Filtro: arquivos novos",
        es: "Filtro: archivos nuevos",
    }
    const action_cycle_scope {
        en: "Filter: minimum reach (local/module/global)",
        pt: "Filtro: alcance mínimo (local/módulo/global)",
        es: "Filtro: alcance mínimo (local/módulo/global)",
    }
    const action_cycle_module {
        en: "Filter: next module",
        pt: "Filtro: próximo módulo",
        es: "Filtro: siguiente módulo",
    }
    const action_clear_filters {
        en: "Clear filters",
        pt: "Limpar filtros",
        es: "Limpiar filtros",
    }
    const action_deeper_tree {
        en: "Structure: show more",
        pt: "Estrutura: mostrar mais",
        es: "Estructura: mostrar más",
    }
    const action_shallower_tree {
        en: "Structure: show less",
        pt: "Estrutura: mostrar menos",
        es: "Estructura: mostrar menos",
    }
    const action_refresh {
        en: "Refresh snapshot",
        pt: "Atualizar o snapshot",
        es: "Actualizar el snapshot",
    }
    const action_next_theme {
        en: "Switch colour theme",
        pt: "Trocar o tema de cores",
        es: "Cambiar el tema de colores",
    }
    const action_help {
        en: "Help",
        pt: "Ajuda",
        es: "Ayuda",
    }
    const action_quit {
        en: "Close the reader",
        pt: "Fechar o leitor",
        es: "Cerrar el lector",
    }
}

// ---- messages in the status bar and questions to confirm ----------------------------------------

texts! {
    const select_unit_first {
        en: "Select a unit first.",
        pt: "Escolha primeiro uma unidade.",
        es: "Elige primero una unidad.",
    }
    const open_code_first {
        en: "Open some code first.",
        pt: "Abra algum código primeiro.",
        es: "Abre primero algo de código.",
    }
    const restore_rules_title {
        en: "Bring back the silenced rules?",
        pt: "Restaurar as regras silenciadas?",
        es: "¿Restaurar las reglas silenciadas?",
    }
    const restore_rules_explain {
        en: "Their findings return and the queue is recomputed on this snapshot.",
        pt: "Os achados voltam e a fila é recalculada neste snapshot.",
        es: "Los hallazgos vuelven y la cola se recalcula en este snapshot.",
    }
    const no_rule_silenced {
        en: "No rule is silenced in this review.",
        pt: "Nenhuma regra está silenciada nesta revisão.",
        es: "Ninguna regla está silenciada en esta revisión.",
    }
    const history_no_earlier {
        en: "No earlier unit in the history.",
        pt: "Não há unidade anterior no histórico.",
        es: "No hay una unidad anterior en el historial.",
    }
    const history_no_later {
        en: "No later unit in the history.",
        pt: "Não há unidade posterior no histórico.",
        es: "No hay una unidad posterior en el historial.",
    }
    const filters_cleared {
        en: "Filters cleared.",
        pt: "Filtros limpos.",
        es: "Filtros limpiados.",
    }
    const queue_is_empty {
        en: "Nothing to show: the queue is empty.",
        pt: "Nada para mostrar: a fila está vazia.",
        es: "Nada que mostrar: la cola está vacía.",
    }
    const no_unreviewed_critical {
        en: "No unreviewed P0/P1 unit left in view.",
        pt: "Nenhuma unidade P0/P1 não revisada restante na visão.",
        es: "Ninguna unidad P0/P1 sin revisar en la vista.",
    }
    const no_unreviewed {
        en: "No unreviewed unit left in view.",
        pt: "Nenhuma unidade não revisada restante na visão.",
        es: "Ninguna unidad sin revisar en la vista.",
    }
    const mark_removed {
        en: "Mark removed.",
        pt: "Marca removida.",
        es: "Marca quitada.",
    }
    const bookmark_removed {
        en: "Bookmark removed.",
        pt: "Favorito removido.",
        es: "Favorita quitada.",
    }
    const bookmarks_none {
        en: "No bookmarks yet. m marks the current unit.",
        pt: "Ainda não há favoritos. m marca a unidade atual.",
        es: "Aún no hay favoritas. m marca la unidad actual.",
    }
    const bookmark_only_one {
        en: "Already on the only bookmark.",
        pt: "Você já está no único favorito.",
        es: "Ya estás en la única favorita.",
    }
    const bookmark_none_in_queue {
        en: "No bookmark is still in the queue.",
        pt: "Nenhum favorito continua na fila.",
        es: "Ninguna favorita sigue en la cola.",
    }
    const refresh_title {
        en: "Take a new snapshot?",
        pt: "Tirar um novo snapshot?",
        es: "¿Tomar un nuevo snapshot?",
    }
    const refresh_line_analyse {
        en: "The changes are analysed again from the current worktree.",
        pt: "As mudanças são analisadas de novo a partir do worktree atual.",
        es: "Los cambios se analizan de nuevo desde el worktree actual.",
    }
    const refresh_line_comments {
        en: "Comments are moved to where their code went; uncertain ones become stale.",
        pt: "Os comentários vão para onde seu código foi; os incertos ficam obsoletos.",
        es: "Los comentarios se mueven adonde fue su código; los dudosos quedan obsoletos.",
    }
    const refresh_line_reviewed {
        en: "Units marked reviewed stay marked only if their code did not change.",
        pt: "Unidades marcadas como revisadas continuam marcadas só se o código não mudou.",
        es: "Las unidades marcadas como revisadas siguen marcadas solo si su código no cambió.",
    }
    const version_removed {
        en: "this code was removed; there is no new version",
        pt: "este código foi removido; não há versão nova",
        es: "este código se eliminó; no hay versión nueva",
    }
    const version_new_only {
        en: "NEW · there is no previous version to compare with",
        pt: "NOVO · não há versão anterior para comparar",
        es: "NUEVO · no hay versión anterior con la que comparar",
    }
    const change_none_further {
        en: "No further change in this code. n goes to the next unreviewed unit.",
        pt: "Não há mais mudanças neste código. n vai para a próxima unidade não revisada.",
        es: "No hay más cambios en este código. n va a la siguiente unidad sin revisar.",
    }
    const change_none_earlier {
        en: "No earlier change in this code.",
        pt: "Não há mudança anterior neste código.",
        es: "No hay un cambio anterior en este código.",
    }
}

messages! {
    fn theme_switched(name: &str, key: &str) {
        en: "Theme: {name}. Set ui.theme = \"{key}\" to keep it.",
        pt: "Tema: {name}. Defina ui.theme = \"{key}\" para mantê-lo.",
        es: "Tema: {name}. Define ui.theme = \"{key}\" para conservarlo.",
    }
    fn peek_hint(hold: char, toggle: char) {
        en: "Hold {hold} while reading code to see the previous version; {toggle} toggles it.",
        pt: "Segure {hold} ao ler código para ver a versão anterior; {toggle} alterna.",
        es: "Mantén pulsada {hold} al leer código para ver la versión anterior; {toggle} la alterna.",
    }
    fn restore_rules_silenced(rules: &str) {
        en: "silenced here: {rules}",
        pt: "silenciadas aqui: {rules}",
        es: "silenciadas aquí: {rules}",
    }
    fn filter_status(summary: &str, count: usize) {
        en: "Filter: {summary} — {count} unit(s).",
        pt: "Filtro: {summary} — {count} unidade(s).",
        es: "Filtro: {summary} — {count} unidad(es).",
    }
    fn marked_reviewed(done: usize, total: usize) {
        en: "Marked reviewed ({done}/{total}). Press r again to undo, n for the next.",
        pt: "Marcada como revisada ({done}/{total}). Pressione r de novo para desfazer, n para a próxima.",
        es: "Marcada como revisada ({done}/{total}). Pulsa r de nuevo para deshacer, n para la siguiente.",
    }
    fn bookmarked(count: usize) {
        en: "Bookmarked ({count}). , jumps to the next.",
        pt: "Favoritada ({count}). , vai para a próxima.",
        es: "Marcada como favorita ({count}). , salta a la siguiente.",
    }
}

// ---- messages after input (update) --------------------------------------------------------------

texts! {
    const peek_already_previous {
        en: "Already showing the previous version.",
        pt: "Já está mostrando a versão anterior.",
        es: "Ya se muestra la versión anterior.",
    }
    const drift_detected {
        en: "The worktree changed since this snapshot. Press R to take a new one.",
        pt: "O worktree mudou desde este snapshot. Pressione R para tirar um novo.",
        es: "El worktree cambió desde este snapshot. Pulsa R para tomar uno nuevo.",
    }
    const snapshot_taken {
        en: "New snapshot taken; comments were relocated.",
        pt: "Novo snapshot tirado; os comentários foram realocados.",
        es: "Nuevo snapshot tomado; los comentarios se reubicaron.",
    }
    const quit_again {
        en: "Press q again to close the reader. Everything is already saved.",
        pt: "Pressione q de novo para fechar o leitor. Tudo já está salvo.",
        es: "Pulsa q de nuevo para cerrar el lector. Todo ya está guardado.",
    }
    const search_cancelled {
        en: "Cancelled. Nothing was changed.",
        pt: "Cancelado. Nada foi alterado.",
        es: "Cancelado. No se cambió nada.",
    }
    const mute_title {
        en: "Silence this rule for this review?",
        pt: "Silenciar esta regra nesta revisão?",
        es: "¿Silenciar esta regla en esta revisión?",
    }
    const mute_explain_1 {
        en: "Use it for a false positive. Every finding of this rule disappears from this review",
        pt: "Use para um falso positivo. Todos os achados desta regra somem desta revisão",
        es: "Úsalo para un falso positivo. Todos los hallazgos de esta regla desaparecen de esta revisión",
    }
    const mute_explain_2 {
        en: "and the scores that counted it drop. The code under review does not change.",
        pt: "e as pontuações que os contavam diminuem. O código revisado não muda.",
        es: "y las puntuaciones que los contaban bajan. El código revisado no cambia.",
    }
    const mute_restore_hint {
        en: "Bring it back from the palette: \"Bring back silenced rules\".",
        pt: "Restaure pela paleta: \"Restaurar regras silenciadas\".",
        es: "Restáuralas desde la paleta: \"Restaurar reglas silenciadas\".",
    }
    const snapshot_taking {
        en: "Taking a new snapshot…",
        pt: "Tirando um novo snapshot…",
        es: "Tomando un nuevo snapshot…",
    }
    const rules_restored {
        en: "Silenced rules are back.",
        pt: "As regras silenciadas voltaram.",
        es: "Las reglas silenciadas han vuelto.",
    }
}

messages! {
    fn peek_unsupported(key: char, toggle: char) {
        en: "This terminal does not report key releases, so {key} cannot be held. {toggle} toggles old and new.",
        pt: "Este terminal não informa quando as teclas são soltas, então {key} não pode ser mantida pressionada. {toggle} alterna entre antigo e novo.",
        es: "Este terminal no informa cuándo se sueltan las teclas, así que {key} no se puede mantener pulsada. {toggle} alterna entre antiguo y nuevo.",
    }
    fn module_only(module: &str) {
        en: "Showing module {module} only. x clears the filter.",
        pt: "Mostrando só o módulo {module}. x limpa o filtro.",
        es: "Mostrando solo el módulo {module}. x limpia el filtro.",
    }
    fn search_matches(count: usize) {
        en: "{count} unit(s) match. x clears the search.",
        pt: "{count} unidade(s) correspondem. x limpa a busca.",
        es: "{count} unidad(es) coinciden. x limpia la búsqueda.",
    }
    fn mute_rule_line(rule: &str) {
        en: "rule:     {rule}",
        pt: "regra:     {rule}",
        es: "regla:     {rule}",
    }
    fn mute_finding_line(title: &str) {
        en: "finding:  {title}",
        pt: "achado:   {title}",
        es: "hallazgo:  {title}",
    }
    fn rule_silenced(rule: &str) {
        en: "Rule {rule} is silenced for this review.",
        pt: "A regra {rule} foi silenciada nesta revisão.",
        es: "La regla {rule} se silenció en esta revisión.",
    }
}

// ---- warnings before finishing a review -----------------------------------------------------------

texts! {
    const finish_warning_drift {
        en: "The worktree changed after this snapshot was taken, so the comments and the impact refer to code that has moved on. R takes a new snapshot.",
        pt: "O worktree mudou depois que este snapshot foi tirado, então os comentários e o impacto se referem a código que já seguiu em frente. R tira um novo snapshot.",
        es: "El worktree cambió después de tomar este snapshot, así que los comentarios y el impacto se refieren a código que ya ha cambiado. R toma un nuevo snapshot.",
    }
}

messages! {
    fn finish_warning_pending(count: usize) {
        en: "{count} P0/P1 unit(s) are not marked as reviewed",
        pt: "{count} unidade(s) P0/P1 não estão marcadas como revisadas",
        es: "{count} unidad(es) P0/P1 no están marcadas como revisadas",
    }
    fn finish_warning_criteria(count: usize) {
        en: "{count} acceptance criteria are still unchecked",
        pt: "{count} critérios de aceitação ainda não foram verificados",
        es: "{count} criterios de aceptación siguen sin verificar",
    }
    fn finish_warning_untested(count: usize) {
        en: "{count} changed symbol(s) have no associated test (which does not prove there is none)",
        pt: "{count} símbolo(s) alterado(s) não têm teste associado (o que não prova que não exista)",
        es: "{count} símbolo(s) modificado(s) no tienen prueba asociada (lo que no prueba que no exista)",
    }
}
