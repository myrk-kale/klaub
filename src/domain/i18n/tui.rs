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
    const tab_review {
        en: "Review",
        pt: "Revisão",
        es: "Revisión",
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
    const mode_impact {
        en: "Impact",
        pt: "Impacto",
        es: "Impacto",
    }
    const mode_comments {
        en: "Comments",
        pt: "Comentários",
        es: "Comentarios",
    }
    const mode_context {
        en: "Context",
        pt: "Contexto",
        es: "Contexto",
    }
    const mode_tests {
        en: "Tests",
        pt: "Testes",
        es: "Pruebas",
    }
    const mode_followup {
        en: "Follow-up",
        pt: "Acompanhamento",
        es: "Seguimiento",
    }
    const mode_coverage {
        en: "Coverage",
        pt: "Cobertura",
        es: "Cobertura",
    }
    const mode_flow {
        en: "Flow",
        pt: "Fluxo",
        es: "Flujo",
    }

    const origin_resolved {
        en: "resolved",
        pt: "resolvido",
        es: "resuelto",
    }
    const origin_heuristic {
        en: "heuristic",
        pt: "heurístico",
        es: "heurístico",
    }
    const origin_unresolved {
        en: "unresolved",
        pt: "não resolvido",
        es: "sin resolver",
    }

    const entrypoint_cli {
        en: "cli",
        pt: "cli",
        es: "cli",
    }
    const entrypoint_main {
        en: "main",
        pt: "main",
        es: "main",
    }
    const entrypoint_worker {
        en: "worker",
        pt: "worker",
        es: "worker",
    }
    const entrypoint_export {
        en: "export",
        pt: "export",
        es: "export",
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

    const edge_calls {
        en: "calls",
        pt: "chama",
        es: "llama",
    }
    const edge_imports {
        en: "imports",
        pt: "importa",
        es: "importa",
    }
    const edge_exports {
        en: "exports",
        pt: "exporta",
        es: "exporta",
    }
    const edge_implements {
        en: "implements",
        pt: "implementa",
        es: "implementa",
    }
    const edge_extends {
        en: "extends",
        pt: "estende",
        es: "extiende",
    }
    const edge_references {
        en: "references",
        pt: "referencia",
        es: "referencia",
    }

    const comment_draft {
        en: "draft",
        pt: "rascunho",
        es: "borrador",
    }
    const comment_prepared {
        en: "prepared",
        pt: "preparado",
        es: "preparado",
    }
    const comment_submitted {
        en: "submitted",
        pt: "enviado",
        es: "enviado",
    }
    const comment_addressed {
        en: "addressed",
        pt: "tratado",
        es: "atendido",
    }
    const comment_unresolved {
        en: "unresolved",
        pt: "em aberto",
        es: "abierto",
    }
    const comment_stale {
        en: "stale",
        pt: "obsoleto",
        es: "obsoleto",
    }

    const criterion_not_checked {
        en: "not checked",
        pt: "não verificado",
        es: "sin verificar",
    }
    const criterion_verified {
        en: "verified",
        pt: "verificado",
        es: "verificado",
    }
    const criterion_pending {
        en: "pending",
        pt: "pendente",
        es: "pendiente",
    }
    const criterion_not_applicable {
        en: "n/a",
        pt: "n/a",
        es: "n/a",
    }

    const outcome_passed {
        en: "passed",
        pt: "passou",
        es: "pasó",
    }
    const outcome_failed {
        en: "FAILED",
        pt: "FALHOU",
        es: "FALLÓ",
    }
    const outcome_skipped {
        en: "skipped",
        pt: "ignorado",
        es: "omitido",
    }
    const outcome_not_run {
        en: "not run",
        pt: "não executado",
        es: "no ejecutado",
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

    const evidence_no_association {
        en: "no association found",
        pt: "nenhuma associação encontrada",
        es: "ninguna asociación encontrada",
    }
    const evidence_not_run {
        en: "not run",
        pt: "não executado",
        es: "no ejecutado",
    }
    const evidence_passed {
        en: "passed",
        pt: "passou",
        es: "pasó",
    }
    const evidence_failed {
        en: "failed",
        pt: "falhou",
        es: "falló",
    }
    const evidence_stale {
        en: "stale",
        pt: "desatualizado",
        es: "desactualizado",
    }

    const link_name {
        en: "name",
        pt: "nome",
        es: "nombre",
    }
    const link_import {
        en: "import",
        pt: "import",
        es: "import",
    }
    const link_reference {
        en: "reference",
        pt: "referência",
        es: "referencia",
    }
    const link_configured {
        en: "configured",
        pt: "configurado",
        es: "configurado",
    }

    const followup_pending {
        en: "pending",
        pt: "pendente",
        es: "pendiente",
    }
    const followup_resolved {
        en: "resolved",
        pt: "resolvido",
        es: "resuelto",
    }
    const followup_partial {
        en: "partial",
        pt: "parcial",
        es: "parcial",
    }
    const followup_unresolved {
        en: "unresolved",
        pt: "em aberto",
        es: "abierto",
    }
    const followup_regression {
        en: "regression",
        pt: "regressão",
        es: "regresión",
    }
    const followup_stale {
        en: "stale",
        pt: "obsoleto",
        es: "obsoleto",
    }

    const flow_current {
        en: "current",
        pt: "atual",
        es: "actual",
    }
    const flow_missing {
        en: "missing",
        pt: "ausente",
        es: "ausente",
    }
    const flow_invalid {
        en: "invalid",
        pt: "inválido",
        es: "inválido",
    }
    const flow_stale {
        en: "stale",
        pt: "desatualizado",
        es: "desactualizado",
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

    const severity_none {
        en: "none",
        pt: "nenhuma",
        es: "ninguna",
    }
    const severity_label {
        en: "severity ",
        pt: "severidade ",
        es: "severidad ",
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
    const queue_details {
        en: "Details",
        pt: "Detalhes",
        es: "Detalles",
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
    fn queue_title(visible: usize, total: usize, summary: &str) {
        en: "Queue · {visible} of {total} unit(s){summary}",
        pt: "Fila · {visible} de {total} unidade(s){summary}",
        es: "Cola · {visible} de {total} unidad(es){summary}",
    }
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
    fn code_more_comments(count: usize) {
        en: " (+{count} more)",
        pt: " (+{count} a mais)",
        es: " (+{count} más)",
    }
}

// ---- main area: impact --------------------------------------------------------------------------

texts! {
    const impact_no_subject {
        en: "no symbol",
        pt: "nenhum símbolo",
        es: "ningún símbolo",
    }
    const impact_whole_file {
        en: "This unit is a whole file, not a symbol, so it has no call graph.",
        pt: "Esta unidade é um arquivo inteiro, não um símbolo, por isso não tem grafo de chamadas.",
        es: "Esta unidad es un archivo completo, no un símbolo, así que no tiene grafo de llamadas.",
    }
    const impact_not_indexed {
        en: "This symbol is not in the call graph (not indexed, or in an unsupported language).",
        pt: "Este símbolo não está no grafo de chamadas (não foi indexado ou está em uma linguagem sem suporte).",
        es: "Este símbolo no está en el grafo de llamadas (no se indexó o está en un lenguaje sin soporte).",
    }
    const impact_depth_limit {
        en: " · depth limit reached",
        pt: " · limite de profundidade atingido",
        es: " · límite de profundidad alcanzado",
    }
    const impact_entrypoints {
        en: "Entrypoints that can reach it",
        pt: "Pontos de entrada que chegam até ele",
        es: "Puntos de entrada que llegan hasta él",
    }
    const impact_no_entrypoints {
        en: "none found (it may be reached in ways the analysis cannot see)",
        pt: "nenhum encontrado (pode ser alcançado de formas que a análise não enxerga)",
        es: "ninguno encontrado (puede alcanzarse de formas que el análisis no ve)",
    }
    const impact_callers {
        en: "Callers",
        pt: "Chamadores",
        es: "Llamadores",
    }
    const impact_no_callers {
        en: "no callers found",
        pt: "nenhum chamador encontrado",
        es: "ningún llamador encontrado",
    }
    const impact_calls {
        en: "Calls",
        pt: "Chamadas",
        es: "Llamadas",
    }
    const impact_no_calls {
        en: "no calls into the project",
        pt: "nenhuma chamada para dentro do projeto",
        es: "ninguna llamada hacia el proyecto",
    }
}

messages! {
    fn impact_title(subject: &str) {
        en: "Impact · {subject}",
        pt: "Impacto · {subject}",
        es: "Impacto · {subject}",
    }
    fn impact_summary(scope: &str, direct: usize, entrypoints: usize, external: usize) {
        en: "{scope} · {direct} direct caller(s) · {entrypoints} entrypoint(s), {external} external",
        pt: "{scope} · {direct} chamador(es) direto(s) · {entrypoints} ponto(s) de entrada, {external} externo(s)",
        es: "{scope} · {direct} llamador(es) directo(s) · {entrypoints} punto(s) de entrada, {external} externo(s)",
    }
    fn impact_evidence(resolved: u32, heuristic: u32, unresolved: u32, tail: &str) {
        en: "evidence: {resolved} resolved · {heuristic} heuristic · {unresolved} unresolved call(s){tail}",
        pt: "evidência: {resolved} resolvida(s) · {heuristic} heurística(s) · {unresolved} chamada(s) não resolvida(s){tail}",
        es: "evidencia: {resolved} resuelta(s) · {heuristic} heurística(s) · {unresolved} llamada(s) sin resolver{tail}",
    }
    fn impact_cannot_show(symbol: &str) {
        en: "Cannot show {symbol}: the file is unreadable or not indexed.",
        pt: "Não é possível mostrar {symbol}: o arquivo está ilegível ou não foi indexado.",
        es: "No se puede mostrar {symbol}: el archivo es ilegible o no está indexado.",
    }
}

// ---- main area: comments, context ---------------------------------------------------------------

texts! {
    const comments_empty {
        en: "No comments yet. Open some code and press c.",
        pt: "Ainda não há comentários. Abra algum código e pressione c.",
        es: "Aún no hay comentarios. Abre algo de código y pulsa c.",
    }
    const comment_target_file {
        en: "file",
        pt: "arquivo",
        es: "archivo",
    }
    const context_hint {
        en: "intent and acceptance criteria (you decide what is verified)",
        pt: "intenção e critérios de aceitação (você decide o que está verificado)",
        es: "intención y criterios de aceptación (tú decides qué está verificado)",
    }
    const context_intent {
        en: "Intent  ",
        pt: "Intenção  ",
        es: "Intención  ",
    }
    const context_intent_unset {
        en: "(not set — Enter to write it)",
        pt: "(não definida — Enter para escrevê-la)",
        es: "(sin definir — Enter para escribirla)",
    }
    const context_untrusted {
        en: "Text from a brief file is untrusted data. Enter changes status, a adds a criterion, i edits the intent.",
        pt: "O texto de um arquivo de brief não é confiável. Enter muda o status, a adiciona um critério, i edita a intenção.",
        es: "El texto de un archivo de brief no es de confianza. Enter cambia el estado, a añade un criterio, i edita la intención.",
    }
}

messages! {
    fn comments_title(total: usize) {
        en: "Comments · {total}",
        pt: "Comentários · {total}",
        es: "Comentarios · {total}",
    }
    fn comments_hint(edit: char, delete: char) {
        en: "(Enter opens the code, {edit} edit, {delete} delete drafts)",
        pt: "(Enter abre o código, {edit} edita, {delete} exclui rascunhos)",
        es: "(Enter abre el código, {edit} edita, {delete} elimina borradores)",
    }
    fn comment_detail(status: &str, confidence: &str, target: &str) {
        en: "{status} · anchor confidence {confidence}% · {target}",
        pt: "{status} · confiança da âncora {confidence}% · {target}",
        es: "{status} · confianza del ancla {confidence}% · {target}",
    }
}

// ---- main area: tests ---------------------------------------------------------------------------

texts! {
    const tests_select_unit {
        en: "Select a unit in the Queue first.",
        pt: "Escolha primeiro uma unidade na Fila.",
        es: "Elige primero una unidad en la Cola.",
    }
    const tests_hint {
        en: "(X runs them after asking, U reloads reports)",
        pt: "(X executa após perguntar, U recarrega os relatórios)",
        es: "(X los ejecuta tras preguntar, U recarga los informes)",
    }
    const tests_evidence {
        en: "Evidence: ",
        pt: "Evidência: ",
        es: "Evidencia: ",
    }
    const tests_none_linked {
        en: "No test was linked to this symbol. That does not prove there is none: Klaub only links tests it can see calling or importing it.",
        pt: "Nenhum teste foi associado a este símbolo. Isso não prova que não exista: o Klaub só associa testes que vê chamando ou importando o símbolo.",
        es: "Ninguna prueba se asoció a este símbolo. Eso no prueba que no exista: Klaub solo asocia las pruebas que ve llamándolo o importándolo.",
    }
    const tests_not_run {
        en: "Tests are associated, but no imported report has a result for them.",
        pt: "Há testes associados, mas nenhum relatório importado tem resultado para eles.",
        es: "Hay pruebas asociadas, pero ningún informe importado tiene resultado para ellas.",
    }
    const tests_passed {
        en: "Every associated test with a result passed in an imported report.",
        pt: "Todos os testes associados com resultado passaram em um relatório importado.",
        es: "Todas las pruebas asociadas con resultado pasaron en un informe importado.",
    }
    const tests_failed {
        en: "At least one associated test failed in an imported report.",
        pt: "Pelo menos um teste associado falhou em um relatório importado.",
        es: "Al menos una prueba asociada falló en un informe importado.",
    }
    const tests_stale {
        en: "The report is older than the newest change; do not trust it.",
        pt: "O relatório é mais antigo que a mudança mais recente; não confie nele.",
        es: "El informe es más antiguo que el cambio más reciente; no confíes en él.",
    }
    const tests_associated {
        en: "Associated tests",
        pt: "Testes associados",
        es: "Pruebas asociadas",
    }
    const tests_imported {
        en: "Imported results",
        pt: "Resultados importados",
        es: "Resultados importados",
    }
    const tests_no_report {
        en: "No test report is configured or found (tests.report_paths).",
        pt: "Nenhum relatório de testes configurado ou encontrado (tests.report_paths).",
        es: "Ningún informe de pruebas configurado o encontrado (tests.report_paths).",
    }
    const tests_stale_suffix {
        en: " · STALE",
        pt: " · DESATUALIZADO",
        es: " · DESACTUALIZADO",
    }
    const tests_output {
        en: "Output of the last run (tail)",
        pt: "Saída da última execução (final)",
        es: "Salida de la última ejecución (final)",
    }
}

messages! {
    fn tests_title(unit: &str) {
        en: "Tests · {unit}",
        pt: "Testes · {unit}",
        es: "Pruebas · {unit}",
    }
    fn tests_link(name: &str, file: &str, line: u32, method: &str, confidence: &str) {
        en: "  {name} ({file}:{line}) via {method} · {confidence}%",
        pt: "  {name} ({file}:{line}) via {method} · {confidence}%",
        es: "  {name} ({file}:{line}) vía {method} · {confidence}%",
    }
    fn tests_result(name: &str, outcome: &str) {
        en: "  {name} — {outcome}",
        pt: "  {name} — {outcome}",
        es: "  {name} — {outcome}",
    }
    fn tests_coverage(file: &str, covered: usize, missed: usize) {
        en: "Coverage of {file}: {covered} line(s) covered, {missed} missed (file level, from an imported report)",
        pt: "Cobertura de {file}: {covered} linha(s) coberta(s), {missed} não coberta(s) (nível de arquivo, de um relatório importado)",
        es: "Cobertura de {file}: {covered} línea(s) cubierta(s), {missed} sin cubrir (nivel de archivo, de un informe importado)",
    }
    fn tests_report(path: &str, stale: &str) {
        en: "Report: {path}{stale}",
        pt: "Relatório: {path}{stale}",
        es: "Informe: {path}{stale}",
    }
}

// ---- main area: follow-up, coverage, flow -------------------------------------------------------

texts! {
    const followup_hint {
        en: "(y resolved · p partial · u unresolved · X regression)",
        pt: "(y resolvido · p parcial · u em aberto · X regressão)",
        es: "(y resuelto · p parcial · u abierto · X regresión)",
    }
    const followup_empty {
        en: "No comments were carried over. After you hand feedback to the agent and it changes the code, opening a new review continues from here.",
        pt: "Nenhum comentário foi trazido adiante. Depois que você entregar o feedback ao agente e ele mudar o código, abrir uma nova revisão continua a partir daqui.",
        es: "Ningún comentario se trajo de la ronda anterior. Cuando entregues el feedback al agente y este cambie el código, abrir una nueva revisión continúa desde aquí.",
    }
    const coverage_hint {
        en: "what you marked as reviewed (never an approval)",
        pt: "o que você marcou como revisado (nunca é uma aprovação)",
        es: "lo que marcaste como revisado (nunca es una aprobación)",
    }
    const coverage_overall {
        en: "Overall  ",
        pt: "Geral  ",
        es: "General  ",
    }
    const flow_hint {
        en: "(Enter opens a step · ←/→/Space fold · p asks the agent · u reloads)",
        pt: "(Enter abre um passo · ←/→/Espaço recolhem · p pede ao agente · u recarrega)",
        es: "(Enter abre un paso · ←/→/Espacio pliegan · p pide al agente · u recarga)",
    }
    const flow_not_found {
        en: "Flow diagram not found",
        pt: "Diagrama de fluxo não encontrado",
        es: "Diagrama de flujo no encontrado",
    }
    const flow_press_p {
        en: "Press p to prepare a request for the agent (you confirm first, and Enter is never sent).",
        pt: "Pressione p para preparar um pedido ao agente (você confirma antes, e Enter nunca é enviado).",
        es: "Pulsa p para preparar una petición al agente (confirmas antes, y Enter nunca se envía).",
    }
    const flow_works_without {
        en: "The review works fully without a diagram.",
        pt: "A revisão funciona por completo sem um diagrama.",
        es: "La revisión funciona por completo sin un diagrama.",
    }
    const flow_unusable {
        en: "The flow file cannot be used",
        pt: "O arquivo de fluxo não pode ser usado",
        es: "El archivo de flujo no se puede usar",
    }
    const flow_fix {
        en: "Fix the file or ask the agent again (p). It is never used partially.",
        pt: "Corrija o arquivo ou peça ao agente de novo (p). Ele nunca é usado parcialmente.",
        es: "Corrige el archivo o pídeselo de nuevo al agente (p). Nunca se usa parcialmente.",
    }
    const flow_external {
        en: "external",
        pt: "externo",
        es: "externo",
    }
    const flow_symbol_missing {
        en: " ⚠ symbol not found",
        pt: " ⚠ símbolo não encontrado",
        es: " ⚠ símbolo no encontrado",
    }
    const flow_changed {
        en: " [changed]",
        pt: " [alterado]",
        es: " [cambiado]",
    }
    const flow_shown_above {
        en: " ↺ shown above",
        pt: " ↺ já mostrado acima",
        es: " ↺ ya mostrado arriba",
    }
}

messages! {
    fn followup_title(count: usize) {
        en: "Follow-up · {count} comment(s) from the previous round",
        pt: "Acompanhamento · {count} comentário(s) da rodada anterior",
        es: "Seguimiento · {count} comentario(s) de la ronda anterior",
    }
    fn coverage_unknown(count: usize) {
        en: "  {count} unit(s) could not be analysed semantically; they are counted, not hidden.",
        pt: "  {count} unidade(s) não puderam ser analisadas semanticamente; elas são contadas, não escondidas.",
        es: "  {count} unidad(es) no se pudieron analizar semánticamente; se cuentan, no se ocultan.",
    }
    fn coverage_pending_critical(count: usize) {
        en: "  {count} P0/P1 unit(s) not reviewed yet (N jumps to the next)",
        pt: "  {count} unidade(s) P0/P1 ainda não revisada(s) (N pula para a próxima)",
        es: "  {count} unidad(es) P0/P1 aún sin revisar (N salta a la siguiente)",
    }
    fn coverage_notice(note: &str) {
        en: "  notice: {note}",
        pt: "  aviso: {note}",
        es: "  aviso: {note}",
    }
    fn flow_title(state: &str) {
        en: "Flow · {state}",
        pt: "Fluxo · {state}",
        es: "Flujo · {state}",
    }
    fn flow_expected(path: &str) {
        en: "Expected at {path}",
        pt: "Esperado em {path}",
        es: "Esperado en {path}",
    }
    fn flow_snapshot_mismatch(found: &str) {
        en: "Written for snapshot {found} — not this one. Treat it as background only.",
        pt: "Escrito para o snapshot {found} — não este. Trate como contexto apenas.",
        es: "Escrito para el snapshot {found} — no este. Trátalo solo como contexto.",
    }
    fn flow_written_by(author: &str) {
        en: "Written by {author} — an explanation, not proof. ● resolved ◐ heuristic ○ unresolved",
        pt: "Escrito por {author} — uma explicação, não uma prova. ● resolvido ◐ heurístico ○ não resolvido",
        es: "Escrito por {author} — una explicación, no una prueba. ● resuelto ◐ heurístico ○ sin resolver",
    }
    fn flow_path(crumbs: &str) {
        en: "path: {crumbs}",
        pt: "caminho: {crumbs}",
        es: "ruta: {crumbs}",
    }
}

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
    fn title_progress(progress: &str, state: &str) {
        en: " {progress} reviewed · {state} ",
        pt: " {progress} revisados · {state} ",
        es: " {progress} revisados · {state} ",
    }
}

// ---- key hints in the status bar ----------------------------------------------------------------

texts! {
    const hint_move {
        en: "move",
        pt: "mover",
        es: "mover",
    }
    const hint_scroll {
        en: "scroll",
        pt: "rolar",
        es: "desplazar",
    }
    const hint_open {
        en: "open",
        pt: "abrir",
        es: "abrir",
    }
    const hint_comment {
        en: "comment",
        pt: "comentar",
        es: "comentar",
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
    const hint_select {
        en: "select",
        pt: "selecionar",
        es: "seleccionar",
    }
    const hint_back_forward {
        en: "back/forward",
        pt: "voltar/avançar",
        es: "atrás/adelante",
    }
    const hint_open_symbol {
        en: "open the symbol",
        pt: "abrir o símbolo",
        es: "abrir el símbolo",
    }
    const hint_back_to_queue {
        en: "back to the queue",
        pt: "voltar à fila",
        es: "volver a la cola",
    }
    const hint_open_code {
        en: "open the code",
        pt: "abrir o código",
        es: "abrir el código",
    }
    const hint_edit {
        en: "edit",
        pt: "editar",
        es: "editar",
    }
    const hint_delete_draft {
        en: "delete a draft",
        pt: "excluir rascunho",
        es: "eliminar borrador",
    }
    const hint_set_intent {
        en: "set intent",
        pt: "definir intenção",
        es: "definir intención",
    }
    const hint_add_criterion {
        en: "add criterion",
        pt: "adicionar critério",
        es: "añadir criterio",
    }
    const hint_run_tests {
        en: "run (asks first)",
        pt: "executar (pergunta antes)",
        es: "ejecutar (pregunta antes)",
    }
    const hint_reload_reports {
        en: "reload reports",
        pt: "recarregar relatórios",
        es: "recargar informes",
    }
    const hint_resolved {
        en: "resolved",
        pt: "resolvido",
        es: "resuelto",
    }
    const hint_partial {
        en: "partial",
        pt: "parcial",
        es: "parcial",
    }
    const hint_not_resolved {
        en: "not resolved",
        pt: "não resolvido",
        es: "sin resolver",
    }
    const hint_regression {
        en: "regression",
        pt: "regressão",
        es: "regresión",
    }
    const hint_open_pending {
        en: "open a pending unit",
        pt: "abrir uma unidade pendente",
        es: "abrir una unidad pendiente",
    }
    const hint_ask_diagram {
        en: "ask the agent for a diagram",
        pt: "pedir um diagrama ao agente",
        es: "pedir un diagrama al agente",
    }
    const hint_reload {
        en: "reload",
        pt: "recarregar",
        es: "recargar",
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
    const hint_saves {
        en: " saves · ",
        pt: " salva · ",
        es: " guarda · ",
    }
    const hint_new_line {
        en: " new line · ",
        pt: " nova linha · ",
        es: " nueva línea · ",
    }
    const hint_severity {
        en: " severity · ",
        pt: " severidade · ",
        es: " severidad · ",
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
    const hint_finishes {
        en: " finishes · ",
        pt: " conclui · ",
        es: " termina · ",
    }
    const hint_goes_back {
        en: " goes back",
        pt: " volta",
        es: " vuelve",
    }
    const hint_chooses {
        en: " chooses · ",
        pt: " escolhe · ",
        es: " elige · ",
    }
    const hint_closes_finished {
        en: " closes (the review stays finished)",
        pt: " fecha (a revisão continua concluída)",
        es: " cierra (la revisión sigue terminada)",
    }
    const editor_empty {
        en: "(empty)",
        pt: "(vazio)",
        es: "(vacío)",
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
    const contract_data_type {
        en: "data type",
        pt: "tipo de dado",
        es: "tipo de dato",
    }
    const contract_before {
        en: "before",
        pt: "antes",
        es: "antes",
    }
    const contract_after {
        en: "after",
        pt: "depois",
        es: "después",
    }
    const contract_parameters {
        en: "parameters",
        pt: "parâmetros",
        es: "parámetros",
    }
    const contract_returns {
        en: "returns",
        pt: "retorno",
        es: "devuelve",
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
    const finish_title {
        en: "Finish the review",
        pt: "Concluir a revisão",
        es: "Terminar la revisión",
    }
    const finish_intro {
        en: "Finishing freezes the comments into a feedback file. It sends nothing.",
        pt: "Concluir congela os comentários em um arquivo de feedback. Não envia nada.",
        es: "Terminar congela los comentarios en un archivo de feedback. No envía nada.",
    }
    const finish_no_warnings {
        en: "✓ No warnings.",
        pt: "✓ Sem avisos.",
        es: "✓ Sin avisos.",
    }
    const finish_warnings {
        en: "Warnings (they do not block):",
        pt: "Avisos (não bloqueiam):",
        es: "Avisos (no bloquean):",
    }
    const deliver_title {
        en: "Deliver the feedback",
        pt: "Entregar o feedback",
        es: "Entregar el feedback",
    }
    const deliver_pasted {
        en: "The instruction that would be pasted:",
        pt: "A instrução que seria colada:",
        es: "La instrucción que se pegaría:",
    }
    const delivery_prepare {
        en: "Prepare in agent",
        pt: "Preparar no agente",
        es: "Preparar en el agente",
    }
    const delivery_copy {
        en: "Copy feedback",
        pt: "Copiar feedback",
        es: "Copiar feedback",
    }
    const delivery_show {
        en: "Show file",
        pt: "Mostrar arquivo",
        es: "Mostrar archivo",
    }
    const delivery_close {
        en: "Close",
        pt: "Fechar",
        es: "Cerrar",
    }
}

messages! {
    fn clip_more_lines(count: usize) {
        en: "… {count} more lines; make the terminal taller",
        pt: "… mais {count} linhas; aumente a altura do terminal",
        es: "… {count} líneas más; aumenta la altura del terminal",
    }
    fn deliver_saved_as(id: &str) {
        en: "Saved as {id}",
        pt: "Salvo como {id}",
        es: "Guardado como {id}",
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
    const action_go_impact {
        en: "Go to Impact (callers, entrypoints)",
        pt: "Ir para o Impacto (chamadores, pontos de entrada)",
        es: "Ir al Impacto (llamadores, puntos de entrada)",
    }
    const action_go_comments {
        en: "Go to Comments",
        pt: "Ir para os Comentários",
        es: "Ir a los Comentarios",
    }
    const action_go_context {
        en: "Go to Context (brief, criteria)",
        pt: "Ir para o Contexto (brief, critérios)",
        es: "Ir al Contexto (brief, criterios)",
    }
    const action_go_tests {
        en: "Go to Tests",
        pt: "Ir para os Testes",
        es: "Ir a las Pruebas",
    }
    const action_go_followup {
        en: "Go to Follow-up",
        pt: "Ir para o Acompanhamento",
        es: "Ir al Seguimiento",
    }
    const action_go_coverage {
        en: "Go to Coverage",
        pt: "Ir para a Cobertura",
        es: "Ir a la Cobertura",
    }
    const action_go_flow {
        en: "Go to Flow",
        pt: "Ir para o Fluxo",
        es: "Ir al Flujo",
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
    const action_comment {
        en: "Comment on selection",
        pt: "Comentar a seleção",
        es: "Comentar la selección",
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
    const inspector_comments {
        en: "Review comments",
        pt: "Comentários da revisão",
        es: "Comentarios de la revisión",
    }
    const inspector_no_comments {
        en: "none yet · press c on a line",
        pt: "nenhum ainda · pressione c em uma linha",
        es: "ninguno aún · pulsa c en una línea",
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
    const action_run_tests {
        en: "Run associated tests (asks first)",
        pt: "Executar testes associados (pergunta antes)",
        es: "Ejecutar pruebas asociadas (pregunta antes)",
    }
    const action_reload_reports {
        en: "Reload test reports",
        pt: "Recarregar relatórios de testes",
        es: "Recargar informes de pruebas",
    }
    const action_reload_flow {
        en: "Reload flow",
        pt: "Recarregar o fluxo",
        es: "Recargar el flujo",
    }
    const action_request_flow {
        en: "Prepare flow request in agent",
        pt: "Preparar pedido de fluxo no agente",
        es: "Preparar petición de flujo en el agente",
    }
    const action_refresh {
        en: "Refresh snapshot",
        pt: "Atualizar o snapshot",
        es: "Actualizar el snapshot",
    }
    const action_finish {
        en: "Finish review…",
        pt: "Concluir a revisão…",
        es: "Terminar la revisión…",
    }
    const action_copy_feedback {
        en: "Copy feedback instruction",
        pt: "Copiar a instrução de feedback",
        es: "Copiar la instrucción de feedback",
    }
    const action_confirm_submitted {
        en: "I submitted the feedback in the agent",
        pt: "Enviei o feedback no agente",
        es: "He enviado el feedback en el agente",
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
    const comment_detached {
        en: "This code is not part of the change; comments attach to changed code only.",
        pt: "Este código não faz parte da mudança; os comentários se prendem apenas ao código alterado.",
        es: "Este código no forma parte del cambio; los comentarios se anclan solo al código modificado.",
    }
    const comment_side_old {
        en: "old",
        pt: "antigo",
        es: "antiguo",
    }
    const comment_side_new {
        en: "new",
        pt: "novo",
        es: "nuevo",
    }
    const comment_place_unit {
        en: "whole unit",
        pt: "unidade inteira",
        es: "unidad completa",
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
    const version_detached {
        en: "This code is not part of the change; there is no other version.",
        pt: "Este código não faz parte da mudança; não há outra versão.",
        es: "Este código no forma parte del cambio; no hay otra versión.",
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
    const run_tests_title {
        en: "Run these tests?",
        pt: "Executar estes testes?",
        es: "¿Ejecutar estas pruebas?",
    }
    const run_tests_source {
        en: "The command comes from your .klaub.toml; Klaub never invents one.",
        pt: "O comando vem do seu .klaub.toml; o Klaub nunca inventa um.",
        es: "El comando viene de tu .klaub.toml; Klaub nunca inventa uno.",
    }
    const run_tests_busy {
        en: "The screen is busy until it finishes or times out.",
        pt: "A tela fica ocupada até terminar ou estourar o tempo.",
        es: "La pantalla queda ocupada hasta que termine o se agote el tiempo.",
    }
    const diagram_request_title {
        en: "Prepare the diagram request in the agent?",
        pt: "Preparar o pedido do diagrama no agente?",
        es: "¿Preparar la petición del diagrama en el agente?",
    }
    const feedback_request_title {
        en: "Prepare the feedback in the agent?",
        pt: "Preparar o feedback no agente?",
        es: "¿Preparar el feedback en el agente?",
    }
    const destination_none_unmanaged {
        en: "none — Klaub is not managing an agent here; use Copy instead",
        pt: "nenhum — o Klaub não está gerenciando um agente aqui; use Copiar",
        es: "ninguno — Klaub no está gestionando un agente aquí; usa Copiar",
    }
    const destination_none_pane {
        en: "none — no agent pane was found; use Copy instead",
        pt: "nenhum — nenhum painel de agente foi encontrado; use Copiar",
        es: "ninguno — no se encontró ningún panel de agente; usa Copiar",
    }
    const paste_text_heading {
        en: "text to paste (Klaub never presses Enter; you decide when to send it):",
        pt: "texto a colar (o Klaub nunca pressiona Enter; você decide quando enviar):",
        es: "texto a pegar (Klaub nunca pulsa Enter; tú decides cuándo enviarlo):",
    }
    const paste_appended {
        en: "If the agent's input already has text, the paste may be appended to it.",
        pt: "Se a entrada do agente já tiver texto, a colagem pode ser anexada a ele.",
        es: "Si la entrada del agente ya tiene texto, lo pegado puede añadirse a él.",
    }
    const clipboard_unavailable {
        en: "No clipboard is available here. Use Show file instead.",
        pt: "Não há área de transferência disponível aqui. Use Mostrar arquivo.",
        es: "No hay portapapeles disponible aquí. Usa Mostrar archivo.",
    }
    const feedback_copied {
        en: "Feedback instruction copied. Paste it into the agent yourself.",
        pt: "Instrução de feedback copiada. Cole-a no agente você mesmo.",
        es: "Instrucción de feedback copiada. Pégala tú mismo en el agente.",
    }
    const recorded_submitted {
        en: "Recorded: you submitted the feedback.",
        pt: "Registrado: você enviou o feedback.",
        es: "Registrado: has enviado el feedback.",
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
    fn reports_reloaded(count: usize) {
        en: "Test reports reloaded ({count} source(s)).",
        pt: "Relatórios de testes recarregados ({count} fonte(s)).",
        es: "Informes de pruebas recargados ({count} fuente(s)).",
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
    fn comment_title(unit: &str, side: &str, place: &str) {
        en: "Comment on {unit} ({side} code, {place})",
        pt: "Comentário em {unit} (código {side}, {place})",
        es: "Comentario en {unit} (código {side}, {place})",
    }
    fn comment_place_line(line: u32) {
        en: "line {line}",
        pt: "linha {line}",
        es: "línea {line}",
    }
    fn comment_place_lines(from: u32, to: u32) {
        en: "lines {from}–{to}",
        pt: "linhas {from}–{to}",
        es: "líneas {from}–{to}",
    }
    fn run_tests_command(command: &str) {
        en: "command: {command}",
        pt: "comando: {command}",
        es: "comando: {command}",
    }
    fn run_tests_cwd(dir: &str) {
        en: "in:      {dir}",
        pt: "em:       {dir}",
        es: "en:       {dir}",
    }
    fn run_tests_limit(seconds: u64) {
        en: "limit:   {seconds} s",
        pt: "limite:   {seconds} s",
        es: "límite:   {seconds} s",
    }
    fn flow_reloaded(state: &str) {
        en: "Flow reloaded: {state}.",
        pt: "Fluxo recarregado: {state}.",
        es: "Flujo recargado: {state}.",
    }
    fn destination_unverified(why: &str) {
        en: "unverified — {why}",
        pt: "não verificado — {why}",
        es: "sin verificar — {why}",
    }
    fn paste_destination(destination: &str) {
        en: "destination: {destination}",
        pt: "destino: {destination}",
        es: "destino: {destination}",
    }
    fn paste_running_there(command: &str) {
        en: "running there: {command}",
        pt: "executando lá: {command}",
        es: "ejecutándose allí: {command}",
    }
    fn paste_shell_warning(command: &str) {
        en: "⚠ {command} is a shell. If your agent is not the one reading the keyboard, the text lands on a shell prompt.",
        pt: "⚠ {command} é um shell. Se o seu agente não é quem lê o teclado, o texto cai em um prompt de shell.",
        es: "⚠ {command} es un shell. Si tu agente no es quien lee el teclado, el texto cae en un prompt de shell.",
    }
    fn paste_harness(name: &str) {
        en: "harness: {name}",
        pt: "harness: {name}",
        es: "harness: {name}",
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
    const tests_timed_out {
        en: "timed out",
        pt: "estourou o tempo",
        es: "agotó el tiempo",
    }
    const tests_cancelled {
        en: "was cancelled",
        pt: "foram cancelados",
        es: "fueron cancelados",
    }
    const tests_passed_exit {
        en: "passed (exit 0)",
        pt: "passaram (saída 0)",
        es: "pasaron (salida 0)",
    }
    const tests_no_exit_code {
        en: "ended without an exit code",
        pt: "terminaram sem código de saída",
        es: "terminaron sin código de salida",
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
    const impact_pivoted {
        en: "Showing the impact of that symbol. Esc goes back.",
        pt: "Mostrando o impacto desse símbolo. Esc volta.",
        es: "Mostrando el impacto de ese símbolo. Esc vuelve.",
    }
    const impact_pick_target {
        en: "Select a caller, callee or entrypoint to pivot on.",
        pt: "Selecione um chamador, chamado ou ponto de entrada para pivotar.",
        es: "Selecciona un llamador, llamado o punto de entrada para pivotar.",
    }
    const comment_code_missing {
        en: "The code this comment was about is not in this snapshot.",
        pt: "O código ao qual este comentário se referia não está neste snapshot.",
        es: "El código al que se refería este comentario no está en este snapshot.",
    }
    const comment_edit_title {
        en: "Edit comment",
        pt: "Editar comentário",
        es: "Editar comentario",
    }
    const comment_edit_only_drafts {
        en: "Only drafts can be edited; this one was already handed over.",
        pt: "Só rascunhos podem ser editados; este já foi entregue.",
        es: "Solo los borradores se pueden editar; este ya se entregó.",
    }
    const comment_delete_title {
        en: "Delete this comment?",
        pt: "Excluir este comentário?",
        es: "¿Eliminar este comentario?",
    }
    const comment_delete_only_drafts {
        en: "Only drafts can be deleted; this one was already handed over.",
        pt: "Só rascunhos podem ser excluídos; este já foi entregue.",
        es: "Solo los borradores se pueden eliminar; este ya se entregó.",
    }
    const intent_title {
        en: "Intent of the change (what the author wanted)",
        pt: "Intenção da mudança (o que o autor queria)",
        es: "Intención del cambio (lo que quería el autor)",
    }
    const criterion_title {
        en: "New acceptance criterion",
        pt: "Novo critério de aceitação",
        es: "Nuevo criterio de aceptación",
    }
    const followup_code_missing {
        en: "That code is not in this snapshot; the comment is stale.",
        pt: "Esse código não está neste snapshot; o comentário está obsoleto.",
        es: "Ese código no está en este snapshot; el comentario está obsoleto.",
    }
    const coverage_showing_pending {
        en: "Showing what is left to review in that module. x clears the filters.",
        pt: "Mostrando o que falta revisar nesse módulo. x limpa os filtros.",
        es: "Mostrando lo que falta por revisar en ese módulo. x limpia los filtros.",
    }
    const flow_step_no_symbol {
        en: "This step does not point to a symbol in the project.",
        pt: "Este passo não aponta para um símbolo do projeto.",
        es: "Este paso no apunta a un símbolo del proyecto.",
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
    const saved_comment {
        en: "Comment saved.",
        pt: "Comentário salvo.",
        es: "Comentario guardado.",
    }
    const saved_comment_updated {
        en: "Comment updated.",
        pt: "Comentário atualizado.",
        es: "Comentario actualizado.",
    }
    const saved_intent {
        en: "Intent saved.",
        pt: "Intenção salva.",
        es: "Intención guardada.",
    }
    const saved_criterion {
        en: "Criterion added.",
        pt: "Critério adicionado.",
        es: "Criterio añadido.",
    }
    const tests_running {
        en: "Running tests… the screen is busy until they finish or time out.",
        pt: "Executando testes… a tela fica ocupada até terminarem ou estourarem o tempo.",
        es: "Ejecutando pruebas… la pantalla queda ocupada hasta que terminen o se agote el tiempo.",
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
    const agent_none_feedback {
        en: "No agent pane is managed here. Use Copy feedback or Show file.",
        pt: "Nenhum painel de agente é gerenciado aqui. Use Copiar feedback ou Mostrar arquivo.",
        es: "Aquí no se gestiona ningún panel de agente. Usa Copiar feedback o Mostrar archivo.",
    }
    const agent_prepared_feedback {
        en: "Prepared in the agent's input. Switch to it (prefix + A), read it, and press Enter yourself.",
        pt: "Preparado na entrada do agente. Vá até ele (prefixo + A), leia e pressione Enter você mesmo.",
        es: "Preparado en la entrada del agente. Ve hasta él (prefijo + A), léelo y pulsa Enter tú mismo.",
    }
    const agent_none_diagram {
        en: "No agent pane is managed here. Copy the request from the Flow screen instead.",
        pt: "Nenhum painel de agente é gerenciado aqui. Copie o pedido pela tela de Fluxo.",
        es: "Aquí no se gestiona ningún panel de agente. Copia la petición desde la pantalla de Flujo.",
    }
    const agent_prepared_diagram {
        en: "Diagram request prepared in the agent. Press Enter there when you want it sent; then U/L reloads the flow.",
        pt: "Pedido de diagrama preparado no agente. Pressione Enter lá quando quiser enviá-lo; depois U/L recarrega o fluxo.",
        es: "Petición de diagrama preparada en el agente. Pulsa Enter allí cuando quieras enviarla; luego U/L recarga el flujo.",
    }
    const comment_deleted {
        en: "Comment deleted.",
        pt: "Comentário excluído.",
        es: "Comentario eliminado.",
    }
    const finished_delivery_again {
        en: "Review finished and saved. F offers the delivery options again.",
        pt: "Revisão concluída e salva. F oferece as opções de entrega de novo.",
        es: "Revisión terminada y guardada. F ofrece de nuevo las opciones de entrega.",
    }
    const finished_saved {
        en: "Review finished and saved.",
        pt: "Revisão concluída e salva.",
        es: "Revisión terminada y guardada.",
    }
}

messages! {
    fn peek_unsupported(key: char, toggle: char) {
        en: "This terminal does not report key releases, so {key} cannot be held. {toggle} toggles old and new.",
        pt: "Este terminal não informa quando as teclas são soltas, então {key} não pode ser mantida pressionada. {toggle} alterna entre antigo e novo.",
        es: "Este terminal no informa cuándo se sueltan las teclas, así que {key} no se puede mantener pulsada. {toggle} alterna entre antiguo y nuevo.",
    }
    fn tests_finished(what: &str, seconds: f32) {
        en: "Tests {what} in {seconds:.1}s. Reports were re-read.",
        pt: "Testes {what} em {seconds:.1}s. Os relatórios foram relidos.",
        es: "Pruebas {what} en {seconds:.1}s. Los informes se releyeron.",
    }
    fn tests_failed_exit(code: i32) {
        en: "failed (exit {code})",
        pt: "falharam (saída {code})",
        es: "fallaron (salida {code})",
    }
    fn module_only(module: &str) {
        en: "Showing module {module} only. x clears the filter.",
        pt: "Mostrando só o módulo {module}. x limpa o filtro.",
        es: "Mostrando solo el módulo {module}. x limpia el filtro.",
    }
    fn followup_decision_title(status: &str) {
        en: "Your decision: {status} — optional note",
        pt: "Sua decisão: {status} — nota opcional",
        es: "Tu decisión: {status} — nota opcional",
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
    fn recorded_decision(status: &str) {
        en: "Recorded: {status}.",
        pt: "Registrado: {status}.",
        es: "Registrado: {status}.",
    }
    fn rule_silenced(rule: &str) {
        en: "Rule {rule} is silenced for this review.",
        pt: "A regra {rule} foi silenciada nesta revisão.",
        es: "La regla {rule} se silenció en esta revisión.",
    }
    fn agent_could_not_prepare_feedback(why: &str) {
        en: "Could not prepare it: {why}. Use Copy feedback or Show file.",
        pt: "Não foi possível preparar: {why}. Use Copiar feedback ou Mostrar arquivo.",
        es: "No se pudo preparar: {why}. Usa Copiar feedback o Mostrar archivo.",
    }
    fn agent_could_not_prepare(why: &str) {
        en: "Could not prepare it: {why}.",
        pt: "Não foi possível preparar: {why}.",
        es: "No se pudo preparar: {why}.",
    }
    fn feedback_file(file: &str, prompt: &str) {
        en: "Feedback file: {file} — tell the agent: {prompt}",
        pt: "Arquivo de feedback: {file} — diga ao agente: {prompt}",
        es: "Archivo de feedback: {file} — dile al agente: {prompt}",
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
