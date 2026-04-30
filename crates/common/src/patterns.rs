use regex::Regex;

pub const COLUMN_DENYLIST: &[&str] = &[
    "email",
    "ssn",
    "dob",
    "phone",
    "npi",
    "credit_card",
    "card_number",
    "cvv",
    "passport",
    "license_number",
    "full_name",
    "first_name",
    "last_name",
    "birthdate",
];

pub struct BuiltinPattern {
    pub name: &'static str,
    pub regex: &'static str,
    pub confidence: f32,
}

pub const BUILTIN_PATTERNS: &[BuiltinPattern] = &[
    BuiltinPattern {
        name: "email",
        regex: r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}\b",
        confidence: 0.85,
    },
    BuiltinPattern {
        name: "ssn",
        regex: r"\b\d{3}-\d{2}-\d{4}\b",
        confidence: 0.95,
    },
    BuiltinPattern {
        name: "phone",
        regex: r"\b(\+?1[\s.-]?)?\(?\d{3}\)?[\s.-]?\d{3}[\s.-]?\d{4}\b",
        confidence: 0.75,
    },
    BuiltinPattern {
        name: "credit_card",
        regex: r"\b(?:\d[ -]?){13,16}\b",
        confidence: 0.7,
    },
];

pub struct CompiledPattern {
    pub name: String,
    pub regex: Regex,
    pub confidence: f32,
}

impl CompiledPattern {
    pub fn from_builtins() -> Vec<Self> {
        BUILTIN_PATTERNS
            .iter()
            .map(|p| CompiledPattern {
                name: p.name.to_string(),
                regex: Regex::new(p.regex).expect("builtin regex is valid"),
                confidence: p.confidence,
            })
            .collect()
    }
}

pub struct Luhn;

impl Luhn {
    pub fn check(s: &str) -> bool {
        let digits: Vec<u32> = s
            .chars()
            .filter(|c| c.is_ascii_digit())
            .filter_map(|c| c.to_digit(10))
            .collect();
        if digits.len() < 13 || digits.len() > 19 {
            return false;
        }
        let sum: u32 = digits
            .iter()
            .rev()
            .enumerate()
            .map(|(i, &d)| {
                if i % 2 == 1 {
                    let v = d * 2;
                    if v > 9 {
                        v - 9
                    } else {
                        v
                    }
                } else {
                    d
                }
            })
            .sum();
        sum.is_multiple_of(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(name: &str) -> CompiledPattern {
        CompiledPattern::from_builtins()
            .into_iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("builtin pattern '{}' not found", name))
    }

    // --- CompiledPattern::from_builtins ---

    #[test]
    fn all_four_builtins_present() {
        let patterns = CompiledPattern::from_builtins();
        let names: Vec<&str> = patterns.iter().map(|p| p.name.as_str()).collect();
        for expected in &["email", "ssn", "phone", "credit_card"] {
            assert!(names.contains(expected), "missing builtin: {}", expected);
        }
        assert_eq!(patterns.len(), 4);
    }

    #[test]
    fn builtin_confidences_in_range() {
        for p in CompiledPattern::from_builtins() {
            assert!(
                p.confidence > 0.0 && p.confidence <= 1.0,
                "{}: confidence {} out of range",
                p.name,
                p.confidence
            );
        }
    }

    // --- Column denylist ---

    #[test]
    fn denylist_contains_required_entries() {
        let required = [
            "email",
            "ssn",
            "dob",
            "phone",
            "npi",
            "credit_card",
            "card_number",
            "cvv",
            "passport",
            "license_number",
            "full_name",
            "first_name",
            "last_name",
            "birthdate",
        ];
        for entry in &required {
            assert!(
                COLUMN_DENYLIST.contains(entry),
                "missing denylist entry: {}",
                entry
            );
        }
    }

    // --- Email ---

    #[test]
    fn email_matches_golden_corpus() {
        let p = pattern("email");
        for addr in &[
            "user@example.com",
            "john.doe+tag@company.co.uk",
            "admin@sub.domain.org",
            "test123@mail.io",
            "UPPER@EXAMPLE.COM",
        ] {
            assert!(p.regex.is_match(addr), "expected email match: {}", addr);
        }
    }

    #[test]
    fn email_rejects_negatives() {
        let p = pattern("email");
        for s in &["notanemail", "missing-at-sign.com", "two@@ats.com"] {
            assert!(!p.regex.is_match(s), "unexpected email match: {}", s);
        }
    }

    // --- SSN ---

    #[test]
    fn ssn_matches_golden_corpus() {
        let p = pattern("ssn");
        for ssn in &["123-45-6789", "987-65-4321", "000-12-3456"] {
            assert!(p.regex.is_match(ssn), "expected SSN match: {}", ssn);
        }
    }

    #[test]
    fn ssn_rejects_negatives() {
        let p = pattern("ssn");
        for s in &[
            "123456789",   // no dashes
            "12-345-6789", // wrong grouping
            "1234-56-789", // wrong grouping
        ] {
            assert!(!p.regex.is_match(s), "unexpected SSN match: {}", s);
        }
    }

    // --- Phone ---

    #[test]
    fn phone_matches_golden_corpus() {
        let p = pattern("phone");
        for num in &[
            "555-123-4567",
            "(555) 123-4567",
            "+1 555-123-4567",
            "555.123.4567",
            "5551234567",
        ] {
            assert!(p.regex.is_match(num), "expected phone match: {}", num);
        }
    }

    #[test]
    fn phone_rejects_negatives() {
        let p = pattern("phone");
        for s in &["hello world", "not a number", "12345"] {
            assert!(!p.regex.is_match(s), "unexpected phone match: {}", s);
        }
    }

    // --- Credit card regex ---

    #[test]
    fn credit_card_regex_matches_13_to_16_digit_strings() {
        let p = pattern("credit_card");
        for s in &[
            "4532015112830366", // 16 digits
            "4111111111111111", // 16 digits
            "5500005555555559", // 16 digits
            "1234567890123",    // 13 digits
        ] {
            assert!(
                p.regex.is_match(s),
                "expected credit_card regex match: {}",
                s
            );
        }
    }

    #[test]
    fn credit_card_regex_rejects_too_few_digits() {
        let p = pattern("credit_card");
        // 11 and 12 digits are below the {13,16} minimum
        assert!(!p.regex.is_match("12345678901"));
        assert!(!p.regex.is_match("123456789012"));
    }

    // --- Luhn ---

    #[test]
    fn luhn_valid_cards() {
        // Well-known test card numbers
        for card in &[
            "4111111111111111", // Visa
            "5500005555555559", // Mastercard
            "371449635398431",  // Amex (15 digits)
            "6011111111111117", // Discover
            "4532015112830366", // Visa
        ] {
            assert!(Luhn::check(card), "expected Luhn valid: {}", card);
        }
    }

    #[test]
    fn luhn_invalid_cards() {
        for card in &[
            "4111111111111112", // Visa off-by-one
            "1234567890123456", // random digits
            "9999999999999999", // all nines
            "4532015112830367", // Visa off-by-one
        ] {
            assert!(!Luhn::check(card), "expected Luhn invalid: {}", card);
        }
    }

    #[test]
    fn luhn_rejects_too_short() {
        assert!(!Luhn::check("123456789012")); // 12 digits
        assert!(!Luhn::check("1234"));
        assert!(!Luhn::check(""));
    }

    #[test]
    fn luhn_rejects_too_long() {
        // 20 digits — over the 19-digit max
        assert!(!Luhn::check("12345678901234567890"));
    }

    #[test]
    fn luhn_strips_spaces_and_dashes() {
        // Spaces and dashes are filtered; underlying digits are validated.
        assert!(Luhn::check("4111 1111 1111 1111"));
        assert!(Luhn::check("4111-1111-1111-1111"));
    }

    #[test]
    fn luhn_non_digit_chars_ignored() {
        // Only digits count; letters are stripped.
        // "4111111111111111" valid, so same with non-digit noise that doesn't change digit count.
        // Padding with letters shouldn't cause a 20-digit rejection since letters are filtered.
        assert!(Luhn::check("4111111111111111abc")); // still 16 digits after filtering
    }
}
