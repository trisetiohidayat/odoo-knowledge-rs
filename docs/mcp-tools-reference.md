# Odoo Knowledge RS MCP Tools Reference

> Scope: Rust endpoint only. This document describes the MCP tools exposed by `odoo-knowledge-rs` and intentionally does not compare with the Python service.
>
> Endpoint: `https://mcp-odoo-rs.trisetio.my.id/mcp`
>
> Protocol: HTTP JSON-RPC MCP.

---

## Table Of Contents

1. [Reference Conventions](#1-reference-conventions)
2. [MCP Lifecycle Calls](#2-mcp-lifecycle-calls)
3. [Common Concepts](#3-common-concepts)
4. [`odoo_search`](#4-odoo_search)
5. [`odoo_impact_analysis`](#5-odoo_impact_analysis)
6. [`odoo_context_bundle`](#6-odoo_context_bundle)
7. [`odoo_trace_business_flow`](#7-odoo_trace_business_flow)
8. [`odoo_find_extension_point`](#8-odoo_find_extension_point)
9. [`odoo_debug_hypotheses`](#9-odoo_debug_hypotheses)
10. [`odoo_compare_symbol`](#10-odoo_compare_symbol)
11. [`odoo_module_context`](#11-odoo_module_context)
12. [`odoo_model_context`](#12-odoo_model_context)
13. [`odoo_method_chain`](#13-odoo_method_chain)
14. [`odoo_field_context`](#14-odoo_field_context)
15. [`odoo_view_chain`](#15-odoo_view_chain)
16. [`odoo_xmlid_lookup`](#16-odoo_xmlid_lookup)
17. [Error Behavior](#17-error-behavior)
18. [Cache And Performance Summary](#18-cache-and-performance-summary)
19. [Static Analysis Notes](#19-static-analysis-notes)

---

## 1. Reference Conventions

### JSON-RPC Envelope

All MCP requests are JSON-RPC requests sent with `POST` and `Content-Type: application/json`.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "odoo_search",
    "arguments": {}
  }
}
```

### Tool Response Envelope

Tool responses are wrapped in MCP content format:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{... JSON string ...}"
      }
    ],
    "isError": false
  }
}
```

The `text` field is a JSON string. MCP clients should parse it as JSON to access the tool payload.

### Required And Optional Fields

- Required fields must be present in `params.arguments`.
- Optional fields may be omitted.
- Most tools accept optional `codebase` to choose an indexed Odoo version such as `odoo-17`, `odoo-18`, or `odoo-19`.
- If `codebase` is omitted, the server uses its default codebase selection logic.

### Additional Properties

All tool input schemas use:

```json
"additionalProperties": false
```

That means clients should not send fields outside the documented schema.

---

## 2. MCP Lifecycle Calls

### `initialize`

Initializes the MCP server connection.

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {}
}
```

Response includes:

- `protocolVersion`
- `capabilities.tools`
- `serverInfo.name`
- `serverInfo.version`

### `tools/list`

Returns all tool schemas.

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}
```

### `tools/call`

Calls one tool.

Request shape:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "odoo_search",
    "arguments": {
      "query": "sale.order"
    }
  }
}
```

### `ping`

Lightweight server check.

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "ping",
  "params": {}
}
```

---

## 3. Common Concepts

### `codebase`

An indexed Odoo source version. Current live examples:

- `odoo-17`
- `odoo-18`
- `odoo-19`

### `module`

An Odoo addon module, for example:

- `sale`
- `point_of_sale`
- `stock`

### `model_name`

An Odoo model technical name, for example:

- `sale.order`
- `product.template`
- `res.partner`

### `field_name`

A field technical name on an Odoo model, for example:

- `available_in_pos`
- `partner_id`
- `state`

### `method_name`

A Python method name on an Odoo model, for example:

- `action_confirm`
- `_compute_amount`
- `write`

### XMLID

A fully qualified XML record identifier in `module.name` form, for example:

```text
point_of_sale.product_template_form_view
```

---

<a id="4-odoo_search"></a>

## 4. `odoo_search`

### Description

Hybrid lexical and metadata search over an indexed Odoo codebase.

### Use This Tool When

- You do not know which specific tool to call yet.
- You want to find a model, method, field, XMLID, view, module, file, or text chunk.
- You want exact-match ranking for structured Odoo names.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["query"],
  "properties": {
    "query": {
      "type": "string",
      "description": "Search query."
    },
    "filters": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "codebase": {
          "type": "string",
          "description": "Optional codebase name."
        },
        "module": {
          "type": "string",
          "description": "Optional module filter."
        },
        "limit": {
          "type": "integer",
          "minimum": 1,
          "maximum": 100,
          "description": "Maximum result count."
        }
      }
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `query` | string | yes | none | Search text or exact structured name. |
| `filters.codebase` | string | no | server default | Indexed Odoo codebase. |
| `filters.module` | string | no | none | Restrict search to one Odoo addon module. |
| `filters.limit` | integer | no | `20` | Maximum result count; schema allows 1-100. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "odoo_search",
    "arguments": {
      "query": "sale.order.action_confirm",
      "filters": {
        "codebase": "odoo-19",
        "limit": 10
      }
    }
  }
}
```

### Output Fields

| Field | Type | Description |
|---|---|---|
| `codebase` | object | Metadata about the selected indexed codebase. |
| `query` | string | Query string used. |
| `results.symbols` | array | Structured symbol matches. |
| `results.chunks` | array | Text chunk matches. |
| `basis` | string | Evidence basis, normally SQLite FTS/search. |
| `confidence` | string | Static confidence label. |

`results.symbols[]` commonly contains:

| Field | Description |
|---|---|
| `kind` | Symbol kind such as `model`, `method`, `field`, `xmlid`, `view`, `module`. |
| `name` | Symbol name. |
| `qualname` | Qualified symbol name, for example `method:sale.order.action_confirm`. |
| `module` | Odoo module where the symbol was found. |
| `file_path` | Relative indexed file path. |
| `rank` | Search ranking score. Lower values are better in SQLite FTS ranking. |

### Example Payload Shape

```json
{
  "codebase": {
    "name": "odoo-19",
    "series": "19.0",
    "version": "19.0"
  },
  "query": "sale.order.action_confirm",
  "results": {
    "symbols": [
      {
        "kind": "method",
        "name": "action_confirm",
        "qualname": "method:sale.order.action_confirm",
        "module": "sale",
        "file_path": "addons/sale/models/sale_order.py"
      }
    ],
    "chunks": []
  },
  "basis": "sqlite_fts5",
  "confidence": "medium"
}
```

### Cache And Performance

- Uses exact-match fast paths for model, method, field, XMLID, view, and module names.
- Uses SQLite FTS5 for lexical symbol and chunk search.
- Uses Odoo-aware ranking.
- Uses in-memory response cache with short TTL for repeated queries.

### Accuracy Notes

Exact structured queries are covered by the accuracy fixture for model, method, field, XMLID, and module top-1 ranking.

### Related Tools

- `odoo_model_context`
- `odoo_method_chain`
- `odoo_field_context`
- `odoo_xmlid_lookup`
- `odoo_module_context`

---

<a id="5-odoo_impact_analysis"></a>

## 5. `odoo_impact_analysis`

### Description

Returns graph edges and symbols related to a target symbol, file, model name, or XMLID.

### Use This Tool When

- You want to understand what may be affected by a symbol.
- You want incoming and outgoing static graph relationships.
- You want symbols related by file locality.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["target"],
  "properties": {
    "target": {
      "type": "string",
      "description": "Symbol qualname/name, file path, model name, or XMLID to inspect."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `target` | string | yes | none | Symbol, model, file path, or XMLID to inspect. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "odoo_impact_analysis",
    "arguments": {
      "target": "method:sale.order.action_confirm",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Type | Description |
|---|---|---|
| `codebase` | object | Codebase metadata. |
| `target` | string | Original target. |
| `normalized_target` | string | Target after static normalization. |
| `matches` | array | Matching symbols. |
| `outgoing_edges` | array | Static graph edges from the target. |
| `incoming_edges` | array | Static graph edges to the target. |
| `related_symbols` | array | Other indexed symbols in related files. |
| `basis` | string | Analysis basis. |
| `confidence` | string | Static confidence. |

### Cache And Performance

- Uses SQLite indexes for symbol and graph lookups.
- Uses in-memory response cache for repeated identical requests.

### Accuracy Notes

This is static impact analysis. It does not prove exact runtime effect in Odoo.

### Related Tools

- `odoo_trace_business_flow`
- `odoo_context_bundle`
- `odoo_search`

---

<a id="6-odoo_context_bundle"></a>

## 6. `odoo_context_bundle`

### Description

Builds a compact context bundle for debugging, implementation, review, or tracing.

### Use This Tool When

- You need a quick context package for an unknown topic.
- You want search results plus related diagnostics and static facts.
- You are starting investigation and do not yet know the exact model/method/field.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["query"],
  "properties": {
    "query": {
      "type": "string",
      "description": "Topic, symbol, model, method, or symptom to bundle context for."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    },
    "module": {
      "type": "string",
      "description": "Optional module filter."
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 50,
      "description": "Maximum search result count."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `query` | string | yes | none | Topic or symptom to investigate. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |
| `module` | string | no | none | Optional module filter. |
| `limit` | integer | no | `10` | Maximum search result count; schema allows 1-50. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "odoo_context_bundle",
    "arguments": {
      "query": "available_in_pos product template",
      "codebase": "odoo-19",
      "module": "point_of_sale",
      "limit": 10
    }
  }
}
```

### Output Fields

Common top-level fields include:

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `query` | Query used to build context. |
| `search` or `results` | Search evidence. |
| `diagnostics_sample` | Relevant diagnostics when available. |
| `basis` | Static basis. |
| `confidence` | Static confidence. |

### Cache And Performance

- Uses search and structured context helpers.
- Uses in-memory response cache for repeated identical requests.

### Accuracy Notes

This is a compact context tool, not a final answer generator. Use exact tools after narrowing the target.

### Related Tools

- `odoo_search`
- `odoo_debug_hypotheses`
- `odoo_find_extension_point`

---

<a id="7-odoo_trace_business_flow"></a>

## 7. `odoo_trace_business_flow`

### Description

Traces an Odoo business entrypoint using method chain and graph edges.

### Use This Tool When

- You want a static trace from a model method.
- You are investigating a business flow such as confirmation, posting, validation, or creation.
- You want related method chain and graph context in one payload.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["model_name", "method_name"],
  "properties": {
    "model_name": {
      "type": "string",
      "description": "Odoo model name, for example sale.order."
    },
    "method_name": {
      "type": "string",
      "description": "Method/entrypoint name, for example action_confirm."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `model_name` | string | yes | none | Odoo model name. |
| `method_name` | string | yes | none | Method entrypoint name. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "odoo_trace_business_flow",
    "arguments": {
      "model_name": "sale.order",
      "method_name": "action_confirm",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

Typical fields include:

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `model` | Model name. |
| `method` | Method name. |
| `method_chain` or `chain` | Static method contributors. |
| `graph` or related edges | Static relationships. |
| `note` | Static-analysis caveat. |
| `basis` | Analysis basis. |
| `confidence` | Static confidence. |

### Cache And Performance

- Uses SQLite lookup indexes and in-memory cache.

### Accuracy Notes

Method order approximates Odoo override behavior. Runtime registry order can differ.

### Related Tools

- `odoo_method_chain`
- `odoo_impact_analysis`
- `odoo_context_bundle`

---

<a id="8-odoo_find_extension_point"></a>

## 8. `odoo_find_extension_point`

### Description

Finds candidate extension points for a development goal.

### Use This Tool When

- You want to customize or extend Odoo behavior.
- You need candidate models, methods, views, or modules to inspect.
- You have a goal but not a known exact symbol.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["goal"],
  "properties": {
    "goal": {
      "type": "string",
      "description": "Development goal or target symbol to find extension points for."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    },
    "module": {
      "type": "string",
      "description": "Optional module filter."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `goal` | string | yes | none | Desired customization or target behavior. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |
| `module` | string | no | none | Optional module scope. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "odoo_find_extension_point",
    "arguments": {
      "goal": "add validation before sale order confirmation",
      "codebase": "odoo-19",
      "module": "sale"
    }
  }
}
```

### Output Fields

Typical fields include:

| Field | Description |
|---|---|
| `goal` | Requested development goal. |
| `search` or `candidates` | Candidate extension symbols. |
| `related_models` | Relevant model facts when inferred. |
| `related_methods` | Relevant method facts when inferred. |
| `basis` | Static evidence basis. |
| `confidence` | Static confidence. |

### Cache And Performance

- Uses search and structured queries.
- Uses in-memory response cache.

### Accuracy Notes

This tool suggests candidates; it does not guarantee the best extension point for runtime behavior.

### Related Tools

- `odoo_search`
- `odoo_model_context`
- `odoo_method_chain`

---

<a id="9-odoo_debug_hypotheses"></a>

## 9. `odoo_debug_hypotheses`

### Description

Builds debugging hypotheses and relevant context for a symptom.

### Use This Tool When

- You have an error message or unexpected behavior.
- You need likely investigation paths.
- You want static facts to guide debugging.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["symptom"],
  "properties": {
    "symptom": {
      "type": "string",
      "description": "Bug symptom, error text, model, method, or behavior to investigate."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    },
    "module": {
      "type": "string",
      "description": "Optional module filter."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `symptom` | string | yes | none | Bug symptom or error text. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |
| `module` | string | no | none | Optional module scope. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "name": "odoo_debug_hypotheses",
    "arguments": {
      "symptom": "sale order confirmation does not create delivery",
      "codebase": "odoo-19",
      "module": "sale"
    }
  }
}
```

### Output Fields

Typical fields include:

| Field | Description |
|---|---|
| `symptom` | Original symptom. |
| `hypotheses` | Static hypotheses and checks. |
| `search` or `context` | Supporting indexed context. |
| `diagnostics_sample` | Related index diagnostics when available. |
| `basis` | Static basis. |
| `confidence` | Confidence label. |

### Cache And Performance

- Uses search and diagnostics queries.
- Uses in-memory response cache.

### Accuracy Notes

Hypotheses are investigation leads, not final root-cause proof.

### Related Tools

- `odoo_context_bundle`
- `odoo_impact_analysis`
- `odoo_trace_business_flow`

---

<a id="10-odoo_compare_symbol"></a>

## 10. `odoo_compare_symbol`

### Description

Compares a symbol across two indexed Odoo codebases.

### Use This Tool When

- You are comparing Odoo versions.
- You want to know whether a symbol exists in both versions.
- You are doing migration analysis.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["symbol", "left_codebase", "right_codebase"],
  "properties": {
    "symbol": {
      "type": "string",
      "description": "Symbol name, qualname, or file path to compare."
    },
    "left_codebase": {
      "type": "string",
      "description": "Left codebase name."
    },
    "right_codebase": {
      "type": "string",
      "description": "Right codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `symbol` | string | yes | none | Symbol name, qualified name, or file path. |
| `left_codebase` | string | yes | none | First codebase. |
| `right_codebase` | string | yes | none | Second codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "tools/call",
  "params": {
    "name": "odoo_compare_symbol",
    "arguments": {
      "symbol": "method:sale.order.action_confirm",
      "left_codebase": "odoo-18",
      "right_codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `symbol` | Requested symbol. |
| `left.codebase` | Left codebase metadata. |
| `left.matches` | Matches in left codebase. |
| `right.codebase` | Right codebase metadata. |
| `right.matches` | Matches in right codebase. |
| `summary.left_count` | Number of left matches. |
| `summary.right_count` | Number of right matches. |
| `summary.status` | Presence summary. |
| `basis` | Static comparison basis. |
| `confidence` | Static confidence. |

### Cache And Performance

- Uses indexed symbol lookup.
- Uses in-memory response cache.

### Accuracy Notes

Comparison is based on indexed static facts, not runtime behavior.

### Related Tools

- `odoo_search`
- `odoo_impact_analysis`

---

<a id="11-odoo_module_context"></a>

## 11. `odoo_module_context`

### Description

Returns manifest, dependencies, models, and views for an Odoo module.

### Use This Tool When

- You need module-level overview.
- You want dependencies and dependents.
- You want models and views declared by a module.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["module_name"],
  "properties": {
    "module_name": {
      "type": "string",
      "description": "Odoo addon module name."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `module_name` | string | yes | none | Odoo addon module name. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "tools/call",
  "params": {
    "name": "odoo_module_context",
    "arguments": {
      "module_name": "point_of_sale",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `module` | Manifest/module record. |
| `depends` | Dependencies declared by module. |
| `dependents` | Modules depending on this module. |
| `models` | Models declared by module. |
| `views` | Views declared by module. |
| `profile` | Index profile label. |
| `basis` | Evidence basis. |
| `confidence` | Confidence label. |

### Cache And Performance

- Uses materialized SQLite context cache when valid.
- Falls back to live SQL query assembly when cache is missing or stale.
- Uses in-memory response cache after request.

### Materialization

Production materialized `odoo-19` module contexts: 647 payloads.

### Related Tools

- `odoo_model_context`
- `odoo_view_chain`
- `odoo_search`

---

<a id="12-odoo_model_context"></a>

## 12. `odoo_model_context`

### Description

Returns model contributors, fields, methods, and views.

### Use This Tool When

- You need a complete static overview of a model.
- You want all modules contributing to a model.
- You want fields, methods, and views for a model.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["model_name"],
  "properties": {
    "model_name": {
      "type": "string",
      "description": "Odoo model name."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `model_name` | string | yes | none | Odoo model technical name. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "method": "tools/call",
  "params": {
    "name": "odoo_model_context",
    "arguments": {
      "model_name": "sale.order",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `model` | Model name. |
| `contributors` | Model class contributors and inheritance facts. |
| `fields` | Field definitions on the model. |
| `methods` | Method definitions on the model. |
| `views` | Related views for the model. |
| `profile` | Index profile label. |
| `basis` | Evidence basis. |
| `confidence` | High when contributors exist, low when none are found. |

### Cache And Performance

- Uses SQLite hot-path indexes.
- Uses in-memory response cache for repeated calls.
- Not materialized by default because large models can produce large payloads.

### Accuracy Notes

Shows static model contributors; runtime Odoo registry may differ.

### Related Tools

- `odoo_field_context`
- `odoo_method_chain`
- `odoo_view_chain`

---

<a id="13-odoo_method_chain"></a>

## 13. `odoo_method_chain`

### Description

Returns the static override chain for an Odoo model method.

### Use This Tool When

- You want to inspect all implementations of a method on a model.
- You want to see which modules override a method.
- You want `super()` and decorator hints.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["model_name", "method_name"],
  "properties": {
    "model_name": {
      "type": "string",
      "description": "Odoo model name."
    },
    "method_name": {
      "type": "string",
      "description": "Method name."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `model_name` | string | yes | none | Odoo model name. |
| `method_name` | string | yes | none | Method name. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "tools/call",
  "params": {
    "name": "odoo_method_chain",
    "arguments": {
      "model_name": "sale.order",
      "method_name": "action_confirm",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `model` | Model name. |
| `method` | Method name. |
| `chain` | Static method implementations. |
| `profile` | Index profile label. |
| `note` | Runtime-order caveat. |
| `basis` | Parser basis. |
| `confidence` | Static confidence. |

`chain[]` commonly contains:

- `module`
- `model_name`
- `class_name`
- `method_name`
- `decorators`
- `calls_super`
- `file_path`
- `line_start`
- `line_end`

### Cache And Performance

- Uses method lookup indexes.
- Uses in-memory response cache.

### Accuracy Notes

The order approximates override order using dependency-based module ordering. It is not exact runtime registry MRO.

### Related Tools

- `odoo_trace_business_flow`
- `odoo_impact_analysis`
- `odoo_model_context`

---

<a id="14-odoo_field_context"></a>

## 14. `odoo_field_context`

### Description

Returns field definitions, origins, and related view usage.

### Use This Tool When

- You need to know where a field is defined.
- You want field type and origin module.
- You want sample views for the field's model.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["model_name", "field_name"],
  "properties": {
    "model_name": {
      "type": "string",
      "description": "Odoo model name."
    },
    "field_name": {
      "type": "string",
      "description": "Field name."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `model_name` | string | yes | none | Odoo model name. |
| `field_name` | string | yes | none | Field technical name. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "tools/call",
  "params": {
    "name": "odoo_field_context",
    "arguments": {
      "model_name": "product.template",
      "field_name": "available_in_pos",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `model` | Model name. |
| `field` | Field name. |
| `definitions` | Field definition rows. |
| `related_views_sample` | Sample related views for the model. |
| `profile` | Index profile label. |
| `basis` | Parser basis. |
| `confidence` | Medium when definitions exist, low when none are found. |

### Cache And Performance

- Uses field lookup indexes.
- Uses in-memory response cache.

### Accuracy Notes

Field data comes from static Python parsing.

### Related Tools

- `odoo_model_context`
- `odoo_view_chain`
- `odoo_search`

---

<a id="15-odoo_view_chain"></a>

## 15. `odoo_view_chain`

### Description

Returns view records by XMLID or model and inheritance links.

### Use This Tool When

- You want view inheritance context.
- You want all views for a model.
- You want records inheriting a specific XMLID.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["xmlid_or_model"],
  "properties": {
    "xmlid_or_model": {
      "type": "string",
      "description": "View XMLID or target model name."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `xmlid_or_model` | string | yes | none | View XMLID or model name. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example: XMLID

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "tools/call",
  "params": {
    "name": "odoo_view_chain",
    "arguments": {
      "xmlid_or_model": "point_of_sale.product_template_form_view",
      "codebase": "odoo-19"
    }
  }
}
```

### Request Example: Model

```json
{
  "jsonrpc": "2.0",
  "id": 13,
  "method": "tools/call",
  "params": {
    "name": "odoo_view_chain",
    "arguments": {
      "xmlid_or_model": "product.template",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `query` | Original XMLID or model query. |
| `views` | Matching view records. |
| `profile` | Index profile label. |
| `basis` | XML parse basis. |
| `confidence` | High when views exist, low when none are found. |

### Cache And Performance

- Uses view lookup indexes.
- Uses in-memory response cache.

### Accuracy Notes

View chain is based on XML source parsing. Runtime view resolution can depend on installed modules and priorities.

### Related Tools

- `odoo_xmlid_lookup`
- `odoo_model_context`
- `odoo_field_context`

---

<a id="16-odoo_xmlid_lookup"></a>

## 16. `odoo_xmlid_lookup`

### Description

Looks up XMLID records, views, actions, and menus.

### Use This Tool When

- You have an exact XMLID.
- You want to know whether it is a record, view, action, or menu.
- You want file and line provenance for an XMLID.

### Input Schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["xmlid"],
  "properties": {
    "xmlid": {
      "type": "string",
      "description": "Fully qualified XMLID."
    },
    "codebase": {
      "type": "string",
      "description": "Optional codebase name."
    }
  }
}
```

### Arguments

| Field | Type | Required | Default | Description |
|---|---|---:|---|---|
| `xmlid` | string | yes | none | Fully qualified XMLID in `module.name` form. |
| `codebase` | string | no | server default | Indexed Odoo codebase. |

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 14,
  "method": "tools/call",
  "params": {
    "name": "odoo_xmlid_lookup",
    "arguments": {
      "xmlid": "point_of_sale.product_template_form_view",
      "codebase": "odoo-19"
    }
  }
}
```

### Output Fields

| Field | Description |
|---|---|
| `codebase` | Codebase metadata. |
| `xmlid` | Requested XMLID. |
| `records` | Matching XML records. |
| `views` | Matching view records. |
| `actions` | Matching action records. |
| `menus` | Matching menu records. |
| `basis` | XML parse basis. |
| `confidence` | High when any match exists, low when none are found. |

### Cache And Performance

- Uses materialized SQLite context cache when valid.
- Falls back to live queries if cache is absent or stale.
- Uses in-memory response cache after request.
- Production materialized `odoo-19` XMLID payloads: 29,767.

### Accuracy Notes

The lookup is exact by XMLID. It does not evaluate runtime module installation state.

### Related Tools

- `odoo_view_chain`
- `odoo_search`
- `odoo_impact_analysis`

---

## 17. Error Behavior

### Missing Required Arguments

If a required argument is absent, the server returns an MCP tool error payload with `isError: true` and an error message.

Example missing `query` for `odoo_search`:

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"error\":\"invalid config: missing required argument: query\"}"
    }
  ],
  "isError": true
}
```

### Unknown Tool

Unknown tools return a JSON payload containing an error and available tool names.

### Unknown JSON-RPC Method

Unknown JSON-RPC methods return JSON-RPC error code `-32601`.

### Request Timeout

If a tool exceeds configured request timeout, the server returns JSON-RPC error code `-32000` with message `request timeout`.

### Empty Results

Most exact context tools do not treat no results as transport errors. They return a normal payload with empty arrays and lower confidence.

---

## 18. Cache And Performance Summary

| Tool | In-Memory Cache | Materialized Cache | Notes |
|---|---:|---:|---|
| `odoo_search` | yes | no | Search space is open-ended; uses short TTL. |
| `odoo_impact_analysis` | yes | no | Graph/symbol lookup. |
| `odoo_context_bundle` | yes | no | Query-driven bundle. |
| `odoo_trace_business_flow` | yes | no | Uses method chain and graph context. |
| `odoo_find_extension_point` | yes | no | Goal-driven search. |
| `odoo_debug_hypotheses` | yes | no | Symptom-driven search. |
| `odoo_compare_symbol` | yes | no | Cross-codebase lookup. |
| `odoo_module_context` | yes | yes | Materialized by module name. |
| `odoo_model_context` | yes | no | Potentially large payload. |
| `odoo_method_chain` | yes | no | Indexed method lookup. |
| `odoo_field_context` | yes | no | Indexed field lookup. |
| `odoo_view_chain` | yes | no | Indexed view lookup. |
| `odoo_xmlid_lookup` | yes | yes | Materialized by XMLID. |

---

## 19. Static Analysis Notes

All tools operate on indexed source-code facts. They do not run Odoo.

This means:

- Results can explain where code is defined.
- Results can reveal static relationships.
- Results can approximate override chains.
- Results can compare indexed versions.

But results cannot guarantee:

- exact runtime registry order,
- installed module state,
- context-dependent behavior,
- monkey patch effects,
- dynamic Python behavior not visible to the parser.

Use MCP output as source-code intelligence and investigation guidance, not as a replacement for runtime verification when runtime correctness matters.
