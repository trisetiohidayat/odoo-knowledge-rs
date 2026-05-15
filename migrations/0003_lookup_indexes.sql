CREATE INDEX IF NOT EXISTS idx_symbols_codebase_name
    ON symbols(codebase_id, name);

CREATE INDEX IF NOT EXISTS idx_symbols_codebase_qualname
    ON symbols(codebase_id, qualname);

CREATE INDEX IF NOT EXISTS idx_symbols_codebase_module_name
    ON symbols(codebase_id, module, name);

CREATE INDEX IF NOT EXISTS idx_models_codebase_model
    ON models(codebase_id, model_name);

CREATE INDEX IF NOT EXISTS idx_models_codebase_module_model
    ON models(codebase_id, module, model_name);

CREATE INDEX IF NOT EXISTS idx_fields_codebase_model_field
    ON fields(codebase_id, model_name, field_name);

CREATE INDEX IF NOT EXISTS idx_methods_codebase_model_method
    ON methods(codebase_id, model_name, method_name);

CREATE INDEX IF NOT EXISTS idx_xml_records_codebase_xmlid
    ON xml_records(codebase_id, xmlid);

CREATE INDEX IF NOT EXISTS idx_views_codebase_xmlid
    ON views(codebase_id, xmlid);

CREATE INDEX IF NOT EXISTS idx_views_codebase_model
    ON views(codebase_id, view_model);

CREATE INDEX IF NOT EXISTS idx_modules_codebase_name
    ON modules(codebase_id, name);
