; highlights.scm — unified Go+gsx grammar.
;
; Go is NATIVE in this grammar (not injected), so this query colors both Go
; tokens and gsx-specific nodes in one self-contained file. The Go section is
; adapted from tree-sitter-go's own highlights.scm; the gsx section follows
; and, because later patterns win in Neovim's highlighter, overrides the
; generic Go captures where a node plays a gsx-specific role (e.g. an element
; name is an `identifier` that should read as a tag, not a variable).

; ─────────────────────────────────────────────────────────────────────────
; Go (base) — adapted from tree-sitter-go/queries/highlights.scm
; ─────────────────────────────────────────────────────────────────────────

; Identifiers (generic fallbacks — MUST precede the specific patterns below).
;
; Ordering is load-bearing and differs from tree-sitter-go's own file, which
; these were adapted from: that file targets first-match-wins highlighters
; (the Rust tree-sitter-highlight crate, Helix), where generic patterns are
; written last and lose. This grammar's query targets last-match-wins
; (Neovim, go-tree-sitter-highlight) — the same convention the gsx section
; below relies on to override Go captures. Left in upstream order, these
; three would clobber every specific capture that follows, rendering every
; Go function name and call as a plain variable.
(type_identifier) @type
(field_identifier) @property
(identifier) @variable

; Function calls
(call_expression
  function: (identifier) @function)
(call_expression
  function: (identifier) @function.builtin
  (#match? @function.builtin "^(append|cap|close|complex|copy|delete|imag|len|make|new|panic|print|println|real|recover)$"))
(call_expression
  function: (selector_expression
    field: (field_identifier) @function.method))

; Function definitions
(function_declaration
  name: (identifier) @function)
(method_declaration
  name: (field_identifier) @function.method)

; Operators
[
  "--" "-" "-=" ":=" "!" "!=" "..." "*" "*=" "/" "/=" "&" "&&" "&="
  "%" "%=" "^" "^=" "+" "++" "+=" "<-" "<" "<<" "<<=" "<=" "=" "=="
  ">" ">=" ">>" ">>=" "|" "|=" "||" "~"
] @operator

; Keywords
[
  "break" "case" "chan" "const" "continue" "default" "defer" "else"
  "fallthrough" "for" "func" "go" "goto" "if" "import" "interface" "map"
  "package" "range" "return" "select" "struct" "switch" "type" "var"
] @keyword

; Literals
[
  (interpreted_string_literal)
  (raw_string_literal)
  (rune_literal)
] @string
(escape_sequence) @string.escape
[
  (int_literal)
  (float_literal)
  (imaginary_literal)
] @number
[
  (true)
  (false)
  (nil)
  (iota)
] @constant.builtin
(comment) @comment

; ─────────────────────────────────────────────────────────────────────────
; gsx (overrides + additions)
; ─────────────────────────────────────────────────────────────────────────

; `component` declaration — the keyword, and the name reads like a function def.
(component_declaration
  name: (identifier) @function)
"component" @keyword

; Element tags: <div>…</div>, <Icon/>, <ui.Button/>, <el-dialog>. The name is a
; single `tag_name` token (plain, dotted, or hyphenated) — color it as a tag.
(element name: (tag_name) @tag)
(element open_name: (tag_name) @tag)
(element close_name: (tag_name) @tag)

; Raw-text elements <script>/<style>: tag name + raw body.
(raw_element open_name: (tag_name) @tag)
(raw_element close_name: (tag_name) @tag)
(raw_text) @none

; Processing instructions: <?marker name=…> / <?start name=…>…<?end>. The
; target (marker/start/end) reads as @keyword.directive rather than @tag —
; same treatment as (doctype) @keyword.directive below: a PI is directive-like
; markup gsx never renders as an element, not a real tag, and this keeps it
; visually distinct from both @tag (real elements) and @keyword.conditional
; (control-flow). One pattern per node covers both pi_target children of a
; marker_region (the opening `start` and the closing `end`) since neither is
; a named field. The leading `<?` opener is a composite token like fragment's
; `<>`/`</>` just below — not individually queryable — so it stays uncolored;
; the closing `>` already reads as @tag.delimiter via the generic rule below.
(marker (pi_target) @keyword.directive)
(marker_region (pi_target) @keyword.directive)

; Markup tag punctuation. (Fragment delimiters <>/</> are single composite
; tokens, not queryable as string literals, so they're left uncolored.)
[
  "<"
  ">"
  "/>"
  "</"
] @tag.delimiter

; Attributes.
(attribute_name) @attribute
(bool_attribute (attribute_name) @attribute)

; Plain double-quoted attribute value (may span multiple lines).
(attribute_value_string) @string

; `{ … }` hole / control-flow / composable braces read as special punctuation.
[
  "{"
  "}"
] @punctuation.special

; `{{ … }}` Go statement block delimiters.
[
  "{{"
  "}}"
] @punctuation.special

; Control-flow / value-form keyword nodes (aliased to `keyword` in the grammar:
; if/for/switch/else/case/default inside control_flow, conditional_attribute,
; switch_attribute,
; composable class value-forms, etc.).
(keyword) @keyword.conditional

; Composable class/style condition guard (`"cls": cond`) — the guard colon.
(class_part cond: (_) @variable)

; Embedded f`…` / js`…` / css`…` literals: the prefix+delimiter opener, the raw
; body text, and the @{ … } interpolation hole markers.
(embedded_open) @keyword
[
  (embedded_text)
  (embedded_text_dq)
] @string.special
"@{" @punctuation.special

; Comments in markup.
(html_comment) @comment
(content_comment) @comment
(line_comment) @comment
(doctype) @keyword.directive

; Markup text content (between tags) — plain literal text, left uncolored so it
; reads as prose rather than code.
(text) @none
