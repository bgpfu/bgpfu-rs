/// Parser for RSPL filter expressions.
#[derive(Debug, Parser)]
#[grammar = "parser/grammar.pest"]
pub struct FilterParser;

#[cfg(test)]
#[allow(non_fmt_panic)]
mod tests {
    use paste::paste;
    use pest::{consumes_to, parses_to};

    use super::*;

    #[test]
    fn parse_autnum() {
        parses_to! {
            parser: FilterParser,
            input: "AS65000",
            rule: Rule::autnum,
            tokens: [autnum(0, 7)]
        }
    }

    #[test]
    fn parse_as_set() {
        parses_to! {
            parser: FilterParser,
            input: "AS-FOO",
            rule: Rule::as_set,
            tokens: [as_set(0, 6)]
        }
    }

    #[test]
    fn parse_hierarchical_as_set() {
        parses_to! {
            parser: FilterParser,
            input: "AS65000:AS-FOO",
            rule: Rule::as_set,
            tokens: [as_set(0, 14)]
        }
    }

    #[test]
    fn parse_route_set() {
        parses_to! {
            parser: FilterParser,
            input: "RS-FOO",
            rule: Rule::route_set,
            tokens: [route_set(0, 6)]
        }
    }

    #[test]
    fn parse_hierarchical_route_set() {
        parses_to! {
            parser: FilterParser,
            input: "RS-FOO:RS-BAR",
            rule: Rule::route_set,
            tokens: [route_set(0, 13)]
        }
    }

    #[test]
    fn parse_filter_set() {
        parses_to! {
            parser: FilterParser,
            input: "FLTR-FOO",
            rule: Rule::filter_set,
            tokens: [filter_set(0, 8)]
        }
    }

    #[test]
    fn parse_hierarchical_filter_set() {
        parses_to! {
            parser: FilterParser,
            input: "AS65000:FLTR-FOO:PeerAS",
            rule: Rule::filter_set,
            tokens: [filter_set(0, 23)]
        }
    }

    #[test]
    fn parse_ipv4_prefix() {
        parses_to! {
            parser: FilterParser,
            input: "192.0.2.0/24",
            rule: Rule::ipv4_prefix,
            tokens: [ipv4_prefix(0, 12)]
        }
    }

    #[test]
    fn parse_ipv6_prefix() {
        parses_to! {
            parser: FilterParser,
            input: "2001:db8::/32",
            rule: Rule::ipv6_prefix,
            tokens: [ipv6_prefix(0, 13)]
        }
    }

    #[test]
    fn parse_ipv4_prefix_range() {
        parses_to! {
            parser: FilterParser,
            input: "192.0.2.0/24^-",
            rule: Rule::ranged_prefix,
            tokens: [
                ranged_prefix(0, 14, [
                    ipv4_prefix(0, 12),
                    less_excl(12, 14)
                ])
            ]
        }
    }

    #[test]
    fn parse_ipv6_prefix_range() {
        parses_to! {
            parser: FilterParser,
            input: "2001:db8:f00::/48^+",
            rule: Rule::ranged_prefix,
            tokens: [
                ranged_prefix(0, 19, [
                    ipv6_prefix(0, 17),
                    less_incl(17, 19)
                ])
            ]
        }
    }

    #[test]
    fn parse_ipv4_literal_prefix_set_singleton() {
        parses_to! {
            parser: FilterParser,
            input: "{ 192.0.2.0/24^26 }",
            rule: Rule::literal_prefix_set,
            tokens: [
                literal_prefix_set(0, 19, [
                    ranged_prefix(2, 17, [
                        ipv4_prefix(2, 14),
                        exact(14, 17, [
                            num(15, 17)
                        ])
                    ])
                ])
            ]
        }
    }

    #[test]
    fn parse_ipv6_literal_prefix_set_singleton() {
        parses_to! {
            parser: FilterParser,
            input: "{ 2001:db8::/32^48 }",
            rule: Rule::literal_prefix_set,
            tokens: [
                literal_prefix_set(0, 20, [
                    ranged_prefix(2, 18, [
                        ipv6_prefix(2, 15),
                        exact(15, 18, [
                            num(16, 18)
                        ])
                    ])
                ])
            ]
        }
    }

    #[test]
    fn parse_ipv4_literal_prefix_set_multiple() {
        parses_to! {
            parser: FilterParser,
            input: "{ 192.0.2.0/24, 10.0.0.0/8^+, }",
            rule: Rule::literal_prefix_set,
            tokens: [
                literal_prefix_set(0, 31, [
                    ranged_prefix(2, 14, [
                        ipv4_prefix(2, 14)
                    ]),
                    ranged_prefix(16, 28, [
                        ipv4_prefix(16, 26),
                        less_incl(26, 28)
                    ])
                ])
            ]
        }
    }

    #[test]
    fn parse_mixed_literal_prefix_set_multiple() {
        parses_to! {
            parser: FilterParser,
            input: "{ 2001:db8:baa::/48^56-64, 10.0.0.0/8^+, }",
            rule: Rule::literal_prefix_set,
            tokens: [
                literal_prefix_set(0, 42, [
                    ranged_prefix(2, 25, [
                        ipv6_prefix(2, 19),
                        range(19, 25, [
                            num(20, 22),
                            num(23, 25)
                        ])
                    ]),
                    ranged_prefix(27, 39, [
                        ipv4_prefix(27, 37),
                        less_incl(37, 39)
                    ])
                ])
            ]
        }
    }

    #[test]
    fn parse_named_prefix_set_any() {
        parses_to! {
            parser: FilterParser,
            input: "ANY",
            rule: Rule::named_prefix_set,
            tokens: [
                named_prefix_set(0, 3, [
                    any_route(0, 3)
                ])
            ]
        }
    }

    #[test]
    fn parse_named_prefix_set_peeras() {
        parses_to! {
            parser: FilterParser,
            input: "PeerAS",
            rule: Rule::named_prefix_set,
            tokens: [
                named_prefix_set(0, 6, [
                    peeras(0, 6)
                ])
            ]
        }
    }

    #[test]
    fn parse_named_prefix_set_autnum() {
        parses_to! {
            parser: FilterParser,
            input: "AS65512",
            rule: Rule::named_prefix_set,
            tokens: [
                named_prefix_set(0, 7, [
                    autnum(0, 7)
                ])
            ]
        }
    }

    #[test]
    fn parse_named_prefix_set_as_set() {
        parses_to! {
            parser: FilterParser,
            input: "AS-BAR",
            rule: Rule::named_prefix_set,
            tokens: [
                named_prefix_set(0, 6, [
                    as_set(0, 6)
                ])
            ]
        }
    }

    #[test]
    fn parse_literal_filter_ranged_as_set() {
        parses_to! {
            parser: FilterParser,
            input: "AS-BAR^+",
            rule: Rule::literal_filter,
            tokens: [
                literal_filter(0, 8, [
                    named_prefix_set(0, 6, [
                        as_set(0, 6)
                    ]),
                    less_incl(6, 8)
                ])
            ]
        }
    }

    macro_rules! parse_filters {
        ( $( $name:ident: $filter:expr => [ $( $names:ident $calls:tt ),* $(,)* ] ),* $(,)? ) => {
            paste! {
                $(
                    #[test]
                    fn [< $name _filter_parses >]() {
                        parses_to! {
                            parser: FilterParser,
                            input: $filter,
                            rule: Rule::filter,
                            tokens: [ $( $names $calls ),* ]
                        }
                    }
                )*
            }
        }
    }

    parse_filters! {
        empty_literal: "{}" => [
            filter_expr_unit(0, 2, [
                literal_filter(0, 2, [
                    literal_prefix_set(0, 2)
                ])
            ])
        ],
        singleton_literal: "{ 10.0.0.0/0 }" => [
            filter_expr_unit(0, 14, [
                literal_filter(0, 14, [
                    literal_prefix_set(0, 14, [
                        ranged_prefix(2, 12, [
                            ipv4_prefix(2, 12)
                        ])
                    ])
                ])
            ])
        ],
        single_filter_set: "FLTR-FOO" => [
            filter_expr_unit(0, 8, [
                named_filter(0, 8, [
                    filter_set(0, 8)
                ])
            ])
        ],
        single_autnum: "AS-FOO" => [
            filter_expr_unit(0, 6, [
                literal_filter(0, 6, [
                    named_prefix_set(0, 6, [
                        as_set(0, 6)
                    ])
                ])
            ])
        ],
        parens_autnum: "(AS-FOO)" => [
            filter_expr_unit(0, 8, [
                filter_expr_unit(1, 7, [
                    literal_filter(1, 7, [
                        named_prefix_set(1, 7, [
                            as_set(1, 7)
                        ])
                    ])
                ])
            ])
        ],
        not_expr: "NOT AS65000" => [
            filter_expr_not(0, 11, [
                literal_filter(4, 11, [
                    named_prefix_set(4, 11, [
                        autnum(4, 11)
                    ])
                ])
            ])
        ],
        and_expr: "{ 192.0.2.0/24 } AND AS-FOO" => [
            filter_expr_and(0, 27, [
                literal_filter(0, 16, [
                    literal_prefix_set(0, 16, [
                        ranged_prefix(2, 14, [
                            ipv4_prefix(2, 14)
                        ])
                    ])
                ]),
                literal_filter(21, 27, [
                    named_prefix_set(21, 27, [
                        as_set(21, 27)
                    ])
                ])
            ])
        ],
        or_expr: "FLTR-FOO OR RS-BAR" => [
            filter_expr_or(0, 18, [
                named_filter(0, 8, [
                    filter_set(0, 8)
                ]),
                literal_filter(12, 18, [
                    named_prefix_set(12, 18, [
                        route_set(12, 18)
                    ])
                ])
            ])
        ],
        complex_expr: "((PeerAS^+ OR AS65000:AS-FOO:PeerAS^+) AND {0.0.0.0/0^8-24})" => [
            filter_expr_unit(0, 60, [
                filter_expr_and(1, 59, [
                    filter_expr_or(2, 37, [
                        literal_filter(2, 10, [
                            named_prefix_set(2, 8, [
                                peeras(2, 8)
                            ]),
                            less_incl(8, 10)
                        ]),
                        literal_filter(14, 37, [
                            named_prefix_set(14, 35, [
                                as_set(14, 35)
                            ]),
                            less_incl(35, 37)
                        ])
                    ]),
                    literal_filter(43, 59, [
                        literal_prefix_set(43, 59, [
                            ranged_prefix(44, 58, [
                                ipv4_prefix(44, 53),
                                range(53, 58, [
                                    num(54, 55),
                                    num(56, 58)
                                ])
                            ])
                        ])
                    ])
                ])
            ])
        ]
    }
}
