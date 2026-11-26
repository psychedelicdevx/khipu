(comment) @comment

(number) @number
(negative_number) @number
(string) @string
(escape_sequence) @string.escape
(interpolation
  "${" @punctuation.special
  "}" @punctuation.special)

[(true) (false)] @boolean
(none) @constant.builtin

((identifier) @variable.builtin
  (#eq? @variable.builtin "self"))

((type_identifier) @type.builtin
  (#any-of? @type.builtin "number" "bool" "string" "any" "list" "record"))
(type_identifier) @type
(type_parameters (identifier) @type)

(struct_declaration name: (identifier) @type)
(enum_declaration name: (identifier) @type)
(struct_literal name: (identifier) @type)
(enum_variant name: (identifier) @constant)
(variant_pattern enum: (identifier) @type)
(variant_pattern variant: (identifier) @constant)

(field_declaration name: (identifier) @property)
(record_field name: (identifier) @property)
(field_expression field: (identifier) @property)

(function_declaration name: (identifier) @function)
(parameter name: (identifier) @variable.parameter)
(call_expression function: (identifier) @function.call)
(call_expression
  function: (field_expression field: (identifier) @function.method.call))
((call_expression function: (identifier) @function.builtin)
  (#any-of? @function.builtin "print" "len" "push" "range" "keys" "assert" "assert_eq" "spawn" "channel" "send" "receive"))

((identifier) @type
  (#match? @type "^[A-Z]"))

(test_block name: (string) @string.special)

[
  "let"
  "fn"
  "struct"
  "enum"
] @keyword

[
  "if"
  "else"
  "match"
] @keyword.conditional

[
  "while"
  "for"
  "in"
  (break_statement)
  (continue_statement)
] @keyword.repeat

"return" @keyword.return

[
  "import"
  "from"
] @keyword.import

(test_block keyword: (identifier) @keyword)
(contract keyword: (identifier) @keyword)
(select_statement keyword: (identifier) @keyword.conditional)

[
  "=" "==" "!=" "<" "<=" ">" ">=" "+" "-" "*" "/" "%" "&&" "||" "!" "??" "->" "=>"
] @operator

["." "?." "," ":" "?"] @punctuation.delimiter

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
