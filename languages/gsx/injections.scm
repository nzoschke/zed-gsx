; injections.scm — unified Go+gsx grammar.
;
; Go itself is NATIVE in this grammar (not injected), so there is no Go
; injection. These injections color the SUBLANGUAGE bodies:
;   • <script> raw-text  → javascript
;   • <style>  raw-text  → css
;   • js`…` literal body  → javascript
;   • css`…` literal body → css
; (Interpolation holes inside them — @{ … } / { … } — are parsed by the gsx
; grammar itself and are not part of the injected content ranges.)

; <script> … </script> raw body → JavaScript
((raw_element
   (tag_name) @_tag
   (raw_text) @injection.content)
 (#match? @_tag "^[Ss][Cc][Rr][Ii][Pp][Tt]$")
 (#set! injection.language "javascript")
 (#set! injection.combined))

; <style> … </style> raw body → CSS
((raw_element
   (tag_name) @_tag
   (raw_text) @injection.content)
 (#match? @_tag "^[Ss][Tt][Yy][Ll][Ee]$")
 (#set! injection.language "css")
 (#set! injection.combined))

; js`…` literal → JavaScript, INCLUDING its @{ } holes.
;
; The naive form — capture each (embedded_text) run — is what tree-sitter calls
; redaction, and it is the default the ecosystem lives with. It does not work
; here: a literal with holes is cut into runs that are each syntactically
; incomplete, so
;
;   @change=js`… htmx.ajax('POST', @{url}, {target: '#x'}); …`
;
; became four separate documents, every one of them a JS parse error. This is
; the same unresolved upstream problem as css-in-js interpolation
; (nvim-treesitter#4542, open since 2023).
;
; Two things make it work anyway:
;
;   1. A quantified ALTERNATION over consecutive siblings, anchored to the
;      opener so it can only start at the literal's first child —
;      `[(embedded_text) (at_hole …)]+` — produces ONE match per literal
;      carrying every range, so the runs form a single document. (A plain
;      `(embedded_text)+` cannot: the holes between runs break the
;      consecutive-sibling requirement, so it keeps only the first run.)
;
;   2. Capturing the hole's INNER expression rather than the whole `@{ … }`
;      node. `@{`/`}` are not JS tokens, but the listed Go expression forms all
;      spell something lexically valid in JS, so `go(@{u}, 1)` reaches the JS
;      parser as `go(u, 1)`. That coincidence is what the css-in-js case lacks
;      (there the hole holds JS being spliced into CSS).
;
;      The list matters: with only (identifier), a hole like
;      `@{textToString(props.X)}` did not match, the `+` run broke at it, and
;      the literal was truncated mid-object — one ERROR per Alpine x-data
;      block. A Go form NOT in the list (a composite literal, `T{}`) still
;      breaks the run, so the list is the thing to extend if that shows up.
;
; Measured on ui/admin_license_usage.gsx: redacted = 4 documents, 4 with parse
; errors; this form = 2 documents (one per literal), 0 errors.
;
; NOT injection.combined: that concatenates every match of the pattern across
; the whole tree into one document, which merged unrelated literals. Grouping
; is per-match, which is exactly per-literal.
((embedded_js_literal
   (embedded_open)
   .
   [(embedded_text) @injection.content
    (at_hole
      [(identifier)
       (selector_expression)
       (call_expression)
       (index_expression)
       (parenthesized_expression)
       (unary_expression)
       (binary_expression)
       (int_literal)
       (float_literal)
       (interpreted_string_literal)] @injection.content)]+)
 (#set! injection.language "javascript"))
((embedded_js_literal
   (embedded_open)
   .
   [(embedded_text_dq) @injection.content
    (at_hole
      [(identifier)
       (selector_expression)
       (call_expression)
       (index_expression)
       (parenthesized_expression)
       (unary_expression)
       (binary_expression)
       (int_literal)
       (float_literal)
       (interpreted_string_literal)] @injection.content)]+)
 (#set! injection.language "javascript"))

; css`…` literal text → CSS, but ONLY when its last declaration is
; `;`-terminated.
;
; Same grouping and hole handling as js above — a gsx hole in a css literal is
; always in VALUE position, so capturing its inner expression yields a
; well-formed declaration: `aspect-ratio: @{w} / @{h};` reaches the CSS parser
; as `aspect-ratio: w / h;`.
;
; The trailing `;` is the other half, and it is not cosmetic. tree-sitter-css
; roots at `stylesheet`, where a top-level declaration is only complete once
; terminated:
;
;   color: red     -> ERROR (incomplete rule)
;   color: red;    -> (declaration), clean
;
; So the #match? gate is the same "inject only a complete document" rule the js
; patterns follow. An unterminated literal is left alone and keeps gsx's own
; @string.special from highlights.scm — no colours, but no ERROR nodes over
; correct source either. Adding the trailing `;` is how an author opts in.
;
; Verified clean for `aspect-ratio: w / h;`, `--fill: 42%;`,
; `transform: translateX(-58%);` and multi-declaration bodies.
((embedded_css_literal
   (embedded_open)
   .
   [(embedded_text) @injection.content
    (at_hole
      [(identifier)
       (selector_expression)
       (call_expression)
       (index_expression)
       (parenthesized_expression)
       (unary_expression)
       (binary_expression)
       (int_literal)
       (float_literal)
       (interpreted_string_literal)] @injection.content)]+) @_css_lit
 (#match? @_css_lit ";[ \t\r\n]*[`\"]$")
 (#set! injection.language "css"))
