const PREC = {
  coalesce: 1,
  or: 2,
  and: 3,
  equality: 4,
  comparison: 5,
  term: 6,
  factor: 7,
  unary: 8,
  call: 9,
};

const commaSep = (rule) => optional(seq(rule, repeat(seq(',', rule)), optional(',')));
const commaSep1 = (rule) => seq(rule, repeat(seq(',', rule)), optional(','));

module.exports = grammar({
  name: 'khipu',

  extras: ($) => [/\s/, $.comment],

  word: ($) => $.identifier,

  conflicts: ($) => [
    [$._expression, $.struct_literal],
    [$.block, $.record],
    [$.test_block, $._expression],
    [$.select_statement, $._expression, $.struct_literal],
    [$.select_statement, $.struct_literal],
  ],

  rules: {
    source_file: ($) => repeat($._statement),

    _statement: ($) =>
      choice(
        $.let_statement,
        $.function_declaration,
        $.return_statement,
        $.if_statement,
        $.while_statement,
        $.for_statement,
        $.break_statement,
        $.continue_statement,
        $.import_statement,
        $.struct_declaration,
        $.enum_declaration,
        $.test_block,
        $.select_statement,
        $.assignment,
        $.expression_statement,
      ),

    let_statement: ($) =>
      seq(
        'let',
        field('name', $.identifier),
        optional(seq(':', field('type', $._type))),
        '=',
        field('value', $._expression),
      ),

    function_declaration: ($) =>
      seq(
        'fn',
        field('name', $.identifier),
        optional(field('type_parameters', $.type_parameters)),
        field('parameters', $.parameters),
        optional(seq('->', field('return_type', $._type))),
        field('body', $.block),
      ),

    type_parameters: ($) => seq('<', commaSep1($.identifier), '>'),

    parameters: ($) => seq('(', commaSep($.parameter), ')'),

    parameter: ($) =>
      seq(field('name', $.identifier), optional(seq(':', field('type', $._type)))),

    return_statement: ($) => prec.right(seq('return', optional($._expression))),

    if_statement: ($) =>
      seq(
        'if',
        field('condition', $._expression),
        field('consequence', $.block),
        optional(seq('else', field('alternative', choice($.block, $.if_statement)))),
      ),

    while_statement: ($) =>
      seq('while', field('condition', $._expression), field('body', $.block)),

    for_statement: ($) =>
      seq(
        'for',
        field('name', $.identifier),
        'in',
        field('iterable', $._expression),
        field('body', $.block),
      ),

    break_statement: (_) => 'break',

    continue_statement: (_) => 'continue',

    import_statement: ($) =>
      seq(
        'import',
        choice(field('name', $.identifier), seq('{', commaSep1(field('name', $.identifier)), '}')),
        'from',
        field('path', $.string),
      ),

    struct_declaration: ($) =>
      seq(
        'struct',
        field('name', $.identifier),
        '{',
        repeat(seq(choice($.field_declaration, $.function_declaration), optional(','))),
        '}',
      ),

    field_declaration: ($) => seq(field('name', $.identifier), ':', field('type', $._type)),

    enum_declaration: ($) =>
      seq('enum', field('name', $.identifier), '{', repeat(seq($.enum_variant, optional(','))), '}'),

    enum_variant: ($) =>
      seq(field('name', $.identifier), optional(seq('(', commaSep1($._type), ')'))),

    test_block: ($) =>
      seq(field('keyword', $.identifier), field('name', $.string), field('body', $.block)),

    select_statement: ($) =>
      prec.dynamic(
        2,
        seq(
          field('keyword', $.identifier),
          '{',
          repeat(seq($.select_arm, optional(','))),
          '}',
        ),
      ),

    select_arm: ($) =>
      seq(
        optional(seq(field('name', $.identifier), '=')),
        field('receive', $.call_expression),
        '=>',
        field('body', choice(prec.dynamic(1, $.block), $._statement)),
      ),

    assignment: ($) =>
      prec.right(seq(field('left', $._expression), '=', field('right', $._expression))),

    expression_statement: ($) => $._expression,

    block: ($) => seq('{', repeat($._statement), '}'),

    _expression: ($) =>
      choice(
        $.identifier,
        $.number,
        $.string,
        $.true,
        $.false,
        $.none,
        $.list,
        $.record,
        $.struct_literal,
        $.function_expression,
        $.match_expression,
        $.parenthesized_expression,
        $.unary_expression,
        $.binary_expression,
        $.call_expression,
        $.index_expression,
        $.field_expression,
      ),

    binary_expression: ($) =>
      choice(
        ...[
          ['??', PREC.coalesce],
          ['||', PREC.or],
          ['&&', PREC.and],
          ['==', PREC.equality],
          ['!=', PREC.equality],
          ['<', PREC.comparison],
          ['<=', PREC.comparison],
          ['>', PREC.comparison],
          ['>=', PREC.comparison],
          ['+', PREC.term],
          ['-', PREC.term],
          ['*', PREC.factor],
          ['/', PREC.factor],
          ['%', PREC.factor],
        ].map(([operator, precedence]) =>
          prec.left(
            precedence,
            seq(
              field('left', $._expression),
              field('operator', operator),
              field('right', $._expression),
            ),
          ),
        ),
      ),

    unary_expression: ($) =>
      prec(PREC.unary, seq(field('operator', choice('-', '!')), field('operand', $._expression))),

    call_expression: ($) =>
      prec(PREC.call, seq(field('function', $._expression), field('arguments', $.arguments))),

    arguments: ($) => seq('(', commaSep($._expression), ')'),

    index_expression: ($) =>
      prec(PREC.call, seq(field('value', $._expression), '[', field('index', $._expression), ']')),

    field_expression: ($) =>
      prec(
        PREC.call,
        seq(field('value', $._expression), choice('.', '?.'), field('field', $.identifier)),
      ),

    parenthesized_expression: ($) => seq('(', $._expression, ')'),

    list: ($) => seq('[', commaSep($._expression), ']'),

    record: ($) => seq('{', commaSep($.record_field), '}'),

    record_field: ($) =>
      seq(field('name', choice($.identifier, $.string)), ':', field('value', $._expression)),

    struct_literal: ($) =>
      prec.dynamic(1, seq(field('name', $.identifier), '{', commaSep($.record_field), '}')),

    function_expression: ($) =>
      seq(
        'fn',
        optional(field('type_parameters', $.type_parameters)),
        field('parameters', $.parameters),
        optional(seq('->', field('return_type', $._type))),
        field('body', $.block),
      ),

    match_expression: ($) =>
      seq(
        'match',
        field('value', $._expression),
        '{',
        repeat(seq($.match_arm, optional(','))),
        '}',
      ),

    match_arm: ($) =>
      seq(
        field('pattern', $._pattern),
        '=>',
        field('body', choice(prec.dynamic(1, $.block), $._expression)),
      ),

    _pattern: ($) =>
      choice(
        $.identifier,
        $.number,
        $.negative_number,
        $.string,
        $.true,
        $.false,
        $.none,
        $.variant_pattern,
      ),

    negative_number: ($) => seq('-', $.number),

    variant_pattern: ($) =>
      choice(
        seq(field('enum', $.identifier), '.', field('variant', $.identifier), optional($.bindings)),
        seq(field('variant', $.identifier), $.bindings),
      ),

    bindings: ($) => seq('(', commaSep1($.identifier), ')'),

    _type: ($) =>
      choice(
        alias($.identifier, $.type_identifier),
        $.generic_type,
        $.function_type,
        $.none,
        $.optional_type,
      ),

    generic_type: ($) => seq(field('name', alias($.identifier, $.type_identifier)), '<', field('argument', $._type), '>'),

    function_type: ($) =>
      prec.right(
        seq('fn', '(', commaSep($._type), ')', optional(seq('->', field('return_type', $._type)))),
      ),

    optional_type: ($) => seq($._type, '?'),

    string: ($) =>
      seq('"', repeat(choice($.string_content, $.escape_sequence, $.interpolation)), '"'),

    string_content: (_) =>
      choice(token.immediate(prec(1, /[^"\\$\n]+/)), token.immediate('$')),

    escape_sequence: (_) => token.immediate(/\\[nt"\\$]/),

    interpolation: ($) => seq(token.immediate('${'), $._expression, '}'),

    number: (_) => /\d+(\.\d+)?/,

    true: (_) => 'true',

    false: (_) => 'false',

    none: (_) => 'none',

    identifier: (_) => /[A-Za-z_][A-Za-z0-9_]*/,

    comment: (_) => token(seq('//', /.*/)),
  },
});
