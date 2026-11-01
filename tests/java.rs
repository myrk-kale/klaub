//! Java extraction.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_raw_string_hashes
)]

use klaub::adapters::parse::TreeSitterParser;
use klaub::domain::graph::{EdgeOrigin, EntrypointKind};
use klaub::domain::language::Language;
use klaub::domain::ports::SourceParser;
use klaub::domain::symbol::{
    HighlightClass, MarkerKind, ParsedFile, Receiver, Symbol, SymbolKind, Visibility,
};

fn parse(source: &str) -> ParsedFile {
    TreeSitterParser::new()
        .parse(Language::Java, source)
        .unwrap()
}

fn symbol<'a>(file: &'a ParsedFile, qualified: &str) -> &'a Symbol {
    file.symbols
        .iter()
        .find(|s| s.qualified_name == qualified)
        .unwrap_or_else(|| {
            panic!(
                "{qualified} not found in {:?}",
                file.symbols
                    .iter()
                    .map(|s| &s.qualified_name)
                    .collect::<Vec<_>>()
            )
        })
}

const SHOP: &str = r#"
package com.shop.cart;

import java.util.List;
import java.util.Map;
import com.shop.db.Database;
import com.shop.util.*;
import static com.shop.util.Strings.slug;

/** A cart. */
@Service
public class Cart extends Base implements Priced {
    private static final String API_KEY = "h7Q2m9Zc41xPw8Lr";
    private final Database db;

    public Cart(Database db) {
        this.db = db;
    }

    @Override
    public int total(List<Item> items, int discount) throws PricingException {
        int sum = 0;
        for (Item item : items) {
            if (item.price() > 0 && discount < 100) {
                sum += item.price();
                db.query("SELECT * FROM items WHERE id = " + item.id());
            }
        }
        if (sum < 0) {
            throw new PricingException("negative");
        }
        return sum;
    }

    protected <T extends Comparable<T>> Map<String, List<T>> group(List<T> values, T... extra) {
        try {
            return helper(values);
        } catch (Exception e) {
        }
        return null;
    }

    private static void helper() {}

    static class Line {
        void draw() {}
    }

    interface Priced {
        int price();
    }

    enum Status { OPEN, CLOSED }

    record Point(int x, int y) {}
}
"#;

#[test]
fn java_symbols_kinds_and_visibility() {
    let file = parse(SHOP);
    assert!(!file.has_syntax_errors);
    assert_eq!(symbol(&file, "Cart").kind, SymbolKind::Class);
    assert_eq!(symbol(&file, "Cart.Cart").kind, SymbolKind::Method);
    assert_eq!(symbol(&file, "Cart.total").visibility, Visibility::Public);
    assert_eq!(
        symbol(&file, "Cart.group").visibility,
        Visibility::Protected
    );
    assert_eq!(symbol(&file, "Cart.helper").visibility, Visibility::Private);
    assert_eq!(symbol(&file, "Cart.Line").kind, SymbolKind::Class);
    assert_eq!(
        symbol(&file, "Cart.Line").visibility,
        Visibility::Internal,
        "no modifier means package-private"
    );
    assert_eq!(symbol(&file, "Cart.Line.draw").kind, SymbolKind::Method);
    assert_eq!(symbol(&file, "Cart.Priced").kind, SymbolKind::Interface);
    assert_eq!(
        symbol(&file, "Cart.Priced.price").visibility,
        Visibility::Public,
        "interface members are public"
    );
    assert_eq!(symbol(&file, "Cart.Status").kind, SymbolKind::Enum);
    assert_eq!(symbol(&file, "Cart.Point").kind, SymbolKind::Struct);
}

#[test]
fn java_signatures_drop_annotations_and_keep_throws() {
    let file = parse(SHOP);
    let total = symbol(&file, "Cart.total");
    assert_eq!(
        total.signature,
        "public int total(List<Item> items, int discount) throws PricingException"
    );
    assert_eq!(total.decorators, vec!["Override"]);
    assert_eq!(total.features.param_count, 2);
    let group = symbol(&file, "Cart.group");
    assert_eq!(
        group.signature,
        "protected <T extends Comparable<T>> Map<String, List<T>> group(List<T> values, T... extra)"
    );
    assert_eq!(
        group.features.param_count, 2,
        "a varargs parameter counts once"
    );
    assert_eq!(symbol(&file, "Cart").decorators, vec!["Service"]);
    assert!(
        symbol(&file, "Cart")
            .signature
            .starts_with("public class Cart extends Base")
    );
}

#[test]
fn java_complexity_counts_branches() {
    let file = parse(SHOP);
    // 1 + for + if + && + if
    assert_eq!(symbol(&file, "Cart.total").features.complexity, 5);
    assert_eq!(
        symbol(&file, "Cart.total").features.raises,
        vec!["PricingException"],
        "only `throw new X` names an exception"
    );
}

#[test]
fn java_calls_receivers_and_loops() {
    let file = parse(SHOP);
    let index = file.find("Cart.total").unwrap();
    let query = file
        .calls
        .iter()
        .find(|c| c.caller == index && c.name == "query")
        .unwrap();
    assert!(query.in_loop);
    assert_eq!(query.receiver, Receiver::Path(vec!["db".into()]));
    let price = file
        .calls
        .iter()
        .find(|c| c.caller == index && c.name == "price")
        .unwrap();
    assert_eq!(price.receiver, Receiver::Path(vec!["item".into()]));

    let group = file.find("Cart.group").unwrap();
    let helper = file
        .calls
        .iter()
        .find(|c| c.caller == group && c.name == "helper")
        .unwrap();
    assert_eq!(helper.receiver, Receiver::None);
}

#[test]
fn java_imports_name_classes_packages_and_static_members() {
    let file = parse(SHOP);
    let find = |module: &str| {
        file.imports
            .iter()
            .find(|i| i.module == module)
            .unwrap_or_else(|| panic!("no import of {module}: {:?}", file.imports))
    };
    assert!(!find("com.shop.db.Database").wildcard);
    assert!(find("com.shop.util").wildcard);
    let slug = find("com.shop.util.Strings");
    assert_eq!(slug.names[0].name, "slug");
}

#[test]
fn java_markers_are_what_the_syntax_shows() {
    let file = parse(SHOP);
    let markers = |name: &str| {
        symbol(&file, name)
            .features
            .markers
            .iter()
            .map(|m| m.kind)
            .collect::<Vec<_>>()
    };
    assert!(markers("Cart.total").contains(&MarkerKind::RawSqlConcat));
    assert!(markers("Cart.group").contains(&MarkerKind::SwallowedError));
    assert!(markers("Cart.total").contains(&MarkerKind::CallInLoop));
    assert!(
        markers("Cart").contains(&MarkerKind::SecretLiteral),
        "{:?}",
        markers("Cart")
    );
}

#[test]
fn java_shell_and_safe_queries() {
    let file = parse(
        r#"
class Tools {
    void run(Statement stmt, String name) throws Exception {
        new ProcessBuilder("bash", "-c", name).start();
        Runtime.getRuntime().exec("sh -c " + name);
        stmt.executeQuery("SELECT 1");
        stmt.executeQuery(String.format("SELECT * FROM t WHERE n = '%s'", name));
    }
}
"#,
    );
    let kinds: Vec<_> = symbol(&file, "Tools.run")
        .features
        .markers
        .iter()
        .map(|m| m.kind)
        .collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == MarkerKind::ShellExecution)
            .count(),
        2,
        "ProcessBuilder and Runtime.exec each start a shell: {kinds:?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == MarkerKind::RawSqlConcat)
            .count(),
        1,
        "the literal query is fine, the formatted one is not: {kinds:?}"
    );
}

#[test]
fn java_tests_are_recognised_by_annotation() {
    let file = parse(
        r#"
class CartTest {
    @Test
    void totals() {}

    @ParameterizedTest
    void many(int n) {}

    void helper() {}
}
"#,
    );
    assert!(symbol(&file, "CartTest.totals").features.is_test);
    assert!(symbol(&file, "CartTest.many").features.is_test);
    assert!(!symbol(&file, "CartTest.helper").features.is_test);
}

#[test]
fn java_entrypoints_main_spring_and_jaxrs() {
    let file = parse(
        r#"
@RestController
@RequestMapping("/api")
class UserController {
    @GetMapping("/users")
    public List<User> list() { return null; }

    @RequestMapping(value = "/users", method = RequestMethod.POST)
    public void create() {}

    public static void main(String[] args) {}
}

@Path("/orders")
class OrderResource {
    @GET
    @Path("{id}")
    public Order get() { return null; }
}
"#,
    );
    let http = |method: &str, route: &str| {
        file.entrypoints
            .iter()
            .find(|e| matches!(&e.kind, EntrypointKind::Http { method: m, route: r } if m == method && r == route))
            .unwrap_or_else(|| panic!("no {method} {route}: {:?}", file.entrypoints))
    };
    let list = http("GET", "/api/users");
    assert_eq!(file.symbols[list.symbol.unwrap()].name, "list");
    assert_eq!(list.origin, EdgeOrigin::Heuristic);
    http("POST", "/api/users");
    http("GET", "/orders/{id}");
    let main = file
        .entrypoints
        .iter()
        .find(|e| e.kind == EntrypointKind::Main)
        .unwrap();
    assert_eq!(file.symbols[main.symbol.unwrap()].name, "main");
}

#[test]
fn a_java_file_with_a_syntax_error_still_yields_its_symbols() {
    let file = parse("class A {\n    void ok() {}\n    void broken( {\n}\n");
    assert!(file.has_syntax_errors);
    assert!(file.symbols.iter().any(|s| s.qualified_name == "A.ok"));
}

#[test]
fn java_highlighting_marks_keywords_strings_types_and_comments() {
    let source = "// hi\nclass A {\n    int f() { return 42; }\n    String s = \"x\";\n}\n";
    let spans = TreeSitterParser::new()
        .highlight(Language::Java, source)
        .unwrap();
    let class_at = |needle: &str| {
        let start = source.find(needle).unwrap();
        let span = spans
            .iter()
            .find(|s| s.start == start)
            .unwrap_or_else(|| panic!("no span at {needle}: {spans:?}"));
        span.class
    };
    assert_eq!(class_at("// hi"), HighlightClass::Comment);
    assert_eq!(class_at("class"), HighlightClass::Keyword);
    assert_eq!(class_at("return"), HighlightClass::Keyword);
    assert_eq!(class_at("42"), HighlightClass::Number);
    assert_eq!(class_at("\"x\""), HighlightClass::String);
    assert_eq!(class_at("int f"), HighlightClass::Type);
}
