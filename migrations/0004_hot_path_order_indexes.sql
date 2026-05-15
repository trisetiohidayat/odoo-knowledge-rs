CREATE INDEX IF NOT EXISTS idx_models_codebase_model_module_path
    ON models(codebase_id, model_name, module, file_path, line_start);

CREATE INDEX IF NOT EXISTS idx_fields_codebase_model_field_module_path
    ON fields(codebase_id, model_name, field_name, module, file_path, line_start);

CREATE INDEX IF NOT EXISTS idx_methods_codebase_model_method_module_path
    ON methods(codebase_id, model_name, method_name, module, file_path, line_start);

CREATE INDEX IF NOT EXISTS idx_views_codebase_xmlid_module
    ON views(codebase_id, xmlid, module);

CREATE INDEX IF NOT EXISTS idx_views_codebase_model_module_xmlid
    ON views(codebase_id, view_model, module, xmlid);
