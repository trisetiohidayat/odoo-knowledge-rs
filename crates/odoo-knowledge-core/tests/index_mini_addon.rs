use std::path::{Path, PathBuf};

use odoo_knowledge_core::codebase::add_codebase;
use odoo_knowledge_core::indexer::index_codebase;
use odoo_knowledge_core::storage::open_database;
use rusqlite::Connection;

#[test]
fn indexes_full_mini_addon_fixture() {
    let workspace = workspace_root();
    let db_path = temp_db_path();
    let con = open_database(&db_path).unwrap();
    let fixture_root = workspace.join("tests/fixtures");

    add_codebase(&con, "fixtures", &fixture_root).unwrap();
    let stats = index_codebase(&con, "fixtures").unwrap();

    assert_eq!(stats.modules, 1);
    assert_eq!(stats.files, 5);
    assert_eq!(stats.models, 1);
    assert_eq!(stats.fields, 1);
    assert_eq!(stats.methods, 2);
    assert_eq!(stats.xml_records, 1);
    assert_eq!(stats.views, 1);
    assert_eq!(stats.frontend, 6);

    assert_count(&con, "modules", "name='mini_sale'", 1);
    assert_count(
        &con,
        "module_dependencies",
        "module='mini_sale' AND depends_on='sale'",
        1,
    );
    assert_count(
        &con,
        "models",
        "module='mini_sale' AND model_name='sale.order' AND class_name='SaleOrder'",
        1,
    );
    assert_count(
        &con,
        "fields",
        "module='mini_sale' AND model_name='sale.order' AND field_name='x_reference' AND field_type='Char' AND compute='_compute_x_reference'",
        1,
    );
    assert_count(
        &con,
        "methods",
        "module='mini_sale' AND model_name='sale.order' AND method_name='action_confirm' AND calls_super=1",
        1,
    );
    assert_count(
        &con,
        "xml_records",
        "module='mini_sale' AND xmlid='mini_sale.mini_sale_order_form' AND record_model='ir.ui.view'",
        1,
    );
    assert_count(
        &con,
        "views",
        "module='mini_sale' AND xmlid='mini_sale.mini_sale_order_form' AND view_model='sale.order' AND inherit_id='sale.view_order_form' AND xpath_count=1",
        1,
    );
    assert_count(
        &con,
        "security_rules",
        "module='mini_sale' AND name='access.mini.sale.order' AND model_ref='model_sale_order' AND group_ref='base.group_user'",
        1,
    );
    assert_count(
        &con,
        "frontend_symbols",
        "module='mini_sale' AND kind='js_patch' AND name='patch:PaymentScreen.prototype' AND target='PaymentScreen.prototype'",
        1,
    );
    assert_count(
        &con,
        "frontend_symbols",
        "module='mini_sale' AND kind='js_import' AND target='@web/core/utils/patch'",
        1,
    );
    assert_count(
        &con,
        "frontend_symbols",
        "module='mini_sale' AND kind='js_registry' AND name='CustomPaymentScreen' AND category='pos_screens'",
        1,
    );
    assert_count(
        &con,
        "symbols",
        "module='mini_sale' AND kind='js_class' AND name='CustomPaymentScreen' AND basis='tree_sitter_javascript_parse'",
        1,
    );
}

fn assert_count(con: &Connection, table: &str, where_clause: &str, expected: i64) {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {where_clause}");
    let actual: i64 = con.query_row(&sql, [], |row| row.get(0)).unwrap();
    assert_eq!(
        actual, expected,
        "unexpected count for {table}: {where_clause}"
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn temp_db_path() -> PathBuf {
    let unique = format!(
        "odoo-knowledge-rs-mini-addon-{}-{}.db",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    );
    let path = std::env::temp_dir().join(unique);
    let _ = std::fs::remove_file(&path);
    path
}
