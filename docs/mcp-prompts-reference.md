# MCP Prompts Reference

This document describes the prompts exposed by the Rust Odoo Knowledge MCP server.

## Purpose

MCP prompts are reusable instruction templates that a client can discover with `prompts/list` and retrieve with `prompts/get`.

In this server, prompts are used to guide AI agents so they choose the correct Odoo codebase and follow the recommended tool sequence.

## Capability

The MCP `initialize` response includes:

```json
{
  "capabilities": {
    "tools": {},
    "prompts": {
      "listChanged": false
    }
  }
}
```

`listChanged: false` means the prompt list is static for the running server version.

## List Prompts

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "prompts/list",
  "params": {}
}
```

Response shape:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "prompts": []
  }
}
```

## `odoo-codebase-selection`

Guides an AI agent to choose the correct indexed Odoo CE/core codebase instead of a local project/addons directory name.

Arguments:

- `odoo_version`: optional version context, for example `17.0`, `Odoo 17 CE`, or `18`.
- `local_project`: optional local project/addons directory name, used only as context.

Example request:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "prompts/get",
  "params": {
    "name": "odoo-codebase-selection",
    "arguments": {
      "odoo_version": "Odoo 17 CE",
      "local_project": "suqma-local"
    }
  }
}
```

Expected guidance:

- Use `odoo-17` for Odoo CE 17.
- Use `odoo-18` for Odoo CE 18.
- Use `odoo-19` for Odoo CE 19.
- Do not use the local project name as `codebase` unless that exact name is indexed.

## `odoo-investigate-symbol`

Guides an AI agent through the recommended MCP tool sequence for investigating a model, method, field, XMLID, view, module, or symptom.

Arguments:

- `symbol`: required model, method, field, XMLID, module, file path, or symptom to investigate.
- `codebase`: optional indexed Odoo source codebase, for example `odoo-17`.
- `module`: optional addon module filter, for example `sale`, `stock`, or `account`.

Example request:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "prompts/get",
  "params": {
    "name": "odoo-investigate-symbol",
    "arguments": {
      "symbol": "sale.order action_confirm",
      "codebase": "odoo-17",
      "module": "sale"
    }
  }
}
```

Expected guidance:

1. Start with `odoo_search` using `filters.codebase`.
2. Use `odoo_model_context` for models.
3. Use `odoo_method_chain` for model methods.
4. Use `odoo_field_context` for fields.
5. Use `odoo_view_chain` for views.
6. Use `odoo_xmlid_lookup` for XMLIDs.
7. Use `odoo_module_context` for addons.
8. Use `odoo_impact_analysis` or `odoo_context_bundle` for broader debugging.

## Error Behavior

Unknown prompt names return JSON-RPC `-32602` invalid params.

Missing required prompt arguments return JSON-RPC `-32602` invalid params.
