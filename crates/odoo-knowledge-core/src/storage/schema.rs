pub const INITIAL_SCHEMA: &str = include_str!("../../../../migrations/0001_initial.sql");
pub const FTS_SCHEMA: &str = include_str!("../../../../migrations/0002_fts.sql");
pub const LOOKUP_INDEXES_SCHEMA: &str =
    include_str!("../../../../migrations/0003_lookup_indexes.sql");
pub const HOT_PATH_ORDER_INDEXES_SCHEMA: &str =
    include_str!("../../../../migrations/0004_hot_path_order_indexes.sql");
pub const MATERIALIZED_CONTEXT_CACHE_SCHEMA: &str =
    include_str!("../../../../migrations/0005_materialized_context_cache.sql");

pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_initial", INITIAL_SCHEMA),
    ("0002_fts", FTS_SCHEMA),
    ("0003_lookup_indexes", LOOKUP_INDEXES_SCHEMA),
    ("0004_hot_path_order_indexes", HOT_PATH_ORDER_INDEXES_SCHEMA),
    (
        "0005_materialized_context_cache",
        MATERIALIZED_CONTEXT_CACHE_SCHEMA,
    ),
];
